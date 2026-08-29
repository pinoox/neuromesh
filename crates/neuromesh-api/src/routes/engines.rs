use crate::state::AppState;
use neuromesh_core::{GraphBackendId, SeedEngineId};
use neuromesh_graph_proxy::detect_proxy_launch_specs;
use serde_json::{json, Value};

pub fn engines_status(state: &AppState) -> Value {
    let cfg = state.config.read();
    let ws = state.workspace();
    let detect = detect_proxy_launch_specs(&ws);
    let candidates: Vec<Value> = detect
        .candidates
        .iter()
        .take(8)
        .map(|c| {
            json!({
                "provider": c.spec.provider.as_str(),
                "server_name": c.spec.server_name,
                "command": c.spec.command,
                "score": c.score,
                "config_path": c.spec.config_path.as_ref().map(|p| p.display().to_string()),
            })
        })
        .collect();

    json!({
        "graph_backend": cfg.graph_backend.backend.as_str(),
        "graph_backend_active": state.mcp_handler.graph_backend_label(),
        "graph_proxy_connected": state.mcp_handler.graph_proxy_active(),
        "fallback_native": cfg.graph_backend.fallback_native,
        "seed_engine": cfg.seed_resolution.engine.as_str(),
        "detected_proxies": candidates,
    })
}

pub fn parse_graph_backend(raw: &str) -> Option<GraphBackendId> {
    GraphBackendId::parse(raw)
}

pub fn parse_seed_engine(raw: &str) -> Option<SeedEngineId> {
    SeedEngineId::parse(raw)
}
