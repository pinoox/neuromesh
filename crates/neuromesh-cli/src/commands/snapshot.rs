use chrono::{DateTime, Utc};
use neuromesh_core::{Config, ProjectId};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_observability::{
    filter_history, load_persisted_history, summarize_history, AggregatedMetrics,
};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    pub workspace: PathBuf,
    pub project_id: ProjectId,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub graph_ready: bool,
    pub persisted_graph: bool,
    pub store_mode: &'static str,
    pub telemetry: AggregatedMetrics,
    pub telemetry_rows: usize,
    pub telemetry_last: Option<DateTime<Utc>>,
    pub monitor_url: String,
    pub monitor_reachable: bool,
    pub monitor_status: String,
    pub mcp_workspace_env: Option<String>,
    pub mcp_predicted_root: PathBuf,
}

pub fn collect(
    workspace: &Path,
    project_id: &ProjectId,
    graph: &NeuralProjectGraph,
    all_projects: bool,
) -> ProjectSnapshot {
    let stats = graph.stats();
    let workspace_str = workspace.display().to_string();
    let history = load_persisted_history();
    let filtered = filter_history(&history, project_id, &workspace_str, all_projects);
    let telemetry = summarize_history(&filtered);
    let telemetry_last = filtered.last().map(|r| r.timestamp);
    let cfg = Config::load();
    let monitor_url = format!("http://{}:{}", cfg.host, cfg.port);
    let monitor_reachable = monitor_tcp_reachable(&cfg.host, cfg.port);
    let monitor_status = if monitor_reachable {
        format!("{monitor_url} (reachable)")
    } else {
        format!("{monitor_url} (offline — run neuromesh monitor)")
    };
    let persisted_graph = neuromesh_core::graph_path(workspace).exists();
    let store_mode = if neuromesh_core::uses_local_dotdir(workspace) {
        "local (.neuromesh trusted)"
    } else {
        "managed (~/.neuromesh/projects/…)"
    };

    ProjectSnapshot {
        workspace: workspace.to_path_buf(),
        project_id: project_id.clone(),
        graph_nodes: stats.total_nodes,
        graph_edges: stats.total_edges,
        graph_ready: matches!(graph.index_state(), neuromesh_graph::IndexState::Ready),
        persisted_graph,
        store_mode,
        telemetry,
        telemetry_rows: filtered.len(),
        telemetry_last,
        monitor_url,
        monitor_reachable,
        monitor_status,
        mcp_workspace_env: neuromesh_index::mcp_workspace_env_summary(),
        mcp_predicted_root: neuromesh_index::resolve_mcp_startup_workspace(),
    }
}

pub fn collect_from_cwd(
    all_projects: bool,
) -> Result<ProjectSnapshot, neuromesh_core::NeuroMeshError> {
    let cwd = std::env::current_dir()?;
    let root = ProjectWalker::discover_workspace(&cwd);
    let project_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project_id = ProjectId::new(&project_name);
    let graph = NeuralProjectGraph::new(project_id.clone());
    let _ = graph.load_persisted(&root);
    Ok(collect(&root, &project_id, &graph, all_projects))
}

fn monitor_tcp_reachable(host: &str, port: u16) -> bool {
    let endpoint = format!("{host}:{port}");
    let Ok(mut addrs) = endpoint.to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}
