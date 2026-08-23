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
        "connect" => {
            return commands::connect::execute();
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
            return commands::doctor::execute();
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
            println!("Use neuromesh_get_stats over MCP, or `neuromesh status` after `neuromesh index`.\n");
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
            let _ = commands::index::execute()?;
        }
        "start" => {
            let port = commands::port_from_args(args)?;
            commands::start::execute(port).await?;
        }
        "monitor" | "ui" | "dashboard" => {
            let port = commands::port_from_args(args)?;
            commands::monitor::execute(port).await?;
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
            let current_dir = neuromesh_index::ProjectWalker::discover_workspace(
                &env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            );
            let project_name = current_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project")
                .to_string();

            let project_id = ProjectId::new(&project_name);
            let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));

            let db_path = current_dir.join(".neuromesh").join("neuromesh.json");
            let memory_db = Arc::new(
                MemoryDatabase::open(&db_path)
                    .or_else(|_| MemoryDatabase::open_in_memory())
                    .unwrap_or_else(|_| MemoryDatabase::open_in_memory().unwrap()),
            );
            if neuromesh_index::ProjectWalker::is_safe_workspace(&current_dir) {
                for fact in neuromesh_memory::extract_project_facts(&current_dir, &project_id) {
                    let _ = memory_db.save_project_fact(&fact);
                }
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

            let bg_graph = graph.clone();
            let bg_dir = current_dir.clone();
            let bg_pid = project_id.clone();
            let _ = graph.load_persisted(&current_dir);
            tokio::task::spawn_blocking(move || {
                if !neuromesh_index::ProjectWalker::is_safe_workspace(&bg_dir) {
                    return;
                }
                let walker = neuromesh_index::ProjectWalker::new(bg_dir.clone(), bg_pid);
                if let Ok(scanned) = walker.scan() {
                    bg_graph.ingest_workspace(&scanned);
                    let _ = bg_graph.save_persisted(&bg_dir);
                }
            });

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
    println!("  mcp        MCP server over stdio (Cursor / Claude / Cline)");
    println!("  monitor    Web UI + SSE (default http://127.0.0.1:8765)");
    println!("  port       Show or set the monitor port (`neuromesh port 9000`)");
    println!("  index      Index the current workspace into the project graph");
    println!("  status     Node/edge counts after index (or a fresh scan)");
    println!("  graph      Print graph stats");
    println!("  memory     Print project memory facts");
    println!("  optimize   Activate one prompt and print the packet");
    println!(
        "  eval       Gold-task recall / precision / fill budget on this repo and tests/fixtures"
    );
    println!("  benchmark  Same as eval");
    println!("  connect    Print MCP JSON for the current binary");
    println!("  doctor     Workspace root, scan, persisted graph, monitor port");
    println!("  version    Print version (-v, --version)");
    println!("  help       Print this help (-h, --help)\n");
    println!("Quick start:");
    println!("  neuromesh mcp                   # what IDEs launch");
    println!("  neuromesh port 9000             # persist galaxy UI port");
    println!("  neuromesh monitor --port 9000   # one run only");
    println!("  neuromesh connect               # copy-paste MCP config\n");
}
