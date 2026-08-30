mod commands;

use neuromesh_core::{Config, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_graph_proxy::{resolve_mcp_launch_spec, GraphProxySession};
use neuromesh_memory::MemoryDatabase;
use std::env;
use std::sync::Arc;

fn program_name() -> String {
    env::args()
        .next()
        .and_then(|a| {
            std::path::Path::new(&a)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "neuromesh".into())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("list");

    // Fast-path synchronous execution (instant 0ms response)
    match command {
        "-v" | "--version" | "version" | "-V" => {
            println!(
                "NeuroMesh v{} — local MCP context engine (alias: nmx)",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(());
        }
        "list" | "" | "help" | "-h" | "--help" => {
            print_help();
            return Ok(());
        }
        "status" | "stats" => {
            return commands::status::execute();
        }
        "usage" | "telemetry" => {
            return commands::usage::execute(&args);
        }
        "store" => {
            return commands::store::execute(args.get(2).map(|s| s.as_str()));
        }
        "config" => {
            return commands::config::execute(&args);
        }
        "connect" => {
            return commands::connect::execute(&args);
        }
        "port" => {
            return commands::port::execute(args.get(2).map(|s| s.as_str()));
        }
        "init" => {
            return commands::init::execute();
        }
        "graph" => {
            return commands::graph::execute();
        }
        "memory" => {
            return commands::memory::execute();
        }
        "doctor" => {
            let cap = commands::max_files_from_args(&args)?;
            return commands::doctor::execute(&args, cap);
        }
        "smoke" => {
            return commands::smoke::execute();
        }
        "packet" | "get_context_packet" => {
            return commands::packet::execute(&args[2..]);
        }
        "models" => {
            return commands::models::execute();
        }
        "projects" => {
            let current = env::current_dir()?;
            let name = current
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "default".to_string());
            println!("\nRegistered projects (this cwd):");
            println!("  • {} ({})\n", name, current.display());
            return Ok(());
        }
        "logs" => {
            return commands::usage::execute(&args);
        }
        "stop" => {
            let snap = commands::snapshot::collect_from_cwd(false)?;
            if snap.monitor_reachable {
                println!(
                    "\nMonitor is still running at {} — stop the neuromesh monitor process in your terminal.\n",
                    snap.monitor_url
                );
            } else {
                println!("\nNo monitor listening on {}.\n", snap.monitor_url);
            }
            return Ok(());
        }
        _ => {}
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main(command, &args))
}

async fn async_main(command: &str, args: &[String]) -> Result<()> {
    match command {
        "index" => {
            let cap = commands::max_files_from_args(args)?;
            let _ = commands::index::execute(cap)?;
        }
        "start" => {
            eprintln!(
                "Note: `start` is an alias for `monitor` (prefer `{} monitor`).",
                program_name()
            );
            let port = commands::port_from_args(args)?;
            let cap = commands::max_files_from_args(args)?;
            commands::monitor::execute(port, cap).await?;
        }
        "monitor" | "ui" | "dashboard" => {
            let port = commands::port_from_args(args)?;
            let cap = commands::max_files_from_args(args)?;
            commands::monitor::execute(port, cap).await?;
        }
        "optimize" => {
            let prompt = args.get(2).cloned();
            commands::optimize::execute(prompt)?;
        }
        "eval" | "evaluate" => commands::evaluate::execute(args)?,
        "benchmark" => commands::benchmark::execute()?,
        "mcp" => {
            // Handshake over stdio must start immediately. Index on a blocking
            // pool thread so we never starve stdin/stdout worker threads.
            eprintln!("NeuroMesh MCP listening on stdio");
            let target_dir = args.get(2).map(std::path::PathBuf::from).or_else(|| {
                std::env::var("NEUROMESH_WORKSPACE")
                    .ok()
                    .map(std::path::PathBuf::from)
            });
            let explicit = target_dir.is_some();
            let current_dir = if explicit {
                neuromesh_index::ProjectWalker::explicit_workspace(target_dir.as_ref().unwrap())
            } else {
                neuromesh_index::resolve_mcp_startup_workspace()
            };
            eprintln!("NeuroMesh MCP workspace: {}", current_dir.display());
            let project_name = current_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project")
                .to_string();

            let project_id = ProjectId::new(&project_name);
            let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
            graph.set_workspace(&current_dir);

            let db_path = neuromesh_core::memory_db_path(&current_dir);
            let memory_db = Arc::new(
                MemoryDatabase::open(&db_path)
                    .or_else(|_| MemoryDatabase::open_in_memory())
                    .unwrap_or_else(|_| MemoryDatabase::open_in_memory().unwrap()),
            );
            if neuromesh_index::ProjectWalker::is_safe_workspace(&current_dir) {
                for fact in neuromesh_memory::extract_project_facts(&current_dir, &project_id) {
                    let _ = memory_db.save_project_fact(&fact);
                }
            } else {
                graph.mark_index_ready();
            }

            let registry = Arc::new(neuromesh_context::ReversibleContextRegistry::new());
            let activator = Arc::new(neuromesh_context::ContextActivator::new(registry.clone()));
            let expansion_engine = Arc::new(neuromesh_context::ExpansionEngine::new(registry));
            let working_memory = Arc::new(parking_lot::RwLock::new(
                neuromesh_memory::WorkingMemory::default(),
            ));

            let cap = commands::max_files_from_args(args)?;
            let _ = graph.load_persisted(&current_dir);
            let cfg = Config::load();
            #[cfg(feature = "embeddings")]
            if cfg.embeddings.enabled {
                let emb = cfg.embeddings.clone();
                std::thread::spawn(move || {
                    if let Err(e) = neuromesh_embed::Embedder::warm(emb) {
                        eprintln!("NeuroMesh embed warm-up: {e}");
                    }
                });
            }
            let _ = neuromesh_mcp::warmup_project_learning(
                memory_db.as_ref(),
                graph.as_ref(),
                &project_id,
            );

            let handler = Arc::new({
                let mut h = neuromesh_mcp::McpToolHandler::new(
                    graph.clone(),
                    activator,
                    expansion_engine,
                    memory_db,
                    working_memory,
                );
                let cfg = Config::load();
                if let Some(spec) = resolve_mcp_launch_spec(&cfg.graph_backend, &current_dir) {
                    match GraphProxySession::connect(spec.clone(), &current_dir).await {
                        Ok(session) => {
                            eprintln!(
                                "NeuroMesh graph backend: {} ({} — {})",
                                cfg.graph_backend.backend.as_str(),
                                spec.provider.as_str(),
                                spec.command
                            );
                            h = h.with_graph_proxy(
                                session,
                                cfg.graph_backend.fallback_native,
                                cfg.graph_backend.backend.as_str(),
                            );
                        }
                        Err(e) if cfg.graph_backend.fallback_native => {
                            eprintln!(
                                "NeuroMesh graph proxy unavailable ({e}); using native graph"
                            );
                        }
                        Err(e) => {
                            eprintln!("NeuroMesh graph proxy failed: {e}");
                        }
                    }
                }
                h
            });
            if graph.stats().total_nodes == 0
                && neuromesh_index::ProjectWalker::is_safe_workspace(&current_dir)
            {
                graph.mark_index_loading();
            }
            commands::spawn_live_sync(graph.clone(), current_dir.clone(), project_id.clone(), cap);

            let server = neuromesh_mcp::McpServer::new(handler);
            server.run_stdio().await?;
        }
        "--help" | "-h" | "help" => {
            print_help();
        }
        unknown => {
            println!("Unknown command: {}", unknown);
            print_help();
        }
    }

    Ok(())
}

fn print_help() {
    let bin = program_name();
    println!(
        "\nNeuroMesh v{} — local MCP context engine",
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage: {bin} <COMMAND> [OPTIONS]  (alias: nmx)\n");
    println!("Commands:");
    println!("  mcp        MCP server over stdio (Cursor, Codex, Antigravity, …)");
    println!("  connect    Install MCP configs (`--global`, `--agent-rules`, `--print`)");
    println!("  smoke      Quick get_context + graph/telemetry check");
    println!("  monitor    Web UI + SSE (aliases: ui, dashboard; start is deprecated)");
    println!("  port       Show or set the monitor port (`{bin} port 9000`)");
    println!("  index      Index the current workspace into the project graph");
    println!("  status     Unified workspace + graph + telemetry snapshot");
    println!("  usage      MCP/CLI token telemetry (`--all`, `--limit N`; alias: telemetry, logs)");
    println!("  store      Where project data lives (managed home vs trusted local)");
    println!("  config     Seed engine + settings (global or nm.config.json per project)");
    println!("  graph      Print graph stats");
    println!("  memory     Print project memory facts");
    println!("  optimize   Activate one prompt and print the packet");
    println!("  packet     JSON packet for benchmarks (`--json`, `--engine`, `--keywords`)");
    println!(
        "  eval       Gold-task recall / precision / fill budget (alias: evaluate, benchmark)"
    );
    println!("  doctor     Workspace root, scan, MCP/proxy/embed (`--mcp`, `--proxy`, `--embed`, `--bench`)");
    println!("  init       Ensure NeuroMesh data directories exist");
    println!("  models     List configured / local AI models");
    println!("  version    Print version (-v, --version)");
    println!("  help       Print this help (-h, --help)\n");
    println!("Quick start:");
    println!("  {bin} connect --global --agent-rules   # once per machine");
    println!("  {bin} smoke                            # verify this repo");
    println!("  {bin} monitor --port 9000              # galaxy UI");
    println!("  {bin} index --max-files auto");
    println!();
    println!("Index file cap:");
    println!("  Default is auto: every production source, then tests, up to 50,000.");
    println!("  {bin} index --max-files 20000   persist a limit");
    println!("  {bin} index --max-files auto    persist auto (or --max-files=auto)");
    println!("  NEUROMESH_MAX_FILES=20000           env override (auto / 0 = auto)");
}
