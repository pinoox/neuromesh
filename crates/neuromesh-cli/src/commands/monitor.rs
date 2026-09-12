use neuromesh_api::{AppState, HttpServer};
use neuromesh_core::{Config, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::MemoryDatabase;
use neuromesh_provider::ProviderFactory;
use std::io::Write;
use std::sync::Arc;

use super::{apply_file_cap, FileCapArg};

pub async fn execute(port_override: Option<u16>, cap: FileCapArg) -> Result<()> {
    let current_dir = neuromesh_index::assert_safe_workspace(&std::env::current_dir()?)?;
    println!("NeuroMesh monitor: starting in {}", current_dir.display());
    let _ = std::io::stdout().flush();

    let mut config = Config::load();
    if let Some(port) = port_override {
        config = config.with_port(port);
    }
    config = apply_file_cap(config, cap);

    let project_id = neuromesh_core::stable_project_id(&current_dir);
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
    // `monitor` has no workspace argument; the root is the current directory,
    // so it is a guess and has to look like a project.
    super::spawn_live_sync(bg_graph, bg_dir, bg_pid, cap, false);

    let state = AppState::new(config, graph, memory_db, provider);
    *state.workspace_path.write() = current_dir.clone();
    state.attach_graph_proxy_if_configured().await;
    let server = HttpServer::new(state);

    server.run().await?;

    Ok(())
}
