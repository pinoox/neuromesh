use crate::state::AppState;
use neuromesh_core::{NodeId, OptimizationMode};
use neuromesh_router::QualityGate;
use neuromesh_task::TaskSignatureExtractor;
use serde_json::{json, Value};

/// Returns project status, biomimetic telemetry, and graph stats
pub fn get_status(state: &AppState) -> Value {
    let metrics = state.metrics.get_metrics();
    let graph_stats = state.graph.stats();
    let local_model = state.local_ai.get_model_info();

    json!({
        "status": "running",
        "protocol": "MCP (Model Context Protocol)",
        "project_id": state.graph.project_id().0,
        "mode": state.config.read().mode.to_string(),
        "local_model": {
            "name": local_model.name,
            "size": local_model.parameter_size,
            "loaded": local_model.loaded
        },
        "graph": graph_stats,
        "metrics": metrics,
        "biomimetic": state.mcp_handler.biomimetic_report()
    })
}

/// Returns graph nodes and edges for 2D Canvas rendering
pub fn get_graph_data(state: &AppState) -> Value {
    let nodes = state.graph.get_all_nodes();
    let edges_map = state.graph.get_edges_map();
    let edges: Vec<Value> = edges_map
        .values()
        .map(|e| {
            json!({
                "id": e.id.0,
                "source": e.source.0,
                "target": e.target.0,
                "edge_type": format!("{:?}", e.edge_type),
                "weight": e.pheromone_weight
            })
        })
        .collect();

    json!({
        "project_id": state.graph.project_id().0,
        "nodes": nodes,
        "edges": edges
    })
}

/// Runs a live biomimetic context simulation
pub fn simulate_context(state: &AppState, prompt: &str, mode: OptimizationMode) -> Value {
    let signature = TaskSignatureExtractor::extract(prompt);
    let gate = QualityGate::evaluate(&signature, mode);
    let view = state
        .activator
        .activate(&state.graph, &signature, gate.effective_mode);

    json!({
        "signature": signature,
        "membrane_state": gate.membrane_state,
        "context_view": view
    })
}

/// Reversibly expands an inactive node or fold
pub fn expand_context(state: &AppState, node_id: &str, reason: &str) -> Value {
    if let Some((view, audit)) = state
        .expansion_engine
        .expand_node(&NodeId::new(node_id), reason)
    {
        json!({
            "success": true,
            "expanded_node": view,
            "audit": audit
        })
    } else {
        json!({
            "success": false,
            "error": "Node not found in reversible registry"
        })
    }
}
