use neuromesh_core::{Config, NeuroMeshError, Result};
use neuromesh_graph::NeuralProjectGraph;
use std::sync::Arc;
use std::time::Instant;

pub fn execute(args: &[String]) -> Result<()> {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("help");
    match sub {
        "prefetch" | "download" | "warm" => prefetch(args),
        "rebuild" => rebuild(args),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(NeuroMeshError::Config(format!(
            "unknown embed subcommand: {other} (use: embed prefetch | embed rebuild)"
        ))),
    }
}

fn prefetch(args: &[String]) -> Result<()> {
    #[cfg(not(feature = "embeddings"))]
    {
        return Err(NeuroMeshError::Config(
            "embed prefetch requires embeddings feature (reinstall release binary)".into(),
        ));
    }
    #[cfg(feature = "embeddings")]
    {
        let quiet = args.iter().any(|a| a == "--quiet" || a == "-q");
        let cfg = Config::load().embeddings;
        if !cfg.enabled {
            if !quiet {
                println!("Embeddings disabled — nothing to prefetch.");
            }
            return Ok(());
        }
        if !quiet {
            if neuromesh_embed::bundled_minilm_available() {
                if let Some(dir) = neuromesh_embed::resolve_bundled_minilm_dir() {
                    println!("Bundled MiniLM found at {} — warming…", dir.display());
                }
            } else {
                println!(
                    "No bundled MiniLM — fetching {} via fastembed (~50–80 MB)…",
                    cfg.model.as_str()
                );
                println!("Tip: run scripts/fetch-minilm-model.sh to bundle weights in-repo.");
            }
        }
        let started = Instant::now();
        neuromesh_embed::Embedder::prefetch_model(cfg, !quiet)
            .map_err(|e| NeuroMeshError::Internal(format!("embedding prefetch failed: {e}")))?;
        if !quiet {
            println!("MiniLM ready ({} ms).", started.elapsed().as_millis());
        }
        Ok(())
    }
}

fn rebuild(args: &[String]) -> Result<()> {
    #[cfg(not(feature = "embeddings"))]
    {
        return Err(NeuroMeshError::Config(
            "embed rebuild requires embeddings feature (reinstall release binary)".into(),
        ));
    }
    #[cfg(feature = "embeddings")]
    {
        let quiet = args.iter().any(|a| a == "--quiet" || a == "-q");
        let current_dir = neuromesh_index::assert_safe_workspace(&std::env::current_dir()?)?;
        let cfg = Config::load().embeddings;
        if !cfg.enabled {
            return Err(NeuroMeshError::Config(
                "embeddings disabled — enable in nm.config.json".into(),
            ));
        }
        let project_name = current_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");
        let project_id = neuromesh_core::ProjectId::new(project_name);
        let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
        let _ = graph.load_persisted(&current_dir);
        if graph.stats().total_nodes == 0 {
            let walker = neuromesh_index::ProjectWalker::new(current_dir.clone(), project_id);
            let report = walker.scan_report_with(&graph.file_fingerprints())?;
            graph.ingest_scan_report(&report);
        }
        if !quiet {
            println!("Rebuilding embeddings sidecar (incremental when possible)…");
        }
        let started = Instant::now();
        neuromesh_graph::rebuild_embeddings_for_workspace(&graph, &current_dir, &cfg)?;
        if !quiet {
            let idx = graph.embedding_index();
            if idx.is_hierarchical() {
                println!(
                    "Embeddings ready: sidecar v6 — {} files, {} symbols (lazy tier-1) × {} dims ({} ms)",
                    idx.file_count(),
                    idx.symbol_count(),
                    idx.dim,
                    started.elapsed().as_millis()
                );
            } else {
                println!(
                    "Embeddings ready: {} symbols × {} dims ({} ms)",
                    idx.symbol_count(),
                    idx.dim,
                    started.elapsed().as_millis()
                );
            }
        }
        Ok(())
    }
}

fn print_help() {
    println!("\nUsage:");
    println!("  neuromesh embed prefetch [--quiet]   Warm MiniLM weights");
    println!("  neuromesh embed rebuild [--quiet]    Build sidecar (hybrid: file tier + lazy symbols; deep: all symbols)");
    println!("\n  Default index is graph-only; run embed rebuild after first index.\n");
}
