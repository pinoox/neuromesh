mod commands;

use neuromesh_api::{AppState, HttpServer};
use neuromesh_core::{Config, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::MemoryDatabase;
use neuromesh_provider::ProviderFactory;
use std::env;
use std::sync::Arc;

/// Best-effort: open the system's default browser. Never fatal — the caller
/// keeps working (stdio MCP, or the dashboard itself) if this fails.
fn open_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if let Err(e) = result {
        eprintln!("NeuroMesh: could not open browser automatically ({e}); open {url} manually.");
    }
}

/// The `mcp` process is spawned silently and often respawned by the MCP
/// client (Claude Desktop, Cursor, …), so auto-opening a tab on every launch
/// would spam the user. Open the dashboard automatically only the very first
/// time NeuroMesh ever starts on this machine; after that, `neuromesh monitor`
/// opens it on demand.
fn maybe_open_ui_first_run(port: u16) {
    let marker = neuromesh_core::neuromesh_home().join(".ui_opened_once");
    if marker.exists() {
        return;
    }
    if std::fs::create_dir_all(neuromesh_core::neuromesh_home()).is_err() {
        return;
    }
    if std::fs::write(&marker, b"1").is_ok() {
        open_browser(&format!("http://127.0.0.1:{port}"));
    }
}

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
        "install" => {
            return commands::install::execute(&args[1..]);
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
        "embed" => {
            return commands::embed::execute(&args);
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
            let current = neuromesh_core::strip_verbatim_prefix(&current);
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
            let _ = commands::index::execute(cap, args)?;
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
            let current_dir = neuromesh_core::strip_verbatim_prefix(&current_dir);
            eprintln!("NeuroMesh MCP workspace: {}", current_dir.display());
            // Derived from the canonical project path, not the directory name:
            // unrelated checkouts both called `app` used to share one identity.
            let project_id = neuromesh_core::stable_project_id(&current_dir);
            let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
            graph.set_workspace(&current_dir);

            // A guessed cwd that is not a project (IDE spawn under AppData\Local,
            // home, etc.) must not walk the tree or touch a persistent store —
            // that delayed `initialize` until the client canceled the context.
            let rejection = if explicit {
                neuromesh_index::ProjectWalker::workspace_rejection_reason(&current_dir)
            } else {
                neuromesh_index::ProjectWalker::discovered_workspace_rejection_reason(&current_dir)
            };
            let indexable = rejection.is_none();
            if let Some(reason) = rejection.as_ref() {
                eprintln!("NeuroMesh will not index this workspace: {reason}");
                eprintln!(
                    "NeuroMesh: pass a project path (`neuromesh mcp <path>`) or set NEUROMESH_WORKSPACE"
                );
                graph.mark_index_ready();
            }

            let memory_db = if indexable {
                let db_path = neuromesh_core::memory_db_path(&current_dir);
                Arc::new(
                    MemoryDatabase::open(&db_path)
                        .or_else(|_| MemoryDatabase::open_in_memory())
                        .unwrap_or_else(|_| MemoryDatabase::open_in_memory().unwrap()),
                )
            } else {
                Arc::new(MemoryDatabase::open_in_memory().unwrap_or_else(|_| {
                    MemoryDatabase::open_in_memory().expect("in-memory NeuroMesh store")
                }))
            };

            if indexable {
                // Facts are nice-to-have telemetry; never block stdio on a walk.
                let facts_dir = current_dir.clone();
                let facts_pid = project_id.clone();
                let facts_db = memory_db.clone();
                std::thread::spawn(move || {
                    for fact in neuromesh_memory::extract_project_facts(&facts_dir, &facts_pid) {
                        let _ = facts_db.save_project_fact(&fact);
                    }
                });
            }

            let cap = commands::max_files_from_args(args)?;
            if indexable {
                let _ = graph.load_persisted(&current_dir);
            }
            let cfg = Config::load();
            #[cfg(feature = "embeddings")]
            if indexable
                && cfg.retrieval.engine != neuromesh_core::RetrievalEngine::Fast
                && cfg.embeddings.enabled
            {
                let emb = cfg.embeddings.clone();
                let graph_bg = graph.clone();
                let dir_bg = current_dir.clone();
                std::thread::spawn(move || {
                    let sidecar = neuromesh_core::embeddings_path(&dir_bg);
                    if sidecar.exists() {
                        if let Err(e) = neuromesh_embed::Embedder::warm(emb) {
                            eprintln!("NeuroMesh embed warm-up: {e}");
                        }
                        let _ = graph_bg.load_embedding_sidecar(&dir_bg);
                    } else {
                        eprintln!(
                            "NeuroMesh: building embeddings in background (lexical fallback until ready)…"
                        );
                        if let Err(e) = neuromesh_graph::rebuild_embeddings_for_workspace(
                            &graph_bg, &dir_bg, &emb,
                        ) {
                            eprintln!("NeuroMesh embed rebuild: {e}");
                        }
                    }
                });
            }
            if indexable {
                let _ = neuromesh_mcp::warmup_project_learning(
                    memory_db.as_ref(),
                    graph.as_ref(),
                    &project_id,
                );
            }

            // Single shared state for both the stdio MCP handler and the local
            // dashboard: one Config lock, one McpToolHandler, one graph — so a
            // setting saved from the dashboard (mode, retrieval engine, graph
            // backend) applies to this same live session, not a separate one.
            let provider = ProviderFactory::create(&cfg.provider);
            let state = AppState::new(cfg, graph.clone(), memory_db, provider);
            if indexable {
                state.attach_graph_proxy_if_configured().await;
            }
            let handler = state.mcp_handler.clone();

            if indexable
                && graph.stats().total_nodes == 0
                && neuromesh_index::ProjectWalker::is_safe_workspace(&current_dir)
            {
                graph.mark_index_loading();
            }
            if indexable {
                commands::spawn_live_sync(
                    graph.clone(),
                    current_dir.clone(),
                    project_id.clone(),
                    cap,
                    explicit,
                );
            }

            // Local dashboard: best-effort only, and never blocks stdio
            // startup. `run_with_port_notify` walks forward to the next free
            // port if the configured one is already held (e.g. a standalone
            // `neuromesh monitor`, or another project's own `mcp` process) —
            // so a second or third simultaneously open project still gets a
            // working dashboard, just on a different port, instead of none.
            {
                let dash_state = state.clone();
                let (port_tx, port_rx) = tokio::sync::oneshot::channel();
                tokio::spawn(async move {
                    if let Err(e) = HttpServer::new(dash_state)
                        .run_with_port_notify(Some(port_tx))
                        .await
                    {
                        eprintln!(
                            "NeuroMesh dashboard not started ({e}); MCP continues over stdio."
                        );
                    }
                });
                // Separate task: only open a browser once we know the real
                // bound port, but this never delays the stdio server below.
                tokio::spawn(async move {
                    if let Ok(actual_port) = port_rx.await {
                        maybe_open_ui_first_run(actual_port);
                    }
                });
            }

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
    println!("  index      Index workspace (default engine=fast; --mode hybrid for embed)");
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
    println!("  doctor     Workspace root, scan, MCP/proxy/embed (`--mcp`, `--proxy`, `--embed`, `--bench`, `--quarantine-oversized`)");
    println!("  install    On-demand embed models (`install embed minilm`, `install embed list`)");
    println!("  embed      Warm installed MiniLM (`embed prefetch`, `embed rebuild`)");
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
