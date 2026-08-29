use neuromesh_core::{Config, Result};
use neuromesh_index::ProjectWalker;
use std::env;
use std::net::TcpListener;

use super::{configured_walker, print_file_cap, snapshot, FileCapArg};

pub fn execute(args: &[String], cap: FileCapArg) -> Result<()> {
    let mcp_diag = args.iter().any(|a| a == "--mcp");
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

    let cfg = Config::load();
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
