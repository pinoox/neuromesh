use neuromesh_core::{ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;

use super::{configured_walker, snapshot, FileCapArg};

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project_id = ProjectId::new(&project_name);
    let walker = configured_walker(
        current_dir.clone(),
        project_id.clone(),
        FileCapArg::Unspecified,
    );
    let scanned = walker.scan().unwrap_or_default();

    let graph = NeuralProjectGraph::new(project_id.clone());
    let loaded = graph.load_persisted(&current_dir);
    if !loaded {
        graph.ingest_workspace(&scanned);
    }

    let snap = snapshot::collect(&current_dir, &project_id, &graph, false);

    println!("\nNeuroMesh status");
    println!("Project        : {}", snap.project_id.0);
    println!("Workspace      : {}", snap.workspace.display());
    println!(
        "Persisted graph: {}",
        if snap.persisted_graph || loaded {
            "yes"
        } else {
            "rebuilt"
        }
    );
    println!("Indexed files  : {}", scanned.len());
    println!(
        "Graph          : {} nodes / {} edges ({})",
        snap.graph_nodes,
        snap.graph_edges,
        if snap.graph_ready {
            "ready"
        } else {
            "indexing or empty"
        }
    );
    println!("Resolved calls : {}", graph.stats().resolved_calls);
    println!("Resolved imports: {}", graph.stats().resolved_imports);
    println!("Workspace tokens: {}", graph.total_tokens());
    println!(
        "Telemetry      : {} requests | {:.1}% mean | {:.1}% overall",
        snap.telemetry_rows,
        snap.telemetry.mean_reduction_pct,
        snap.telemetry.overall_reduction_pct
    );
    if let Some(ts) = snap.telemetry_last {
        println!("Last activity  : {ts}");
    }
    println!("Monitor        : {}", snap.monitor_status);
    println!("Store          : {}", snap.store_mode);
    if let Some(env_line) = &snap.mcp_workspace_env {
        println!("MCP IDE env    : {env_line}");
    }
    println!("MCP predicted  : {}", snap.mcp_predicted_root.display());
    println!();
    Ok(())
}
