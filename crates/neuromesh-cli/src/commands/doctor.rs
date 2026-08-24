use neuromesh_core::{Config, Result};
use neuromesh_index::ProjectWalker;
use std::env;
use std::net::TcpListener;

use super::{configured_walker, print_file_cap, FileCapArg};

pub fn execute(cap: FileCapArg) -> Result<()> {
    println!("\nNeuroMesh doctor");
    println!(
        "OS             : {} ({})",
        env::consts::OS,
        env::consts::ARCH
    );
    println!("Version        : {}", env!("CARGO_PKG_VERSION"));

    let cfg = Config::load();
    let addr = format!("{}:{}", cfg.host, cfg.port);
    match TcpListener::bind(&addr) {
        Ok(_) => println!("Monitor port   : {addr} available"),
        Err(_) => println!("Monitor port   : {addr} in use"),
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

    let graph_path = root.join(".neuromesh").join("graph.json");
    println!(
        "Persisted graph: {}",
        if graph_path.exists() {
            "present"
        } else {
            "missing (run neuromesh index)"
        }
    );
    println!();
    Ok(())
}
