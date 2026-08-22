use neuromesh_api::{AppState, HttpServer};
use neuromesh_core::{Config, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_memory::MemoryDatabase;
use neuromesh_parser::CodeIntelligenceEngine;
use neuromesh_provider::ProviderFactory;
use std::fs;
use std::sync::Arc;

pub async fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let local_config_path = current_dir.join(".neuromesh").join("config.json");
    let home_config_path = dirs::home_dir().map(|h| h.join(".neuromesh").join("config.json"));

    let config = if local_config_path.exists() {
        let content = fs::read_to_string(&local_config_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else if let Some(hp) = home_config_path.filter(|p| p.exists()) {
        let content = fs::read_to_string(&hp)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Config::default()
    };

    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let project_id = ProjectId::new(&project_name);
    let walker = ProjectWalker::new(current_dir.clone(), project_id.clone());
    let scanned = walker.scan().unwrap_or_default();

    let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
    for (file, content) in &scanned {
        let ast = CodeIntelligenceEngine::analyze(&file.relative_path, content, file.language);
        graph.ingest_ast(file, &ast);
    }
    graph.finalize_links();

    let db_path = current_dir.join(".neuromesh").join("neuromesh.json");
    let memory_db = Arc::new(MemoryDatabase::open(&db_path)?);
    let provider = ProviderFactory::create(&config.provider);

    let state = AppState::new(config, graph, memory_db, provider);
    let server = HttpServer::new(state);

    println!("🧠 Starting NeuroMesh V1 Universal Context Gateway...");
    server.run().await?;

    Ok(())
}
