use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_observability::{load_persisted_history, summarize_history, TelemetrySurface};
use neuromesh_task::TaskSignatureExtractor;
use std::sync::Arc;
use std::time::Instant;

use super::{configured_walker, snapshot, FileCapArg};

pub fn execute() -> Result<()> {
    let current_dir = neuromesh_index::assert_safe_workspace(&std::env::current_dir()?)?;
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

    let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
    let _ = graph.load_persisted(&current_dir);
    if graph.stats().total_nodes == 0 {
        graph.ingest_workspace(&scanned);
    }

    let prompt = "smoke test: list main entry points";
    let signature = TaskSignatureExtractor::extract(prompt);
    let registry = Arc::new(ReversibleContextRegistry::new());
    let activator = ContextActivator::new(registry);
    let started = Instant::now();
    let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
    let latency_ms = started.elapsed().as_millis() as u64;

    let workspace_tokens = graph.total_tokens().max(1);
    let tokens_after = view.active_tokens;
    let reduction = if workspace_tokens > 0 {
        (workspace_tokens.saturating_sub(tokens_after) as f32 / workspace_tokens as f32) * 100.0
    } else {
        0.0
    };

    neuromesh_observability::record_activity(neuromesh_observability::ActivityRecord {
        request_id: neuromesh_observability::cli_request_id("smoke"),
        project_id: project_id.clone(),
        mode: "smoke".into(),
        command: Some("smoke".into()),
        surface: TelemetrySurface::Cli,
        workspace_path: Some(current_dir.display().to_string()),
        client_id: None,
        tokens_before: workspace_tokens,
        tokens_after,
        token_reduction_pct: reduction,
        nodes_before: graph.stats().total_nodes,
        nodes_after: view.active_nodes.len(),
        expansions_count: 0,
        cache_hit: false,
        provider: "neuromesh-cli".into(),
        model: "smoke".into(),
        latency_ms,
        success: true,
        task_id: Some(prompt.into()),
    });

    let snap = snapshot::collect(&current_dir, &project_id, &graph, false);
    let history = load_persisted_history();
    let summary = summarize_history(&history);

    println!("\nNeuroMesh smoke — OK");
    println!("Workspace      : {}", current_dir.display());
    println!(
        "Graph          : {} nodes / {} edges",
        snap.graph_nodes, snap.graph_edges
    );
    println!(
        "Smoke packet   : {} tokens ({:.1}% vs workspace dump)",
        tokens_after, reduction
    );
    println!(
        "Coverage       : {}",
        view.coverage
            .as_ref()
            .map(|c| c.claim.as_str())
            .unwrap_or("unknown")
    );
    println!(
        "Telemetry disk : {} rows ({:.1}% mean reduction all-time)",
        summary.total_requests, summary.mean_reduction_pct
    );
    println!("Monitor        : {}", snap.monitor_status);
    println!();
    Ok(())
}
