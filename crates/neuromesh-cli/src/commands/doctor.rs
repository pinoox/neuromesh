use neuromesh_core::{Config, Result};
use neuromesh_index::ProjectWalker;
use std::env;
use std::net::TcpListener;

pub fn execute() -> Result<()> {
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

    let cwd = env::current_dir()?;
    let root = ProjectWalker::discover_workspace(&cwd);
    println!("Workspace      : {}", root.display());
    if !ProjectWalker::is_safe_workspace(&root) {
        println!("Safety         : refused (home or drive root)");
        return Ok(());
    }
    println!("Safety         : ok");

    let walker = ProjectWalker::new(root.clone(), neuromesh_core::ProjectId::new("doctor"));
    match walker.scan_report() {
        Ok(report) => {
            println!("Scan           : {} source files", report.files.len());
            if report.truncated {
                println!(
                    "Truncated      : hit {}-file cap; {} more files not indexed (test trees queued last)",
                    report.file_cap, report.omitted_over_cap
                );
            }
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
