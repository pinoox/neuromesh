use neuromesh_context::gold::{
    evaluate_view, fixture_gold_cases, load_gold_tasks, packet_file_names, packet_paths,
};
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_task::TaskSignatureExtractor;
use std::env;
use std::sync::Arc;
use std::time::Instant;

pub fn execute() -> Result<()> {
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

    let graph = Arc::new(NeuralProjectGraph::new(project_id));
    let index_started = Instant::now();
    graph.ingest_workspace(&scanned);
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
            let view = activator.activate(&graph, &signature, mode);
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
    Ok(())
}
