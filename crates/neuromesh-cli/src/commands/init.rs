use neuromesh_core::{
    ensure_project_data_dir, leftover_workspace_dotdir, neuromesh_home, uses_local_dotdir, Result,
};

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let dir = ensure_project_data_dir(&current_dir)?;
    let mode = if uses_local_dotdir(&current_dir) {
        "local (this workspace)"
    } else {
        "managed"
    };

    println!("✓ NeuroMesh data directory ready");
    println!("  Store     : {mode}");
    println!("  Directory : {}", dir.display());
    println!("  Home      : {}", neuromesh_home().display());
    if leftover_workspace_dotdir(&current_dir).is_some() {
        println!(
            "  Leftover  : {}/.neuromesh exists and is ignored",
            current_dir.display()
        );
        println!("             neuromesh store local   to trust it");
    }
    println!("  Next      : neuromesh index");
    Ok(())
}
