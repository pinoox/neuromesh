use neuromesh_core::{Config, NeuroMeshError, Result};
use std::time::Instant;

pub fn execute(args: &[String]) -> Result<()> {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("help");
    match sub {
        "prefetch" | "download" | "warm" => prefetch(args),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(NeuroMeshError::Config(format!(
            "unknown embed subcommand: {other} (use: embed prefetch)"
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

fn print_help() {
    println!("\nUsage: neuromesh embed prefetch [--quiet]");
    println!("\n  prefetch   Warm MiniLM (bundled weights preferred; HF download as fallback)");
    println!("  --quiet    Suppress progress output\n");
}
