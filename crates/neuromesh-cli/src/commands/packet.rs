use neuromesh_context::gold::{packet_file_names, packet_paths};
use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{
    Config, OptimizationMode, ProjectId, Result, RetrievalEngine, RetrievalMetadata, TaskSignature,
};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::TaskSignatureExtractor;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

use super::{configured_walker, FileCapArg};

#[derive(Debug, Default)]
struct PacketArgs {
    json: bool,
    query: Option<String>,
    engine: Option<RetrievalEngine>,
    keywords: Vec<String>,
    expansion: Vec<String>,
    path_hints: Vec<String>,
    entity_types: Vec<String>,
    intent: Option<String>,
}

#[derive(Debug, Serialize)]
struct PacketJsonOut {
    exit_code: i32,
    latency_ms: u64,
    mode: String,
    coverage: Option<String>,
    workspace_tokens: usize,
    packet_tokens: usize,
    seed_tokens: usize,
    fill_used: usize,
    fill_cap: usize,
    reduction_vs_workspace_pct: f32,
    selected_files: Vec<String>,
    selected_files_count: usize,
    identifiers: Vec<String>,
    seeds_missed: Vec<String>,
    seed_resolution: Option<neuromesh_core::SeedResolutionTelemetry>,
    retrieval: Option<RetrievalMetadata>,
    task: TaskJsonOut,
}

#[derive(Debug, Serialize)]
struct TaskJsonOut {
    client_keywords: Vec<String>,
    client_expansion: Vec<String>,
    client_path_hints: Vec<String>,
    client_entity_types: Vec<String>,
    client_intent: Option<String>,
    retrieval_engine_override: Option<String>,
}

pub fn execute(args: &[String]) -> Result<()> {
    let parsed = parse_args(args)?;
    let prompt = parsed.query.clone().ok_or_else(|| {
        neuromesh_core::NeuroMeshError::Config(
            "packet requires a prompt (--query or positional)".into(),
        )
    })?;

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
    let workspace_tokens = graph.total_tokens().max(1);

    let mut signature = TaskSignatureExtractor::extract(&prompt);
    apply_client_signals(&mut signature, &parsed);
    let registry = Arc::new(ReversibleContextRegistry::new());
    let activator = ContextActivator::new(registry);
    let started = Instant::now();
    let view = activator.activate_tiered(&graph, &signature, OptimizationMode::Balanced);
    let latency_ms = started.elapsed().as_millis() as u64;

    let mut files: Vec<String> = packet_file_names(&view).into_iter().collect();
    files.sort();
    let reduction = if workspace_tokens > 0 {
        (workspace_tokens.saturating_sub(view.active_tokens) as f32 / workspace_tokens as f32)
            * 100.0
    } else {
        0.0
    };

    let seeds_missed = view
        .coverage
        .as_ref()
        .map(|c| c.seeds_missed.clone())
        .unwrap_or_default();

    let payload = PacketJsonOut {
        exit_code: 0,
        latency_ms,
        mode: view.budget_mode.clone(),
        coverage: view.coverage.as_ref().map(|c| c.claim.clone()),
        workspace_tokens,
        packet_tokens: view.active_tokens,
        seed_tokens: view.budget_seed_tokens,
        fill_used: view.budget_fill_used,
        fill_cap: view.budget_fill_cap,
        reduction_vs_workspace_pct: reduction,
        selected_files_count: files.len(),
        selected_files: files.clone(),
        identifiers: signature.identifiers.clone(),
        seeds_missed,
        seed_resolution: view.seed_resolution_telemetry.clone(),
        retrieval: view.retrieval.clone(),
        task: TaskJsonOut {
            client_keywords: signature.client_keywords.clone(),
            client_expansion: signature.client_expansion.clone(),
            client_path_hints: signature.client_path_hints.clone(),
            client_entity_types: signature.client_entity_types.clone(),
            client_intent: signature.client_intent.clone(),
            retrieval_engine_override: signature
                .retrieval_engine_override
                .map(|e| e.as_str().to_string()),
        },
    };

    if parsed.json {
        println!("{}", serde_json::to_string(&payload)?);
    } else {
        print_human(&prompt, &payload, &packet_paths(&view));
    }

    neuromesh_observability::record_activity(neuromesh_observability::ActivityRecord {
        request_id: neuromesh_observability::cli_request_id("packet"),
        project_id: graph.project_id(),
        mode: "packet".into(),
        command: Some("packet".into()),
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
        model: "packet".into(),
        latency_ms,
        success: true,
        task_id: Some(prompt),
    });

    Ok(())
}

fn print_human(prompt: &str, payload: &PacketJsonOut, paths: &std::collections::HashSet<String>) {
    println!("\nNeuroMesh packet");
    println!("Prompt: {}", prompt);
    println!(
        "Mode: {} · {} ms · coverage {}",
        payload.mode,
        payload.latency_ms,
        payload.coverage.as_deref().unwrap_or("unknown")
    );
    if let Some(engine) = payload.task.retrieval_engine_override.as_deref() {
        println!("Engine override: {}", engine);
    }
    if !payload.task.client_keywords.is_empty() {
        println!("Keywords: {}", payload.task.client_keywords.join(", "));
    }
    if !payload.task.client_expansion.is_empty() {
        println!("Expansion: {}", payload.task.client_expansion.join(", "));
    }
    println!("Workspace tokens: {}", payload.workspace_tokens);
    println!(
        "Packet tokens: {} (seed {} + fill {} / {})",
        payload.packet_tokens, payload.seed_tokens, payload.fill_used, payload.fill_cap
    );
    println!(
        "Reduction vs workspace: {:.1}%",
        payload.reduction_vs_workspace_pct
    );
    println!("Files ({}):", payload.selected_files_count);
    for name in &payload.selected_files {
        println!("  {}", name);
    }
    if !payload.seeds_missed.is_empty() {
        println!("Seeds missed: {}", payload.seeds_missed.join(", "));
    }
    if !paths.is_empty() && payload.selected_files_count != paths.len() {
        println!("Paths ({} total nodes)", paths.len());
    }
    println!();
}

fn parse_args(args: &[String]) -> Result<PacketArgs> {
    let mut out = PacketArgs::default();
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        let arg = args[i].as_str();
        if i == 0 && matches!(arg, "packet" | "get_context_packet") {
            i += 1;
            continue;
        }
        match arg {
            "--json" => out.json = true,
            "--engine" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--engine needs a value".into())
                })?;
                out.engine = Some(RetrievalEngine::parse(raw).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config(format!(
                        "unknown engine {raw} (expected: {})",
                        RetrievalEngine::help_line()
                    ))
                })?);
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--engine=") => {
                out.engine = Some(RetrievalEngine::parse(raw).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config(format!(
                        "unknown engine {raw} (expected: {})",
                        RetrievalEngine::help_line()
                    ))
                })?);
            }
            "--keywords" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--keywords needs a value".into())
                })?;
                out.keywords.extend(parse_csv(raw));
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--keywords=") => {
                out.keywords.extend(parse_csv(raw));
            }
            "--expansion" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--expansion needs a value".into())
                })?;
                out.expansion.extend(parse_csv(raw));
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--expansion=") => {
                out.expansion.extend(parse_csv(raw));
            }
            "--path-hints" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--path-hints needs a value".into())
                })?;
                out.path_hints.extend(parse_csv(raw));
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--path-hints=") => {
                out.path_hints.extend(parse_csv(raw));
            }
            "--entity-types" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--entity-types needs a value".into())
                })?;
                out.entity_types.extend(parse_csv(raw));
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--entity-types=") => {
                out.entity_types.extend(parse_csv(raw));
            }
            "--query" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--query needs a value".into())
                })?;
                out.query = Some(raw.clone());
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--query=") => {
                out.query = Some(raw.to_string());
            }
            "--intent" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--intent needs a value".into())
                })?;
                out.intent = Some(raw.clone());
                i += 1;
            }
            v if let Some(raw) = v.strip_prefix("--intent=") => {
                out.intent = Some(raw.to_string());
            }
            _ if arg.starts_with('-') => {
                return Err(neuromesh_core::NeuroMeshError::Config(format!(
                    "unknown flag {arg}"
                )));
            }
            _ => positional.push(args[i].clone()),
        }
        i += 1;
    }
    if out.query.is_none() {
        out.query = positional.join(" ").trim().to_owned().into();
        if out.query.as_deref() == Some("") {
            out.query = None;
        }
    }
    Ok(out)
}

fn parse_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn normalize_keyword(raw: &str) -> String {
    raw.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn push_unique_normalized(out: &mut Vec<String>, raw: &str) {
    let normalized = normalize_keyword(raw);
    if normalized.is_empty() {
        return;
    }
    if !out
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&normalized))
    {
        out.push(normalized);
    }
}

fn apply_client_signals(signature: &mut TaskSignature, args: &PacketArgs) {
    for kw in &args.keywords {
        push_unique_normalized(&mut signature.client_keywords, kw);
    }
    for term in &args.expansion {
        push_unique_normalized(&mut signature.client_expansion, term);
    }
    for hint in &args.path_hints {
        let hint = hint.trim().replace('\\', "/");
        if hint.is_empty() {
            continue;
        }
        if !signature.client_path_hints.iter().any(|h| h == &hint) {
            signature.client_path_hints.push(hint);
        }
    }
    for et in &args.entity_types {
        let et = et.trim().to_lowercase();
        if et.is_empty() {
            continue;
        }
        if !signature.client_entity_types.iter().any(|e| e == &et) {
            signature.client_entity_types.push(et);
        }
    }
    if let Some(intent) = args.intent.as_deref() {
        let intent = intent.trim();
        if !intent.is_empty() {
            signature.client_intent = Some(intent.to_string());
        }
    }
    signature.retrieval_engine_override = args.engine;
    let prompt = args.query.as_deref().unwrap_or("");
    let engine = args
        .engine
        .unwrap_or_else(|| Config::load().retrieval.engine);
    let enabled =
        engine == RetrievalEngine::Fast && Config::load().seed_resolution.effective_auto_extract();
    apply_auto_extract_keywords(signature, prompt, enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_packet_flags() {
        let args = vec![
            "--json".into(),
            "--engine".into(),
            "hybrid".into(),
            "--keywords".into(),
            "Session,redirect".into(),
            "--expansion".into(),
            "retry,HTTPAdapter".into(),
            "--query".into(),
            "How does redirect work?".into(),
        ];
        let parsed = parse_args(&args).unwrap();
        assert!(parsed.json);
        assert_eq!(parsed.engine, Some(RetrievalEngine::Hybrid));
        assert_eq!(parsed.keywords, vec!["Session", "redirect"]);
        assert_eq!(parsed.expansion, vec!["retry", "HTTPAdapter"]);
        assert_eq!(parsed.query.as_deref(), Some("How does redirect work?"));
    }

    #[test]
    fn positional_prompt_when_no_query_flag() {
        let args = vec![
            "--json".into(),
            "Where".into(),
            "is".into(),
            "HTTPAdapter.send?".into(),
        ];
        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.query.as_deref(), Some("Where is HTTPAdapter.send?"));
    }
}
