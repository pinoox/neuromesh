use neuromesh_core::{Config, ProjectId, Result, RetrievalEngine};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::{MemoryDatabase, ProjectFact};
use std::fs;
use std::sync::Arc;

use super::{configured_walker, persist_file_cap, print_file_cap, FileCapArg};

pub fn execute(
    cap: FileCapArg,
    args: &[String],
) -> Result<(Arc<NeuralProjectGraph>, Arc<MemoryDatabase>)> {
    if let Some(mode) = index_mode_from_args(args) {
        std::env::set_var("NEUROMESH_ENGINE", mode.as_str());
    }
    let cfg = Config::load();
    let current_dir = neuromesh_index::assert_safe_workspace(&std::env::current_dir()?)?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("neuromesh-project")
        .to_string();

    if let Some(path) = persist_file_cap(cap)? {
        let label = match cap {
            FileCapArg::Auto => "auto".to_string(),
            FileCapArg::Limit(n) => n.to_string(),
            FileCapArg::Unspecified => unreachable!(),
        };
        println!("Saved          : {} (max_files = {label})", path.display());
    }

    let project_id = ProjectId::new(&project_name);
    let walker = configured_walker(current_dir.clone(), project_id.clone(), cap);

    println!(
        "🔍 Indexing Project Workspace (engine={})…",
        cfg.retrieval.engine.as_str()
    );
    let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
    let _ = graph.load_persisted(&current_dir);
    let report = walker.scan_report_with(&graph.file_fingerprints())?;
    let skipped_count = report.skipped_count();
    let skipped_summary = report.skipped_summary();
    let total_files = if report.present.is_empty() {
        report.files.len()
    } else {
        report.present.len()
    };
    let db_dir = neuromesh_core::ensure_project_data_dir(&current_dir)?;
    fs::create_dir_all(&db_dir)?;
    let memory_db = Arc::new(MemoryDatabase::open(&db_dir.join("neuromesh.json"))?);

    let mut total_tokens = 0;
    let mut has_vue = false;
    let mut has_scss = false;
    let mut has_ts = false;
    let mut has_html = false;
    let mut has_kotlin = false;
    let mut has_svelte = false;
    let mut has_js = false;

    for (file, _) in &report.files {
        total_tokens += file.token_count;
        if matches!(
            file.language,
            neuromesh_index::SourceLanguage::HTML
                | neuromesh_index::SourceLanguage::Twig
                | neuromesh_index::SourceLanguage::Svg
        ) {
            has_html = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Vue {
            has_vue = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Svelte {
            has_svelte = true;
        }
        if matches!(
            file.language,
            neuromesh_index::SourceLanguage::SCSS
                | neuromesh_index::SourceLanguage::CSS
                | neuromesh_index::SourceLanguage::Less
        ) {
            has_scss = true;
        }
        if file.language == neuromesh_index::SourceLanguage::TypeScript {
            has_ts = true;
        }
        if file.language == neuromesh_index::SourceLanguage::JavaScript {
            has_js = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Kotlin {
            has_kotlin = true;
        }
    }
    graph.ingest_scan_report(&report);
    graph.save_persisted(&current_dir)?;
    #[cfg(feature = "embeddings")]
    {
        let emb = cfg.embeddings.clone();
        if emb.index_on_build {
            let _ = neuromesh_graph::maybe_rebuild_embeddings(&graph, &current_dir, &emb);
        } else if emb.enabled {
            let sidecar = neuromesh_core::embeddings_path(&current_dir);
            if sidecar.exists() {
                let _ = graph.load_embedding_sidecar(&current_dir);
                println!("Embeddings   : sidecar loaded ({})", sidecar.display());
            } else if matches!(
                cfg.retrieval.engine,
                RetrievalEngine::Hybrid | RetrievalEngine::Deep
            ) {
                println!(
                    "Embeddings   : building sidecar (engine={})…",
                    cfg.retrieval.engine.as_str()
                );
                if let Err(e) =
                    neuromesh_graph::rebuild_embeddings_for_workspace(&graph, &current_dir, &emb)
                {
                    eprintln!("Embeddings   : rebuild failed ({e})");
                }
            } else {
                println!(
                    "Embeddings   : off or deferred — run `neuromesh embed rebuild` or `neuromesh index --mode hybrid`"
                );
            }
        } else {
            println!("Embeddings   : deferred until L3 (engine=fast, zero-embed index)");
        }
    }

    if has_html {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "structure",
            "markup_language",
            "HTML files are present in the workspace",
        ))?;
    }
    if has_vue {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "framework",
            "frontend_framework",
            "Vue single-file components are present",
        ))?;
    }
    if has_svelte {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "framework",
            "svelte",
            "Svelte components are present",
        ))?;
    }
    if has_js {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "javascript",
            "JavaScript files are present",
        ))?;
    }
    if has_scss {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "styling",
            "stylesheets",
            "SCSS, CSS, or LESS files are present",
        ))?;
    }
    if has_ts {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "type_system",
            "TypeScript files are present",
        ))?;
    }
    if has_kotlin {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "kotlin",
            "Kotlin files are present",
        ))?;
    }

    for fact in neuromesh_memory::extract_project_facts(&current_dir, &project_id) {
        memory_db.save_project_fact(&fact)?;
    }

    let stats = graph.stats();
    println!("✓ Indexing Complete");
    println!("  Indexed Files  : {}", total_files);
    print_file_cap(&report, "  ");
    if skipped_count > 0 {
        println!("  Skipped        : {} ({})", skipped_count, skipped_summary);
    }
    println!("  Processed Tokens : {}", total_tokens);
    println!("  Workspace Tokens : {}", graph.total_tokens());
    println!("  Graph Nodes    : {}", stats.total_nodes);
    println!("  Pheromone Edges: {}", stats.total_edges);

    neuromesh_observability::record_activity(neuromesh_observability::ActivityRecord {
        request_id: neuromesh_observability::cli_request_id("index"),
        project_id: project_id.clone(),
        mode: "index".into(),
        command: Some("index".into()),
        surface: neuromesh_observability::TelemetrySurface::Cli,
        workspace_path: Some(current_dir.display().to_string()),
        client_id: None,
        tokens_before: total_tokens,
        tokens_after: graph.total_tokens(),
        token_reduction_pct: 0.0,
        nodes_before: 0,
        nodes_after: stats.total_nodes,
        expansions_count: 0,
        cache_hit: false,
        provider: "neuromesh-cli".into(),
        model: "index".into(),
        latency_ms: 0,
        success: true,
        task_id: Some(format!("index {total_files} files")),
    });

    Ok((graph, memory_db))
}

fn index_mode_from_args(args: &[String]) -> Option<RetrievalEngine> {
    for (i, arg) in args.iter().enumerate() {
        if arg == "--mode" {
            if let Some(raw) = args.get(i + 1) {
                return RetrievalEngine::parse(raw);
            }
        }
        if let Some(raw) = arg.strip_prefix("--mode=") {
            return RetrievalEngine::parse(raw);
        }
    }
    None
}
