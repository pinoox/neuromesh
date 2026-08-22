use neuromesh_core::{ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_memory::MemoryDatabase;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project_id = ProjectId::new(&project_name);
    let walker = ProjectWalker::new(current_dir.clone(), project_id.clone());
    let scanned = walker.scan().unwrap_or_default();

    let graph = NeuralProjectGraph::new(project_id.clone());
    let loaded = graph.load_persisted(&current_dir);
    if !loaded {
        graph.ingest_workspace(&scanned);
    }
    let stats = graph.stats();

    let db_path = current_dir.join(".neuromesh").join("neuromesh.json");
    let memory_entries = if db_path.exists() {
        MemoryDatabase::open(&db_path)
            .ok()
            .and_then(|db| db.get_project_facts(&project_id).ok())
            .map(|f| f.len())
            .unwrap_or(0)
    } else {
        0
    };

    println!("\nNeuroMesh status");
    println!("Project        : {}", project_name);
    println!("Workspace      : {}", current_dir.display());
    println!("Persisted graph: {}", if loaded { "yes" } else { "rebuilt" });
    println!("Indexed files  : {}", scanned.len());
    println!("Graph nodes    : {}", stats.total_nodes);
    println!("Graph edges    : {}", stats.total_edges);
    println!("Resolved calls : {}", stats.resolved_calls);
    println!("Resolved imports: {}", stats.resolved_imports);
    println!("Workspace tokens: {}", graph.total_tokens());
    println!("Memory facts   : {}", memory_entries);
    println!();
    Ok(())
}
