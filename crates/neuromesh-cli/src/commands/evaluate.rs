use neuromesh_context::benchmark_suite::{
    aggregate_cell_results, BenchmarkCellResult, ReleaseGateReport,
};
use neuromesh_context::gold::{
    evaluate_view, fixture_gold_cases, load_gold_tasks, packet_file_names, packet_paths,
};
use neuromesh_context::learning_eval::{
    compute_ranking_metrics, dose_response_rank, emitted_paths_from_view,
};
use neuromesh_context::retrieval::failure::{classify_retrieval_failure, FailureClass};
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_task::TaskSignatureExtractor;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub fn execute(args: &[String]) -> Result<()> {
    let learning_mode = args.iter().any(|a| a == "--learning");
    let release_gates = args.iter().any(|a| a == "--release-gates");
    let calibrate = args.iter().any(|a| a == "--calibrate");
    let current_dir = neuromesh_index::assert_safe_workspace(&env::current_dir()?)?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project_id = ProjectId::new(&project_name);
    let walker = super::configured_walker(
        current_dir.clone(),
        project_id.clone(),
        super::FileCapArg::Unspecified,
    );
    let scanned = walker.scan()?;
    if scanned.is_empty() {
        println!(
            "No source files found in {}. Nothing to evaluate.",
            current_dir.display()
        );
        return Ok(());
    }

    let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
    let index_started = Instant::now();
    graph.ingest_workspace(&scanned);
    #[cfg(feature = "embeddings")]
    {
        let emb = neuromesh_core::Config::load().embeddings;
        if emb.enabled {
            let _ = neuromesh_graph::maybe_rebuild_embeddings(&graph, &current_dir, &emb);
            let _ = neuromesh_embed::Embedder::warm(emb);
        }
    }
    let index_ms = index_started.elapsed().as_millis();
    let stats = graph.stats();
    let workspace_tokens = graph.total_tokens().max(1);

    let tasks = {
        let gold_path = current_dir.join("tests").join("gold_tasks.toml");
        if gold_path.exists() {
            load_gold_tasks(&gold_path)
        } else {
            neuromesh_context::gold::builtin_gold_tasks()
        }
    };

    let registry = Arc::new(ReversibleContextRegistry::new());
    let activator = ContextActivator::new(registry);

    if learning_mode {
        return execute_learning_eval(&current_dir, &graph, &activator);
    }

    println!("\nNeuroMesh evaluation — {}", project_name);
    println!("Workspace: {}", current_dir.display());
    println!(
        "Indexed {} files · {} nodes · {} edges · {} workspace tokens · index {} ms\n",
        scanned.len(),
        stats.total_nodes,
        stats.total_edges,
        workspace_tokens,
        index_ms
    );
    println!(
        "{:<28} {:<12} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>5} {:>6}",
        "Task",
        "Mode",
        "WS tok",
        "Selected",
        "Packet",
        "vsWS%",
        "vsSel%",
        "Recall",
        "Prec",
        "Grep",
        "ms"
    );
    println!("{}", "-".repeat(118));

    for task in &tasks {
        let signature = TaskSignatureExtractor::extract(&task.prompt);
        for mode in [
            OptimizationMode::MaxSavings,
            OptimizationMode::Balanced,
            OptimizationMode::MaxQuality,
        ] {
            let started = Instant::now();
            let view = if mode == OptimizationMode::Balanced {
                activator.activate_tiered(&graph, &signature, mode)
            } else {
                activator.activate(&graph, &signature, mode)
            };
            let ms = started.elapsed().as_millis();
            let metrics = evaluate_view(task, &view, ms as u64);
            let files = packet_file_names(&view);
            println!(
                "{:<28} {:<12} {:>8} {:>8} {:>8} {:>6.1}% {:>6.1}% {:>7.2} {:>7.2} {:>5} {:>6}",
                if task.id.len() > 27 {
                    format!("{}…", &task.id[..26])
                } else {
                    task.id.clone()
                },
                view.budget_mode,
                metrics.workspace_tokens,
                metrics.selected_raw,
                metrics.packet_tokens,
                metrics.reduction_vs_workspace,
                metrics.reduction_vs_selected,
                metrics.recall,
                metrics.precision,
                metrics.grep_still_needed,
                ms
            );
            if mode == OptimizationMode::Balanced && !files.is_empty() {
                let mut names: Vec<_> = packet_paths(&view).into_iter().collect();
                names.sort();
                println!("    files: {}", names.join(", "));
            }
        }
    }

    println!("\nFill caps: max_savings=0 extra · balanced=5000 extra · max_quality=16000 extra.");
    println!("Seeds always ship (a large target function can exceed the fill cap).");
    println!("WS tok = indexed workspace. Selected = raw tokens of packet files before fold. Packet = after fold.");
    println!("Grep = 0 when gold files are already in the packet (recall 1.0); 1 otherwise.");

    if release_gates {
        let mut cells = Vec::new();
        for (i, task) in tasks.iter().enumerate() {
            let signature = TaskSignatureExtractor::extract(&task.prompt);
            let started = Instant::now();
            let view = activator.activate_tiered(&graph, &signature, OptimizationMode::Balanced);
            let ms = started.elapsed().as_millis() as u64;
            let metrics = evaluate_view(task, &view, ms);
            let claim = view
                .retrieval
                .as_ref()
                .map(|r| r.claim.as_str())
                .unwrap_or("unknown");
            let level = view
                .retrieval
                .as_ref()
                .map(|r| r.retrieval_level.clone())
                .unwrap_or_else(|| "L1".into());
            let critical = view
                .retrieval
                .as_ref()
                .map(|r| r.critical_gaps.len())
                .unwrap_or(0);
            let failure = classify_retrieval_failure(
                claim,
                view.coverage
                    .as_ref()
                    .map(|c| c.seeds_hit.len())
                    .unwrap_or(0),
                critical,
                view.over_budget,
                &level,
                None,
            );
            let embedding_primary = view
                .retrieval
                .as_ref()
                .and_then(|r| r.resolution_tier.as_deref())
                .is_some_and(|t| t == "embedding_primary")
                || view
                    .retrieval
                    .as_ref()
                    .map(|r| r.embedding_used)
                    .unwrap_or(false);
            let no_seed = claim == "no_seed_resolved"
                || claim == "no_confident_match"
                || failure == FailureClass::NoSeed
                || metrics.recall == 0.0;
            cells.push(BenchmarkCellResult {
                benchmark: "A_regression".into(),
                cell_id: task.id.clone(),
                split: format!(
                    "{:?}",
                    neuromesh_context::benchmark_suite::split_for_cell(i, tasks.len())
                ),
                recall: metrics.recall,
                precision: metrics.precision,
                task_success: None,
                claimed_sufficient: claim == "likely_sufficient",
                tokens: metrics.packet_tokens,
                latency_ms: ms,
                retrieval_level: level.clone(),
                failure_class: failure.as_str().to_string(),
                l1_ms: view
                    .retrieval
                    .as_ref()
                    .and_then(|r| r.latency_ms.get("L1").copied())
                    .unwrap_or(ms),
                l2_ms: view
                    .retrieval
                    .as_ref()
                    .and_then(|r| r.latency_ms.get("L2").copied()),
                l3_ms: view
                    .retrieval
                    .as_ref()
                    .and_then(|r| r.latency_ms.get("L3").copied()),
                no_seed,
                embedding_primary,
            });
        }
        let suite = aggregate_cell_results(&cells);
        let report = ReleaseGateReport::evaluate(&suite);
        println!(
            "\nRelease gates (Benchmark A): {}",
            if report.passed { "PASS" } else { "FAIL" }
        );
        println!("{}", serde_json::to_string_pretty(&report)?);

        if calibrate {
            use neuromesh_context::benchmark_suite::{split_for_cell, DataSplit};
            use neuromesh_context::retrieval::calibration::calibrate_likely_threshold;

            let dev_cells: Vec<_> = cells
                .iter()
                .enumerate()
                .filter(|(i, _)| split_for_cell(*i, cells.len()) == DataSplit::Dev)
                .map(|(_, c)| c)
                .collect();
            let scores: Vec<f32> = dev_cells
                .iter()
                .map(|c| if c.claimed_sufficient { 0.75 } else { 0.5 })
                .collect();
            let recall: Vec<f32> = dev_cells.iter().map(|c| c.recall).collect();
            let claimed: Vec<bool> = dev_cells.iter().map(|c| c.claimed_sufficient).collect();
            let report = calibrate_likely_threshold(&scores, |_| true, &recall, &claimed);
            println!("\nCalibration (dev split, n={}):", report.sample_count);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    let fixtures = current_dir.join("tests").join("fixtures");
    if fixtures.is_dir() {
        println!("\nFixture repos:");
        if let Ok(entries) = std::fs::read_dir(&fixtures) {
            let mut dirs: Vec<_> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            dirs.sort();
            for path in dirs {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let walker = ProjectWalker::new(path.clone(), ProjectId::new(&name));
                let Ok(scanned) = walker.scan() else {
                    println!("  {name}: scan failed");
                    continue;
                };
                if scanned.is_empty() {
                    println!("  {name}: 0 files (scan empty)");
                    continue;
                }
                let graph = NeuralProjectGraph::new(ProjectId::new(&name));
                graph.ingest_workspace(&scanned);
                let registry = Arc::new(ReversibleContextRegistry::new());
                let activator = ContextActivator::new(registry);
                println!("  {name}: {} files", scanned.len());
                let gold_path = path.join("gold_tasks.toml");
                let tasks = if gold_path.exists() {
                    load_gold_tasks(&gold_path)
                } else {
                    fixture_gold_cases()
                        .into_iter()
                        .filter(|(dir, _)| *dir == name)
                        .map(|(_, task)| task)
                        .collect()
                };
                for task in tasks {
                    let signature = TaskSignatureExtractor::extract(&task.prompt);
                    let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
                    let metrics = evaluate_view(&task, &view, 0);
                    println!(
                        "    {} recall={:.2} prec={:.2} vsWS={:.1}% vsSel={:.1}% grep={} files={:?}",
                        task.id,
                        metrics.recall,
                        metrics.precision,
                        metrics.reduction_vs_workspace,
                        metrics.reduction_vs_selected,
                        metrics.grep_still_needed,
                        packet_paths(&view)
                    );
                }
            }
        }
    }
    println!();

    neuromesh_observability::record_activity(neuromesh_observability::ActivityRecord {
        request_id: neuromesh_observability::cli_request_id("eval"),
        project_id: project_id.clone(),
        mode: "eval".into(),
        command: Some("eval".into()),
        surface: neuromesh_observability::TelemetrySurface::Cli,
        workspace_path: Some(current_dir.display().to_string()),
        client_id: None,
        tokens_before: workspace_tokens,
        tokens_after: workspace_tokens,
        token_reduction_pct: 0.0,
        nodes_before: 0,
        nodes_after: stats.total_nodes,
        expansions_count: tasks.len(),
        cache_hit: false,
        provider: "neuromesh-cli".into(),
        model: "eval".into(),
        latency_ms: index_ms as u64,
        success: true,
        task_id: Some(format!("eval {} tasks", tasks.len())),
    });

    Ok(())
}

fn execute_learning_eval(
    current_dir: &Path,
    graph: &NeuralProjectGraph,
    activator: &ContextActivator,
) -> Result<()> {
    let fixture = current_dir
        .join("tests")
        .join("fixtures")
        .join("learning-causal");
    if !fixture.is_dir() {
        println!("Learning eval requires tests/fixtures/learning-causal/");
        return Ok(());
    }
    let walker = ProjectWalker::new(fixture.clone(), ProjectId::new("learning-causal"));
    let scanned = walker.scan()?;
    let eval_graph = NeuralProjectGraph::new(ProjectId::new("learning-causal"));
    eval_graph.ingest_workspace(&scanned);
    eval_graph.finalize_links();

    let prompt = "how does promocodeinput component work in checkout";
    let signature = TaskSignatureExtractor::extract(prompt);
    let gold = vec!["src/components/PromoCodeInput.vue".into()];
    let levels: [i32; 8] = [0, 1, 2, 5, 10, 25, 50, 100];

    println!("\nNeuroMesh learning eval — dose-response\n");
    println!(
        "{:>6} {:>10} {:>10} {:>6} {:>8} {:>8}",
        "dose", "bonus", "score", "rank", "emitted", "MRR"
    );
    println!("{}", "-".repeat(58));

    let baseline = activator.activate(&eval_graph, &signature, OptimizationMode::Balanced);
    let mut last_view = baseline.clone();

    for &dose in &levels {
        if dose > 0 {
            if let Some(node) = eval_graph.resolve_feedback_node("PromoCodeInput") {
                for _ in 0..dose {
                    eval_graph.reinforce_node_access(&node.id, true);
                }
            }
        }
        let view = activator.activate(&eval_graph, &signature, OptimizationMode::Balanced);
        let bonus = eval_graph
            .node_learning_profile("PromoCodeInput")
            .map(|p| p.learning_bonus)
            .unwrap_or(0.0);
        let (rank, score, emitted) =
            dose_response_rank(&view.rank_candidates, "PromoCodeInput").unwrap_or((0, 0.0, false));
        let metrics = compute_ranking_metrics(&gold, &last_view, &view, 8);
        println!(
            "{:>6} {:>10.2} {:>10.2} {:>6} {:>8} {:>8.3}",
            dose, bonus, score, rank, emitted, metrics.mrr
        );
        last_view = view;
    }

    let emitted = emitted_paths_from_view(&last_view);
    println!("\nFinal emitted: {:?}", emitted);
    println!("Token efficiency (packet/workspace): {:.3}", {
        let ws = eval_graph.total_tokens().max(1) as f32;
        last_view.active_tokens as f32 / ws
    });
    let _ = graph;
    Ok(())
}
