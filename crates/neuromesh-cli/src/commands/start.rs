use neuromesh_api::{AppState, HttpServer};
use neuromesh_core::{Config, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::MemoryDatabase;
use neuromesh_provider::ProviderFactory;
use std::io::Write;
use std::sync::Arc;

use super::{apply_file_cap, FileCapArg};

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
    super::spawn_live_sync(graph.clone(), current_dir.clone(), project_id.clone(), cap);

    let db_path = neuromesh_core::memory_db_path(&current_dir);
    let memory_db = Arc::new(MemoryDatabase::open(&db_path)?);
    let provider = ProviderFactory::create(&config.provider);

    let state = AppState::new(config, graph, memory_db, provider);
    let server = HttpServer::new(state);

    println!("🧠 Starting NeuroMesh V1 Universal Context Gateway...");
    server.run().await?;

    Ok(())
}
