use neuromesh_core::{Config, Result};
use std::fs;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let neuromesh_dir = current_dir.join(".neuromesh");

    if neuromesh_dir.exists() {
        println!("✓ NeuroMesh already initialized in this project.");
        return Ok(());
    }

    fs::create_dir_all(&neuromesh_dir)?;
    let config = Config::default();
    let config_json = serde_json::to_string_pretty(&config)?;
    fs::write(neuromesh_dir.join("config.json"), config_json)?;

    println!("🧠 NeuroMesh V1 Initialized Successfully");
    println!("  Directory: {}", neuromesh_dir.display());
    println!("  Configuration: config.json created");
    println!("  Next step: run 'neuromesh index' to index your repository.");

    Ok(())
}
