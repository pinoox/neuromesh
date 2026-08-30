use neuromesh_context::gold::packet_file_names;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::TaskSignatureExtractor;
use std::sync::Arc;
use std::time::Instant;

use super::{configured_walker, FileCapArg};

pub fn execute(task_prompt: Option<String>) -> Result<()> {
    let prompt =
        task_prompt.unwrap_or_else(|| "How does handle_tool_call extract intent?".to_string());

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

    let graph = NeuralProjectGraph::new(project_id);
    graph.ingest_workspace(&scanned);
    let workspace_tokens = graph.total_tokens().max(1);

    let signature = TaskSignatureExtractor::extract(&prompt);
    let registry = Arc::new(ReversibleContextRegistry::new());
    let activator = ContextActivator::new(registry);
    let started = Instant::now();
    let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
    let ms = started.elapsed().as_millis();

    let mut files: Vec<String> = packet_file_names(&view).into_iter().collect();
    files.sort();
    let reduction = if workspace_tokens > 0 {
        (workspace_tokens.saturating_sub(view.active_tokens) as f32 / workspace_tokens as f32)
            * 100.0
    } else {
        0.0
    };

    println!("\nNeuroMesh optimize");
    println!("Prompt: {}", prompt);
    println!("Identifiers: {}", signature.identifiers.join(", "));
    println!(
        "Mode: {} · {} ms · coverage {}",
        view.budget_mode,
        ms,
        view.coverage
            .as_ref()
            .map(|c| c.claim.as_str())
            .unwrap_or("unknown")
    );
    println!("Workspace tokens: {}", workspace_tokens);
    println!(
        "Packet tokens: {} (seed {} + fill {} / {})",
        view.active_tokens, view.budget_seed_tokens, view.budget_fill_used, view.budget_fill_cap
    );
    println!("Reduction vs workspace: {:.1}%", reduction);
    println!("Files ({}):", files.len());
    for name in &files {
        println!("  {}", name);
    }
    if let Some(coverage) = &view.coverage {
        if !coverage.seeds_missed.is_empty() {
            println!("Seeds missed: {}", coverage.seeds_missed.join(", "));
        }
    }

    neuromesh_observability::record_activity(neuromesh_observability::ActivityRecord {
        request_id: neuromesh_observability::cli_request_id("optimize"),
        project_id: graph.project_id(),
        mode: "optimize".into(),
        command: Some("optimize".into()),
        surface: neuromesh_observability::TelemetrySurface::Cli,
        workspace_path: Some(current_dir.display().to_string()),
        client_id: None,
        tokens_before: workspace_tokens,
        tokens_after: view.active_tokens,
        token_reduction_pct: reduction,
        nodes_before: graph.stats().total_nodes,
        nodes_after: view.active_nodes.len(),
        expansions_count: 0,
        cache_hit: false,
        provider: "neuromesh-cli".into(),
        model: "optimize".into(),
        latency_ms: ms as u64,
        success: true,
        task_id: Some(prompt.clone()),
    });

    println!();
    Ok(())
}
