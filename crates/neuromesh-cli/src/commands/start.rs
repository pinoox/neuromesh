use neuromesh_api::{AppState, HttpServer};
use neuromesh_core::{Config, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::MemoryDatabase;
use neuromesh_provider::ProviderFactory;
use std::io::Write;
use std::sync::Arc;

use super::{apply_file_cap, configured_walker, FileCapArg};

pub async fn execute(port_override: Option<u16>, cap: FileCapArg) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    println!("NeuroMesh gateway: starting in {}", current_dir.display());
    let _ = std::io::stdout().flush();

    let mut config = Config::load();
    if let Some(port) = port_override {
        config = config.with_port(port);
    }
    config = apply_file_cap(config, cap);

    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let project_id = ProjectId::new(&project_name);
    let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
    let _ = graph.load_persisted(&current_dir);

    let bg_graph = graph.clone();
    let bg_dir = current_dir.clone();
    let bg_pid = project_id.clone();
    tokio::task::spawn_blocking(move || {
        if !neuromesh_index::ProjectWalker::is_safe_workspace(&bg_dir) {
            return;
        }
        let walker = configured_walker(bg_dir.clone(), bg_pid, cap);
        if let Ok(scanned) = walker.scan() {
            bg_graph.ingest_workspace(&scanned);
            let _ = bg_graph.save_persisted(&bg_dir);
        }
    });

    let db_path = current_dir.join(".neuromesh").join("neuromesh.json");
    let memory_db = Arc::new(MemoryDatabase::open(&db_path)?);
    let provider = ProviderFactory::create(&config.provider);

    let state = AppState::new(config, graph, memory_db, provider);
    let server = HttpServer::new(state);

    println!("🧠 Starting NeuroMesh V1 Universal Context Gateway...");
    server.run().await?;

    Ok(())
}
