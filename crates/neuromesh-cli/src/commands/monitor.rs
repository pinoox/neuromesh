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
    println!("NeuroMesh monitor: starting in {}", current_dir.display());
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
    if graph.load_persisted(&current_dir) {
        let stats = graph.stats();
        println!(
            "Loaded persisted graph: {} nodes · {} edges",
            stats.total_nodes, stats.total_edges
        );
    }

    let db_path = neuromesh_core::memory_db_path(&current_dir);
    let memory_db = Arc::new(
        MemoryDatabase::open(&db_path)
            .or_else(|_| MemoryDatabase::open_in_memory())
            .unwrap_or_else(|_| MemoryDatabase::open_in_memory().unwrap()),
    );
    let provider = ProviderFactory::create(&config.provider);

    let bg_graph = graph.clone();
    let bg_dir = current_dir.clone();
    let bg_pid = project_id.clone();
    tokio::task::spawn_blocking(move || {
        if !neuromesh_index::ProjectWalker::is_safe_workspace(&bg_dir) {
            eprintln!("NeuroMesh monitor: refused to index an unsafe workspace root");
            return;
        }
        let walker = configured_walker(bg_dir.clone(), bg_pid, cap);
        match walker.scan() {
            Ok(scanned) => {
                bg_graph.ingest_workspace(&scanned);
                let _ = bg_graph.save_persisted(&bg_dir);
                let stats = bg_graph.stats();
                println!(
                    "Indexed {} files · {} nodes · {} edges · {} workspace tokens",
                    scanned.len(),
                    stats.total_nodes,
                    stats.total_edges,
                    bg_graph.total_tokens()
                );
            }
            Err(e) => eprintln!("NeuroMesh monitor: index failed: {e}"),
        }
    });

    let state = AppState::new(config, graph, memory_db, provider);
    let server = HttpServer::new(state);

    server.run().await?;

    Ok(())
}
