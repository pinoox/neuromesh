use neuromesh_core::{
    current_project_store, leftover_workspace_dotdir, neuromesh_home, project_data_dir,
    trust_workspace_local, untrust_workspace_local, uses_local_dotdir, Result,
};

pub fn execute(arg: Option<&str>) -> Result<()> {
    match arg {
        None | Some("get") | Some("status") | Some("-v") | Some("--show") => print_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
            Ok(())
        }
        Some("managed") | Some("global") | Some("home") => set_managed(),
        Some("local") | Some("project") | Some("workspace") => set_local(),
        Some(other) => Err(neuromesh_core::NeuroMeshError::Config(format!(
            "unknown store command: {other} (use managed or local)"
        ))),
    }
}

fn print_status() -> Result<()> {
    let ws = std::env::current_dir()?;
    let global = current_project_store();
    let local = uses_local_dotdir(&ws);
    let dir = project_data_dir(&ws);
    println!("\nNeuroMesh store");
    println!("Global default : {global}  (~/.neuromesh/config.json project_store)");
    println!(
        "This workspace : {}",
        if local {
            "local — <workspace>/.neuromesh is trusted"
        } else {
            "managed — data is not written into the repo"
        }
    );
    println!("Data directory : {}", dir.display());
    println!("Home           : {}", neuromesh_home().display());
    if let Some(left) = leftover_workspace_dotdir(&ws) {
        println!(
            "Leftover       : {} (ignored unless trusted)",
            left.display()
        );
    }
    println!("Change with    : neuromesh store local | neuromesh store managed");
    println!("Env            : NEUROMESH_STORE=local|managed");
    println!();
    Ok(())
}

fn set_local() -> Result<()> {
    let ws = std::env::current_dir()?;
    let dir = trust_workspace_local(&ws)?;
    println!("Trusted local store for this workspace");
    println!("Directory : {}", dir.display());
    println!("Graph and memory now live in the repo. Other projects stay managed.");
    Ok(())
}

fn set_managed() -> Result<()> {
    let ws = std::env::current_dir()?;
    let dir = untrust_workspace_local(&ws)?;
    println!("Managed store (default)");
    println!("Directory : {}", dir.display());
    println!("Workspace .neuromesh is not trusted. Leftover folders are ignored.");
    Ok(())
}

fn print_help() {
    println!(
        "\
Usage: neuromesh store [managed|local]

Where graph.bin, memory, and per-project config live.

  Default is managed: ~/.neuromesh/projects/<name>-<hash>/
  A <workspace>/.neuromesh folder is not read or written.

  neuromesh store              print mode and data directory
  neuromesh store managed      keep data in ~/.neuromesh (untrust this repo)
  neuromesh store local        trust <cwd>/.neuromesh for this workspace only

Settings: ~/.neuromesh/config.json  project_store / trust_local
Env     : NEUROMESH_STORE=local|managed   one-shot override
"
    );
}
