use neuromesh_core::{Config, GraphBackendId, Result, RetrievalEngine};
use neuromesh_graph_proxy::{detect_proxy_launch_specs, probe_graph_proxy};
use neuromesh_index::ProjectWalker;
use std::env;
use std::net::TcpListener;

use super::{configured_walker, print_file_cap, snapshot, FileCapArg};

pub fn execute(args: &[String], cap: FileCapArg) -> Result<()> {
    let mcp_diag = args.iter().any(|a| a == "--mcp");
    let proxy_diag = args.iter().any(|a| a == "--proxy" || a == "--probe");
    let embed_diag = args.iter().any(|a| a == "--embed");
    let embed_bench = args.iter().any(|a| a == "--bench");
    let probe_live = args.iter().any(|a| a == "--probe");
    println!("\nNeuroMesh doctor");
    println!(
        "OS             : {} ({})",
        env::consts::OS,
        env::consts::ARCH
    );
    println!("Version        : {}", env!("CARGO_PKG_VERSION"));
    println!(
        "CLI            : {} (alias: nmx)",
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "neuromesh".into())
    );

    let engine_diag = args.iter().any(|a| a == "--engine");
    let cfg = Config::load();
    println!(
        "Retrieval engine : {} (seed={}, embed={})",
        cfg.retrieval.engine.as_str(),
        cfg.seed_resolution.engine.as_str(),
        if cfg.embeddings.enabled { "on" } else { "off" }
    );
    if engine_diag {
        println!("  Mode preset    : {:?}", cfg.mode);
        println!(
            "  Auto keywords  : {}",
            cfg.seed_resolution.auto_extract_keywords
        );
        if cfg.retrieval.engine == RetrievalEngine::Fast {
            println!("  ONNX at MCP    : skipped (zero-embed fast engine)");
        }
    }
    println!(
        "Graph backend  : {} (fallback_native={})",
        cfg.graph_backend.backend.as_str(),
        cfg.graph_backend.fallback_native
    );

    let addr = format!("{}:{}", cfg.host, cfg.port);
    match TcpListener::bind(&addr) {
        Ok(_) => println!("Monitor port   : {addr} available"),
        Err(_) => println!("Monitor port   : {addr} in use (monitor may be running)"),
    }
    println!("Change with    : neuromesh port <n>  |  --port <n>  |  NEUROMESH_PORT");
    match cfg.max_files {
        Some(n) => println!("Max files      : {n} (explicit)"),
        None => println!("Max files      : auto (production sources, ceiling 50,000)"),
    }
    println!("Change with    : neuromesh index --max-files <n|auto>  |  NEUROMESH_MAX_FILES");

    let cwd = env::current_dir()?;
    let root = ProjectWalker::discover_workspace(&cwd);
    println!("Workspace      : {}", root.display());
    if !ProjectWalker::is_safe_workspace(&root) {
        println!("Safety         : refused (home or drive root)");
        return Ok(());
    }
    println!("Safety         : ok");

    if proxy_diag {
        println!("\nGraph proxy detection");
        let report = detect_proxy_launch_specs(&root);
        if report.candidates.is_empty() {
            println!("  No CBM/Graphify MCP servers found in IDE configs.");
            println!("  Install codebase-memory-mcp and add to ~/.cursor/mcp.json, then set:");
            println!("    neuromesh config graph-backend auto");
        } else {
            for (i, c) in report.candidates.iter().take(5).enumerate() {
                println!(
                    "  [{}] {} — {} {:?} (score {})",
                    i + 1,
                    c.spec.provider.as_str(),
                    c.spec.server_name,
                    c.spec.command,
                    c.score
                );
                if let Some(p) = &c.spec.config_path {
                    println!("       {}", p.display());
                }
            }
        }
        if let Some(spec) = neuromesh_graph_proxy::resolve_for_workspace(&cfg.graph_backend, &root)
        {
            println!(
                "\n  Effective for current config: {} via {}",
                spec.provider.as_str(),
                spec.command
            );
        } else if cfg.graph_backend.backend.uses_proxy() {
            println!("\n  Config requests proxy but no launch spec resolved — native at runtime.");
        }

        if probe_live {
            println!("\nGraph proxy live probe");
            let mut probe_cfg = cfg.graph_backend.clone();
            if probe_cfg.backend == GraphBackendId::Native {
                probe_cfg.backend = GraphBackendId::Auto;
                println!("  (backend is native — probing with auto-detect)");
            }
            let report = tokio::runtime::Runtime::new()
                .map_err(|e| neuromesh_core::NeuroMeshError::Config(e.to_string()))?
                .block_on(probe_graph_proxy(&probe_cfg, &root))?;
            if report.connected {
                println!("  Status         : connected");
                if let Some(p) = &report.provider {
                    println!("  Provider       : {p}");
                }
                if let Some(c) = &report.command {
                    println!("  Command        : {c}");
                }
                if !report.tools.is_empty() {
                    println!("  Tools          : {}", report.tools.join(", "));
                }
                if report.sample_files > 0 {
                    println!(
                        "  Sample packet  : {} files, {} tokens, coverage {}",
                        report.sample_files,
                        report.packet_tokens,
                        report.coverage.as_deref().unwrap_or("—")
                    );
                } else if let Some(err) = &report.error {
                    println!("  Sample packet  : (skipped — {err})");
                } else {
                    println!("  Sample packet  : 0 files");
                }
            } else {
                println!("  Status         : failed");
                if let Some(err) = &report.error {
                    println!("  Error          : {err}");
                }
            }
        }
    }

    if embed_diag && cfg.embeddings.enabled {
        let emb = cfg.embeddings.clone();
        println!("\nEmbedding engine");
        println!(
            "  Config         : {} ({}, dim {}, threads {:?})",
            if emb.enabled { "enabled" } else { "disabled" },
            emb.model.as_str(),
            emb.matryoshka_dim,
            emb.intra_threads
        );
        #[cfg(not(feature = "embeddings"))]
        println!("  Binary         : embeddings feature not compiled");
        #[cfg(feature = "embeddings")]
        {
            if neuromesh_embed::bundled_minilm_available() {
                if let Some(dir) = neuromesh_embed::resolve_bundled_minilm_dir() {
                    println!("  Model install  : ok ({})", dir.display());
                } else {
                    println!("  Model install  : ok");
                }
            } else {
                println!(
                    "  Model install  : missing ({})",
                    neuromesh_embed::install_hint()
                );
            }
            let sidecar = neuromesh_core::embeddings_path(&root);
            if sidecar.exists() {
                println!("  Sidecar        : {} (present)", sidecar.display());
                if let Ok(Some(sc)) = neuromesh_graph::load_sidecar(&sidecar) {
                    println!(
                        "  Vectors        : {} symbols × {} dims (gen {}, sidecar v{})",
                        sc.node_ids.len(),
                        sc.dim,
                        sc.graph_generation,
                        sc.version
                    );
                    if !sc.module_centroids.is_empty() {
                        println!(
                            "  Module clusters: {} directory centroids",
                            sc.module_centroids.len()
                        );
                    }
                }
            } else {
                println!("  Sidecar        : missing (run neuromesh index with embeddings on)");
            }
            if emb.enabled {
                if !neuromesh_embed::bundled_minilm_available() {
                    println!("  Warm load      : skipped (install model first)");
                } else {
                    let cold_start = std::time::Instant::now();
                    match neuromesh_embed::Embedder::warm(emb.clone()) {
                        Ok(()) => {
                            println!(
                                "  Warm load      : ok ({} ms, singleton)",
                                cold_start.elapsed().as_millis()
                            );
                            let sample_start = std::time::Instant::now();
                            match neuromesh_embed::embed_query_cached(
                                &emb,
                                "doctor probe middleware",
                            ) {
                                Ok(vec) => println!(
                                    "  Sample embed   : ok ({} dims, {} ms cached path)",
                                    vec.len(),
                                    sample_start.elapsed().as_millis()
                                ),
                                Err(e) => println!("  Sample embed   : failed ({e})"),
                            }
                        }
                        Err(e) => println!("  Model load     : failed ({e})"),
                    }
                    if embed_bench {
                        const N: usize = 20;
                        let mut samples = Vec::with_capacity(N);
                        for i in 0..N {
                            neuromesh_embed::packet_cache_begin();
                            let t0 = std::time::Instant::now();
                            let _ = neuromesh_embed::embed_query_cached(
                                &emb,
                                &format!("bench probe middleware route {i}"),
                            );
                            samples.push(t0.elapsed().as_millis() as u64);
                            neuromesh_embed::packet_cache_end();
                        }
                        samples.sort_unstable();
                        let p50 = samples[samples.len() / 2];
                        let p95 = samples[(samples.len() * 95) / 100];
                        println!("  Bench ({N} queries) : p50 {p50} ms, p95 {p95} ms");
                    }
                }
            }
        }
    } else if embed_diag {
        println!("\nEmbedding engine : disabled (engine=fast — `neuromesh config engine hybrid`)");
    }

    if mcp_diag {
        println!("\nMCP workspace detection");
        if let Some(env_line) = neuromesh_index::mcp_workspace_env_summary() {
            println!("  IDE env      : {env_line}");
        } else {
            println!("  IDE env      : (none — Cursor/VS Code may send root in initialize)");
        }
        let predicted = neuromesh_index::resolve_mcp_startup_workspace();
        println!("  Predicted    : {}", predicted.display());
        println!("  Portable MCP : {{ \"command\": \"neuromesh\", \"args\": [\"mcp\"] }}");
    }

    let walker = configured_walker(root.clone(), neuromesh_core::ProjectId::new("doctor"), cap);
    match walker.scan_report() {
        Ok(report) => {
            println!("Scan           : {} source files", report.files.len());
            print_file_cap(&report, "");
            if report.skipped_count() > 0 {
                println!(
                    "Skipped        : {} files ({})",
                    report.skipped_count(),
                    report.skipped_summary()
                );
            }
        }
        Err(e) => println!("Scan           : failed ({e})"),
    }

    let snap = snapshot::collect_from_cwd(false)?;
    println!(
        "Graph          : {} nodes / {} edges ({})",
        snap.graph_nodes,
        snap.graph_edges,
        if snap.graph_ready {
            "ready"
        } else {
            "indexing or empty"
        }
    );
    println!(
        "Telemetry      : {} requests | {:.1}% mean reduction",
        snap.telemetry_rows, snap.telemetry.mean_reduction_pct
    );
    println!("Monitor        : {}", snap.monitor_status);
    println!(
        "Data directory : {}",
        neuromesh_core::project_data_dir(&root).display()
    );
    println!("Store          : {}", snap.store_mode);
    println!(
        "Persisted graph: {}",
        if snap.persisted_graph {
            "present"
        } else {
            "missing (run neuromesh index)"
        }
    );
    if let Some(left) = neuromesh_core::leftover_workspace_dotdir(&root) {
        println!(
            "Leftover       : {} exists and is not trusted",
            left.display()
        );
        println!("Trust with     : neuromesh store local");
    }
    println!();
    Ok(())
}
