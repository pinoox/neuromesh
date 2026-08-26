use neuromesh_cache::{MyceliumCache, MyceliumConfig, MyceliumStats};
use neuromesh_context::{CodeSkeletonizer, ContextActivator, ExpansionEngine};
use neuromesh_core::{NeuroMeshError, NodeId, OptimizationMode, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::{MemoryDatabase, WorkingMemory};
use neuromesh_router::QualityGate;
use neuromesh_task::TaskSignatureExtractor;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct McpToolHandler {
    graph: Arc<NeuralProjectGraph>,
    activator: Arc<ContextActivator>,
    expansion_engine: Arc<ExpansionEngine>,
    memory_db: Arc<MemoryDatabase>,
    working_memory: Arc<parking_lot::RwLock<WorkingMemory>>,
    mycelium: Arc<MyceliumCache>,
}

impl McpToolHandler {
    pub fn new(
        graph: Arc<NeuralProjectGraph>,
        activator: Arc<ContextActivator>,
        expansion_engine: Arc<ExpansionEngine>,
        memory_db: Arc<MemoryDatabase>,
        working_memory: Arc<parking_lot::RwLock<WorkingMemory>>,
    ) -> Self {
        Self {
            graph,
            activator,
            expansion_engine,
            memory_db,
            working_memory,
            mycelium: Arc::new(MyceliumCache::new(MyceliumConfig::default())),
        }
    }

    pub fn graph(&self) -> &Arc<NeuralProjectGraph> {
        &self.graph
    }

    pub fn mycelium_stats(&self) -> MyceliumStats {
        self.mycelium.stats()
    }

    pub fn biomimetic_report(&self) -> Value {
        let phys = self.activator.last_physarum();
        let myc = self.mycelium.stats();
        json!({
            "physarum_solver": if phys.used { "active" } else { "idle" },
            "physarum_last_ms": phys.ms,
            "physarum_sla_ms": 20,
            "synaptic_stdp": "armed",
            "bio_genetic_slicing": "active",
            "folds_in_session": self.activator.registry().fold_count(),
            "mycelial_prefetching": if myc.total_prefetches > 0 { "active" } else { "idle" },
            "mycelium": myc,
            "cellular_osmotic_gate": "active",
            "last_packet": self.activator.last_packet(),
        })
    }

    fn mycelium_file_hit(&self, file_id: &NodeId) -> bool {
        self.mycelium.get_prewarmed(file_id).is_some()
    }

    fn record_mycelium_path(&self, path: &[NodeId]) {
        for window in path.windows(2) {
            self.mycelium.record_transition(&window[0], &window[1]);
            if let Some(node) = self.graph.get_node(&window[1]) {
                if let Some(content) = self.graph.read_source(&node.file_path) {
                    self.mycelium.prewarm_node(window[1].clone(), content);
                }
            }
        }
    }

    fn prefetch_mycelium(&self, view: &neuromesh_core::ContextView) {
        let files: Vec<NodeId> = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
            .map(|n| n.node.id.clone())
            .collect();
        self.record_mycelium_path(&files);
        if let Some(last) = files.last() {
            for tip in self.mycelium.predict_next_nodes(last) {
                if let Some(node) = self.graph.get_node(&tip.target_node) {
                    if let Some(content) = self.graph.read_source(&node.file_path) {
                        self.mycelium.prewarm_node(tip.target_node, content);
                    }
                }
            }
        }
    }

    pub async fn handle_tool_call(&self, name: &str, arguments: &Value) -> Result<Value> {
        match name {
            // 1. Task-conditioned evidence packet (seed files + fill-budget connectors)
            "neuromesh_get_context" | "activate_context" => {
                let start_time = std::time::Instant::now();
                let task_desc = read_task_description(arguments)?;

                let mode_str = arguments["mode"].as_str().unwrap_or("balanced");
                let requested_mode = match mode_str {
                    "max_quality" => OptimizationMode::MaxQuality,
                    "max_savings" => OptimizationMode::MaxSavings,
                    _ => OptimizationMode::Balanced,
                };

                let signature = TaskSignatureExtractor::extract(&task_desc);
                let gate = QualityGate::evaluate(&signature, requested_mode);
                let view = self
                    .activator
                    .activate(&self.graph, &signature, gate.effective_mode);
                self.prefetch_mycelium(&view);

                for active in &view.active_nodes {
                    self.graph
                        .record_neural_spike(active.node.id.clone(), false, true);
                }

                let elapsed_ms = start_time.elapsed().as_millis() as u64;
                let workspace_tokens = self.graph.total_tokens().max(1);
                let opt_tokens = view.active_tokens;
                let seeds_missed = view
                    .coverage
                    .as_ref()
                    .map(|c| !c.seeds_missed.is_empty())
                    .unwrap_or(false);
                let vs_workspace = if seeds_missed && opt_tokens == 0 {
                    0.0
                } else {
                    (workspace_tokens.saturating_sub(opt_tokens) as f32 / workspace_tokens as f32)
                        * 100.0
                };
                let selected_raw = view.total_raw_tokens.max(opt_tokens);
                let vs_selected = if selected_raw > 0 {
                    (selected_raw.saturating_sub(opt_tokens) as f32 / selected_raw as f32) * 100.0
                } else {
                    0.0
                };

                let index_meta = self.graph.index_meta();
                let files: Vec<Value> = view
                    .active_nodes
                    .iter()
                    .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
                    .map(|n| {
                        json!({
                            "path": n.node.file_path,
                            "skeleton": n.node.content,
                            "tokens": n.node.token_cost,
                            "why": n.expansion_reason,
                            "line_range": n.node.line_range,
                            "folded_symbols": n.folded_symbols,
                            "folds": n.folded_symbols.iter().flat_map(|sym| {
                                view.fold_ids.iter().filter(|id| id.contains(sym)).cloned().collect::<Vec<_>>()
                            }).collect::<Vec<_>>(),
                        })
                    })
                    .collect();

                let symbols: Vec<Value> = view
                    .active_nodes
                    .iter()
                    .filter(|n| n.node.node_type != neuromesh_core::NodeType::File)
                    .map(|n| {
                        json!({
                            "name": n.node.name,
                            "path": n.node.file_path,
                            "signature": n.node.signature,
                            "why": n.expansion_reason,
                            "kind": n.node.node_type,
                            "id": n.node.id,
                            "lines": n.node.line_range,
                            "score": n.activation_score,
                        })
                    })
                    .collect();

                neuromesh_observability::record_global_telemetry(
                    neuromesh_core::OptimizationMetadata {
                        request_id: telemetry_request_id("mcp"),
                        task_id: Some(task_desc.chars().take(50).collect()),
                        project_id: self.graph.project_id(),
                        mode: gate.effective_mode.to_string(),
                        tokens_before: workspace_tokens,
                        tokens_after: opt_tokens,
                        token_reduction_pct: vs_workspace,
                        nodes_before: self.graph.stats().total_nodes,
                        nodes_after: view.active_nodes.len(),
                        expansions_count: 0,
                        cache_hit: false,
                        provider: "Cursor / Claude MCP".to_string(),
                        model: "Frontier Model".to_string(),
                        latency_ms: elapsed_ms,
                        success: true,
                        timestamp: chrono::Utc::now(),
                    },
                );

                Ok(json!({
                    "task": {
                        "intent": signature.intent,
                        "entity": signature.entity,
                        "identifiers": signature.identifiers,
                        "file_hints": signature.file_hints,
                        "confidence": signature.confidence,
                    },
                    "membrane_state": gate.membrane_state,
                    "effective_mode": format!("{:?}", gate.effective_mode),
                    "latency_ms": elapsed_ms,
                    "evidence_packet": {
                        "index": {
                            "generation": index_meta.generation,
                            "file_count": index_meta.file_count,
                            "indexed_at": index_meta.indexed_at,
                            "stale_files": index_meta.stale_files,
                        },
                        "seeds": view.seeds,
                        "files": files,
                        "symbols": symbols,
                        "unresolved": view.unresolved,
                        "coverage": view.coverage,
                        "fold_ids": view.fold_ids,
                        "next_actions": view.next_actions,
                        "budget": {
                            "used": view.budget_used,
                            "cap": view.budget_cap,
                            "mode": view.budget_mode,
                            "seed_tokens": view.budget_seed_tokens,
                            "fill_used": view.budget_fill_used,
                            "fill_cap": view.budget_fill_cap,
                            "over_budget": view.over_budget,
                        },
                        "inactive_hints": view.inactive_descriptors,
                        "workspace_tokens": workspace_tokens,
                        "selected_raw_tokens": selected_raw,
                        "active_tokens": opt_tokens,
                        "reduction_vs_workspace_pct": format!("{:.1}%", vs_workspace),
                        "reduction_vs_selected_pct": format!("{:.1}%", vs_selected),
                        "seed_call_coverage": view.seed_call_coverage,
                        "physarum_used": view.physarum_used,
                        "physarum_ms": view.physarum_ms,
                        "selection_method": view.selection_method,
                    }
                }))
            }

            // 2. Get File Skeleton with Folded Introns
            "neuromesh_get_file_skeleton" => {
                let start_time = std::time::Instant::now();
                let file_path = arguments["file_path"]
                    .as_str()
                    .or_else(|| arguments["path"].as_str())
                    .or_else(|| arguments["file"].as_str())
                    .unwrap_or("");
                let active_symbols: HashSet<String> = arguments["active_symbols"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let node_id = NodeId::from_file_path(file_path);
                let content_opt = self
                    .graph
                    .get_node(&node_id)
                    .and_then(|n| self.graph.read_source(&n.file_path))
                    .or_else(|| std::fs::read_to_string(file_path).ok());

                if let Some(content) = content_opt {
                    let res = CodeSkeletonizer::skeletonize(file_path, &content, &active_symbols);
                    for fold in &res.folds {
                        self.expansion_engine
                            .registry()
                            .register_fold(std::path::PathBuf::from(file_path), fold.clone());
                    }
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;

                    neuromesh_observability::record_global_telemetry(
                        neuromesh_core::OptimizationMetadata {
                            request_id: telemetry_request_id("mcp-skel"),
                            task_id: Some(format!("Skeleton: {}", file_path)),
                            project_id: self.graph.project_id(),
                            mode: "Genetic Slicing".to_string(),
                            tokens_before: res.original_tokens,
                            tokens_after: res.skeleton_tokens,
                            token_reduction_pct: res.token_reduction_pct,
                            nodes_before: 1,
                            nodes_after: 1,
                            expansions_count: 0,
                            cache_hit: false,
                            provider: "Cursor / Claude MCP".to_string(),
                            model: "Frontier Model".to_string(),
                            latency_ms: elapsed_ms,
                            success: true,
                            timestamp: chrono::Utc::now(),
                        },
                    );

                    Ok(json!({
                        "file_path": file_path,
                        "skeleton_code": res.skeleton_code,
                        "original_tokens": res.original_tokens,
                        "skeleton_tokens": res.skeleton_tokens,
                        "token_reduction_pct": format!("{:.1}%", res.token_reduction_pct),
                        "introns_folded": res.introns_folded,
                        "folds": res.folds
                    }))
                } else {
                    Ok(json!({ "error": format!("File not found or unreadable: {}", file_path) }))
                }
            }

            // 3. Reversibly Expand Folded Intron or Inactive Context
            "neuromesh_expand_fold" | "expand_context" => {
                let start_time = std::time::Instant::now();
                let node_id_str = read_fold_query(arguments);
                let reason = arguments["reason"]
                    .as_str()
                    .unwrap_or("Agent requested expansion");

                if node_id_str.is_empty() {
                    Ok(json!({
                        "success": false,
                        "error": "Node or fold not found in reversible registry: missing fold_id (pass fold_id, node_id, or query from next_actions)"
                    }))
                } else if let Some(fold) = self.expansion_engine.expand_fold(&node_id_str) {
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let file_id = NodeId::from_file_path(&fold.file_path);
                    let cache_hit = self.mycelium_file_hit(&file_id);
                    neuromesh_observability::record_global_telemetry(
                        neuromesh_core::OptimizationMetadata {
                            request_id: telemetry_request_id("mcp-exp"),
                            task_id: Some(format!("Expand: {}", fold.fold_id)),
                            project_id: self.graph.project_id(),
                            mode: "expand_fold".to_string(),
                            tokens_before: fold.restored_tokens,
                            tokens_after: fold.restored_tokens,
                            token_reduction_pct: 0.0,
                            nodes_before: 1,
                            nodes_after: 1,
                            expansions_count: 1,
                            cache_hit,
                            provider: "Cursor / Claude MCP".to_string(),
                            model: "Frontier Model".to_string(),
                            latency_ms: elapsed_ms,
                            success: true,
                            timestamp: chrono::Utc::now(),
                        },
                    );
                    Ok(json!({
                        "success": true,
                        "kind": "fold",
                        "fold_id": fold.fold_id,
                        "symbol_name": fold.symbol_name,
                        "signature": fold.signature,
                        "original_body": fold.original_body,
                        "file_path": fold.file_path,
                        "start_line": fold.start_line,
                        "end_line": fold.end_line,
                        "restored_tokens": fold.restored_tokens,
                        "latency_ms": elapsed_ms,
                        "reason": reason,
                        "mycelium_hit": cache_hit,
                    }))
                } else if let Some((view, audit)) = self
                    .expansion_engine
                    .expand_node(&NodeId::new(&node_id_str), reason)
                {
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;

                    neuromesh_observability::record_global_telemetry(
                        neuromesh_core::OptimizationMetadata {
                            request_id: telemetry_request_id("mcp-exp"),
                            task_id: Some(format!("Expand: {}", node_id_str)),
                            project_id: self.graph.project_id(),
                            mode: "expand_fold".to_string(),
                            tokens_before: 0,
                            tokens_after: audit.added_tokens,
                            token_reduction_pct: 0.0,
                            nodes_before: 1,
                            nodes_after: 1,
                            expansions_count: 1,
                            cache_hit: false,
                            provider: "Cursor / Claude MCP".to_string(),
                            model: "Frontier Model".to_string(),
                            latency_ms: elapsed_ms,
                            success: true,
                            timestamp: chrono::Utc::now(),
                        },
                    );

                    Ok(json!({
                        "success": true,
                        "kind": "node",
                        "expanded_node": view,
                        "audit": audit
                    }))
                } else {
                    Ok(json!({
                        "success": false,
                        "error": format!("Node or fold not found in reversible registry: {}", node_id_str)
                    }))
                }
            }

            // 4. Search Symbols & Code Nodes
            "neuromesh_search_symbols" | "search_context" | "get_symbol" => {
                let start_time = std::time::Instant::now();
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["name"].as_str())
                    .unwrap_or("");
                let limit = arguments["limit"].as_u64().unwrap_or(20) as usize;
                let nodes = self.graph.search_symbols(query, limit);
                let elapsed_ms = start_time.elapsed().as_millis() as u64;

                neuromesh_observability::record_global_telemetry(
                    neuromesh_core::OptimizationMetadata {
                        request_id: telemetry_request_id("mcp-sym"),
                        task_id: Some(format!("Search: {}", query)),
                        project_id: self.graph.project_id(),
                        mode: "symbol_search".to_string(),
                        tokens_before: 0,
                        tokens_after: 0,
                        token_reduction_pct: 0.0,
                        nodes_before: self.graph.stats().total_nodes,
                        nodes_after: nodes.len(),
                        expansions_count: 0,
                        cache_hit: false,
                        provider: "Cursor / Claude MCP".to_string(),
                        model: "Frontier Model".to_string(),
                        latency_ms: elapsed_ms,
                        success: true,
                        timestamp: chrono::Utc::now(),
                    },
                );

                Ok(json!({
                    "query": query,
                    "matches_count": nodes.len(),
                    "results": nodes
                }))
            }

            // 5. Get Graph Dependencies & Synaptic Weights
            "neuromesh_get_dependencies" | "get_dependency_graph" => {
                let symbol_or_path = arguments["symbol_or_path"]
                    .as_str()
                    .or_else(|| arguments["file_path"].as_str())
                    .unwrap_or("");

                let resolved = self.graph.resolve_best(symbol_or_path);
                let Some(node) = resolved else {
                    return Ok(json!({
                        "target": symbol_or_path,
                        "connected_neighbors_count": 0,
                        "dependencies": [],
                        "error": "Symbol or file not found in the project graph"
                    }));
                };

                let neighbors = self.graph.get_neighbor_views(&node.id);
                Ok(json!({
                    "target": {
                        "id": node.id,
                        "name": node.name,
                        "path": node.file_path,
                        "kind": node.node_type,
                        "signature": node.signature,
                    },
                    "connected_neighbors_count": neighbors.len(),
                    "dependencies": neighbors
                }))
            }

            "neuromesh_trace" => {
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["symbol"].as_str())
                    .or_else(|| arguments["function_name"].as_str())
                    .unwrap_or("");
                let direction = neuromesh_graph::TraceDirection::parse(
                    arguments["direction"].as_str().unwrap_or("both"),
                );
                let depth = arguments["depth"].as_u64().unwrap_or(3) as usize;
                let result = self.graph.trace_symbol(query, direction, depth);
                Ok(json!(result))
            }

            "neuromesh_analyze_impact" => {
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["symbol_or_path"].as_str())
                    .unwrap_or("");
                let depth = arguments["depth"].as_u64().unwrap_or(3) as usize;
                Ok(json!(self.graph.analyze_impact(query, depth)))
            }

            "neuromesh_get_architecture" => Ok(json!(self.graph.architecture_summary())),

            // 6. Record Feedback & Trigger Synaptic STDP Plasticity Learning
            "neuromesh_record_feedback" => {
                let success = read_bool(&arguments["task_success"], true);
                let touched_nodes = read_string_list(arguments, "touched_nodes");

                let mut path: Vec<NodeId> = Vec::new();
                for node_name in &touched_nodes {
                    let node_id = NodeId::new(node_name);
                    self.graph
                        .record_neural_spike(node_id.clone(), true, success);
                    path.push(node_id);
                }
                self.graph.apply_stdp_on_path(&path);
                self.graph.reinforce_path(&path, success);
                self.record_mycelium_path(&path);
                if let Ok(cwd) = std::env::current_dir() {
                    let _ = self.graph.save_persisted(&cwd);
                }

                Ok(json!({
                    "status": "Feedback recorded",
                    "success": success,
                    "stdp_learning_applied": true,
                    "path_nodes": path.len(),
                    "updated_graph_stats": self.graph.stats()
                }))
            }

            // 7. Get Project Memory & Conventions
            "neuromesh_get_project_memory" => {
                let pid = self.graph.project_id();
                let facts = self.memory_db.get_project_facts(&pid)?;
                Ok(json!({
                    "project_id": pid.0,
                    "facts_count": facts.len(),
                    "project_memory": facts
                }))
            }

            // 8. Search Episodic Memory
            "search_memory" | "get_previous_solution" => {
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["task_similarity_query"].as_str())
                    .unwrap_or("");
                let pid = self.graph.project_id();
                let episodes = self.memory_db.find_similar_episodes(&pid, query)?;
                Ok(json!({
                    "query": query,
                    "episodes_count": episodes.len(),
                    "episodes": episodes
                }))
            }

            // 9. Get Task State
            "get_task_state" => {
                let wm = self.working_memory.read().clone();
                Ok(json!({ "working_memory": wm }))
            }

            // 10. Get System Stats & Biomimetic Health
            "neuromesh_get_stats" => {
                let stats = self.graph.stats();
                Ok(json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "project_id": self.graph.project_id().0,
                    "graph_stats": stats,
                    "biomimetic_engine": self.biomimetic_report()
                }))
            }

            _ => Ok(json!({ "error": format!("Unknown tool: {}", name) })),
        }
    }
}

fn read_fold_query(arguments: &Value) -> String {
    ["fold_id", "node_id", "query", "id", "name"]
        .into_iter()
        .find_map(|k| arguments.get(k).and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string()
}

fn read_task_description(arguments: &Value) -> Result<String> {
    let raw = [
        "task_description",
        "prompt",
        "task",
        "description",
        "text",
        "message",
        "query",
    ]
    .into_iter()
    .find_map(|k| arguments.get(k).and_then(Value::as_str))
    .unwrap_or("")
    .trim();
    if raw.is_empty() {
        Err(NeuroMeshError::Config(
            "neuromesh_get_context requires a prompt (task_description, prompt, or task)".into(),
        ))
    } else {
        Ok(raw.to_string())
    }
}

fn read_bool(value: &Value, default: bool) -> bool {
    if let Some(b) = value.as_bool() {
        return b;
    }
    match value.as_str().map(|s| s.trim()) {
        Some("true") | Some("1") | Some("yes") => true,
        Some("false") | Some("0") | Some("no") => false,
        _ => default,
    }
}

fn read_string_list(arguments: &Value, key: &str) -> Vec<String> {
    match arguments.get(key) {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split([',', '\n'])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn telemetry_request_id(prefix: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    format!(
        "{prefix}-{}-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_context::ReversibleContextRegistry;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_memory::MemoryDatabase;
    use neuromesh_parser::CodeIntelligenceEngine;
    use parking_lot::RwLock;
    use std::path::PathBuf;

    fn indexed(rel: &str) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("neuromesh"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: "test".into(),
            byte_size: 80,
            token_count: 40,
            language: SourceLanguage::Rust,
            last_modified: chrono::Utc::now(),
        }
    }

    #[test]
    fn mycelium_records_packet_transitions() {
        let graph = Arc::new(NeuralProjectGraph::new(ProjectId::new("neuromesh")));
        let a = "pub fn start_job() { enqueue_job(); }\n";
        let b = "pub fn enqueue_job() { let x = 1; x }\n";
        graph.ingest_file(
            &indexed("src/worker.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("worker.rs"), a, SourceLanguage::Rust),
            Some(a),
        );
        graph.ingest_file(
            &indexed("src/queue.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("queue.rs"), b, SourceLanguage::Rust),
            Some(b),
        );
        graph.finalize_links();
        let registry = Arc::new(ReversibleContextRegistry::new());
        let handler = McpToolHandler::new(
            graph,
            Arc::new(ContextActivator::new(registry.clone())),
            Arc::new(ExpansionEngine::new(registry)),
            Arc::new(MemoryDatabase::open_in_memory().unwrap()),
            Arc::new(RwLock::new(WorkingMemory::default())),
        );
        let idle = handler.biomimetic_report();
        assert_eq!(idle["mycelial_prefetching"].as_str(), Some("idle"));
        let args = json!({ "task_description": "How does start_job enqueue_job?" });
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            handler
                .handle_tool_call("neuromesh_get_context", &args)
                .await
                .unwrap();
            handler
                .handle_tool_call("neuromesh_get_context", &args)
                .await
                .unwrap();
        });
        let stats = handler.mycelium_stats();
        assert!(
            stats.total_hyphal_trails > 0 || stats.total_prefetches > 0,
            "mycelium should record packet transitions: {stats:?}"
        );
    }

    #[test]
    fn get_context_accepts_task_alias_and_rejects_empty() {
        let graph = Arc::new(NeuralProjectGraph::new(ProjectId::new("neuromesh")));
        let registry = Arc::new(ReversibleContextRegistry::new());
        let handler = McpToolHandler::new(
            graph,
            Arc::new(ContextActivator::new(registry.clone())),
            Arc::new(ExpansionEngine::new(registry)),
            Arc::new(MemoryDatabase::open_in_memory().unwrap()),
            Arc::new(RwLock::new(WorkingMemory::default())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert!(handler
                .handle_tool_call("neuromesh_get_context", &json!({}))
                .await
                .is_err());
            assert!(handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "foo": "how does Router work" })
                )
                .await
                .is_err());
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does start_job enqueue_job?" }),
                )
                .await
                .expect("task alias should populate the prompt");
            assert!(packet.get("evidence_packet").is_some());
            let via_text = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "text": "How does start_job enqueue_job?" }),
                )
                .await
                .expect("text alias should populate the prompt");
            assert!(via_text.get("evidence_packet").is_some());
        });
    }

    #[test]
    fn expand_fold_accepts_query_from_next_actions() {
        let graph = Arc::new(NeuralProjectGraph::new(ProjectId::new("neuromesh")));
        let tools = r#"
pub fn handle_tool_call() {
    let signature = 1;
    signature
}
pub fn unused_helper() {
    let x = 1;
    let y = 2;
    let z = 3;
    let w = 4;
    x + y + z + w
}
"#;
        graph.ingest_file(
            &indexed("src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.finalize_links();
        let registry = Arc::new(ReversibleContextRegistry::new());
        let handler = McpToolHandler::new(
            graph,
            Arc::new(ContextActivator::new(registry.clone())),
            Arc::new(ExpansionEngine::new(registry)),
            Arc::new(MemoryDatabase::open_in_memory().unwrap()),
            Arc::new(RwLock::new(WorkingMemory::default())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does handle_tool_call work?" }),
                )
                .await
                .expect("packet");
            let fold_id = packet["evidence_packet"]["fold_ids"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .or_else(|| {
                    packet["evidence_packet"]["next_actions"]
                        .as_array()
                        .and_then(|actions| {
                            actions.iter().find_map(|a| {
                                (a["tool"] == "neuromesh_expand_fold")
                                    .then(|| a["query"].as_str())
                                    .flatten()
                            })
                        })
                })
                .expect("fold id from packet or next_actions")
                .to_string();
            let expanded = handler
                .handle_tool_call("neuromesh_expand_fold", &json!({ "query": fold_id }))
                .await
                .expect("query alias must expand the printed fold");
            assert_eq!(expanded["success"], true);
            assert!(expanded["original_body"]
                .as_str()
                .unwrap_or("")
                .contains("let w = 4"));
        });
    }
}
