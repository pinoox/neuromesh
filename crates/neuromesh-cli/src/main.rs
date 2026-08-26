mod commands;

use neuromesh_core::{ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::MemoryDatabase;
use std::env;
use std::sync::Arc;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("list");

    // Fast-path synchronous execution (instant 0ms response)
    match command {
        "-v" | "--version" | "version" | "-V" => {
            println!(
                "NeuroMesh v{} — local MCP context engine",
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
            return commands::doctor::execute(cap);
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
            println!("\n📁 Registered Projects:");
            println!("  • {} ({})\n", name, current.display());
            return Ok(());
        }
        "logs" => {
            println!("\nNo durable audit log is written.");
            println!("Use `neuromesh usage` for MCP token telemetry, or `neuromesh status` after `neuromesh index`.\n");
            return Ok(());
        }
        "stop" => {
            println!("✓ NeuroMesh server stopped.");
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
            let port = commands::port_from_args(args)?;
            let cap = commands::max_files_from_args(args)?;
            commands::start::execute(port, cap).await?;
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
        "eval" | "evaluate" => commands::evaluate::execute()?,
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
            let target_dir = target_dir.unwrap_or_else(|| {
                env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

            let current_dir = if explicit {
                neuromesh_index::ProjectWalker::explicit_workspace(&target_dir)
            } else {
                neuromesh_index::ProjectWalker::discover_workspace(&target_dir)
            };
            let project_name = current_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project")
                .to_string();

            let project_id = ProjectId::new(&project_name);
            let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));

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

            let handler = Arc::new(neuromesh_mcp::McpToolHandler::new(
                graph.clone(),
                activator,
                expansion_engine,
                memory_db,
                working_memory,
            ));

            let cap = commands::max_files_from_args(args)?;
            let _ = graph.load_persisted(&current_dir);
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
    println!(
        "\nNeuroMesh v{} — local MCP context engine",
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage: neuromesh <COMMAND> [OPTIONS]\n");
    println!("Commands:");
    println!("  mcp        MCP server over stdio (Cursor, Codex, Antigravity, …)");
    println!("  monitor    Web UI + SSE (default http://127.0.0.1:8765)");
    println!("  port       Show or set the monitor port (`neuromesh port 9000`)");
    println!("  index      Index the current workspace into the project graph");
    println!("  status     Node/edge counts after index (or a fresh scan)");
    println!("  usage      MCP token telemetry (`--all`, `--limit N`)");
    println!("  store      Where project data lives (managed home vs trusted local)");
    println!("  graph      Print graph stats");
    println!("  memory     Print project memory facts");
    println!("  optimize   Activate one prompt and print the packet");
    println!(
        "  eval       Gold-task recall / precision / fill budget on this repo and tests/fixtures"
    );
    println!("  benchmark  Same as eval");
    println!("  connect    Install MCP configs (or `--print` snippets)");
    println!("  doctor     Workspace root, scan, persisted graph, monitor port");
    println!("  version    Print version (-v, --version)");
    println!("  help       Print this help (-h, --help)\n");
    println!("Quick start:");
    println!("  neuromesh mcp                   # what IDEs launch");
    println!("  neuromesh port 9000             # persist galaxy UI port");
    println!("  neuromesh monitor --port 9000   # one run only");
    println!("  neuromesh index --max-files auto");
    println!("  neuromesh connect               # write MCP configs for this repo\n");
    println!("Index file cap:");
    println!("  Default is auto: every production source, then tests, up to 50,000.");
    println!("  neuromesh index --max-files 20000   persist a limit");
    println!("  neuromesh index --max-files auto    persist auto (or --max-files=auto)");
    println!("  NEUROMESH_MAX_FILES=20000           env override (auto / 0 = auto)");
}
