use crate::graph_proxy::proxy_evidence_response;
use crate::packet_cache::PacketDetailCache;
use crate::response::{
    apply_semantic_cache_hit, cache_and_build, collect_file_entries, collect_symbols,
    explain_packet, fold_descriptors_from_skeleton, ContextBuild, ResponseDetail,
};
#[cfg(feature = "embeddings")]
use crate::semantic_cache::McpSemanticCache;
use neuromesh_cache::{MyceliumCache, MyceliumConfig, MyceliumStats};
use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{CodeSkeletonizer, ContextActivator, ExpansionEngine};
use neuromesh_core::{
    Config, NeuroMeshError, NodeId, OptimizationMode, Result, SeedEngineId, TaskSignature,
};
#[cfg(feature = "embeddings")]
use neuromesh_embed::{embed_query_cached, packet_cache_begin, packet_cache_end, SemanticCacheKey};
#[cfg(feature = "embeddings")]
use neuromesh_graph::graph_digest;
use neuromesh_graph::{IndexState, NeuralProjectGraph};
use neuromesh_graph_proxy::{GraphProxySession, ProxySearchContext};
use neuromesh_memory::{MemoryDatabase, WorkingMemory};
use neuromesh_router::QualityGate;
use neuromesh_task::{normalize_keyword, TaskSignatureExtractor};
use parking_lot::RwLock;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct McpToolHandler {
    graph: Arc<NeuralProjectGraph>,
    activator: Arc<ContextActivator>,
    expansion_engine: Arc<ExpansionEngine>,
    memory_db: Arc<MemoryDatabase>,
    working_memory: Arc<parking_lot::RwLock<WorkingMemory>>,
    mycelium: Arc<MyceliumCache>,
    packet_cache: PacketDetailCache,
    #[cfg(feature = "embeddings")]
    semantic_cache: McpSemanticCache,
    client_id: RwLock<Option<String>>,
    /// External graph MCP (CBM/Graphify). None = native only.
    graph_proxy: RwLock<Option<Arc<tokio::sync::Mutex<GraphProxySession>>>>,
    graph_proxy_fallback_native: RwLock<bool>,
    graph_backend_label: RwLock<String>,
}

struct ToolTelemetry {
    prefix: &'static str,
    task_id: String,
    mode: String,
    command: Option<String>,
    tokens_before: usize,
    tokens_after: usize,
    token_reduction_pct: f32,
    nodes_after: usize,
    expansions_count: usize,
    cache_hit: bool,
    latency_ms: u64,
    success: bool,
}

impl ToolTelemetry {
    fn new(prefix: &'static str, task_id: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            prefix,
            task_id: task_id.into(),
            mode: mode.into(),
            command: None,
            tokens_before: 0,
            tokens_after: 0,
            token_reduction_pct: 0.0,
            nodes_after: 0,
            expansions_count: 0,
            cache_hit: false,
            latency_ms: 0,
            success: true,
        }
    }
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
            packet_cache: PacketDetailCache::new(),
            #[cfg(feature = "embeddings")]
            semantic_cache: McpSemanticCache::default(),
            client_id: RwLock::new(None),
            graph_proxy: RwLock::new(None),
            graph_proxy_fallback_native: RwLock::new(true),
            graph_backend_label: RwLock::new("native".into()),
        }
    }

    /// Attach an external graph backend. Native graph remains loaded for fallback and other tools.
    pub fn with_graph_proxy(
        self,
        session: GraphProxySession,
        fallback_native: bool,
        backend_label: impl Into<String>,
    ) -> Self {
        *self.graph_proxy.write() = Some(Arc::new(tokio::sync::Mutex::new(session)));
        *self.graph_proxy_fallback_native.write() = fallback_native;
        *self.graph_backend_label.write() = backend_label.into();
        self
    }

    /// Hot-swap or attach graph proxy at runtime (monitor config changes).
    pub async fn connect_graph_proxy(
        &self,
        session: GraphProxySession,
        fallback_native: bool,
        backend_label: impl Into<String>,
    ) {
        *self.graph_proxy.write() = Some(Arc::new(tokio::sync::Mutex::new(session)));
        *self.graph_proxy_fallback_native.write() = fallback_native;
        *self.graph_backend_label.write() = backend_label.into();
    }

    pub fn clear_graph_proxy(&self) {
        *self.graph_proxy.write() = None;
        *self.graph_backend_label.write() = "native".into();
    }

    pub fn graph_backend_label(&self) -> String {
        self.graph_backend_label.read().clone()
    }

    pub fn graph_proxy_active(&self) -> bool {
        self.graph_proxy.read().is_some()
    }

    pub fn set_client_id(&self, client: String) {
        *self.client_id.write() = Some(client);
    }

    pub fn graph(&self) -> &Arc<NeuralProjectGraph> {
        &self.graph
    }

    pub fn warmup_persisted_learning(&self) {
        let pid = self.graph.project_id();
        let _ = crate::learning::warmup_project_learning(&self.memory_db, &self.graph, &pid);
    }

    pub fn persist_project_state(&self) {
        let _ = self.graph.save_persisted_if_ready();
    }

    pub fn flush_on_shutdown(&self) {
        self.persist_project_state();
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

    /// One row per MCP process so `neuromesh usage` / monitor show the handshake.
    pub fn record_session_telemetry(&self) {
        static ONCE: AtomicBool = AtomicBool::new(false);
        if ONCE.swap(true, Ordering::Relaxed) {
            return;
        }
        self.emit_telemetry(ToolTelemetry::new(
            "mcp-session",
            "MCP initialize",
            "mcp_session",
        ));
    }

    fn emit_telemetry(&self, tel: ToolTelemetry) {
        neuromesh_observability::record_metadata(neuromesh_core::OptimizationMetadata {
            request_id: telemetry_request_id(tel.prefix),
            task_id: Some(tel.task_id),
            project_id: self.graph.project_id(),
            mode: tel.mode,
            tokens_before: tel.tokens_before,
            tokens_after: tel.tokens_after,
            token_reduction_pct: tel.token_reduction_pct,
            nodes_before: self.graph.stats().total_nodes,
            nodes_after: tel.nodes_after,
            expansions_count: tel.expansions_count,
            cache_hit: tel.cache_hit,
            provider: "Cursor / Claude MCP".to_string(),
            model: "Frontier Model".to_string(),
            latency_ms: tel.latency_ms,
            success: tel.success,
            timestamp: chrono::Utc::now(),
            workspace_path: self.graph.workspace_root().map(|p| p.display().to_string()),
            surface: "mcp".into(),
            client_id: self.client_id.read().clone(),
            command: tel.command,
        });
    }

    #[cfg(test)]
    pub(crate) fn expire_packet_for_test(&self, packet_id: &str) {
        self.packet_cache.expire_for_test(packet_id);
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
        #[cfg(test)]
        if name == "__neuromesh_panic_probe" {
            panic!("intentional panic from tool probe");
        }
        match name {
            // 1. Task-conditioned evidence packet (seed files + fill-budget connectors)
            "get_context_packet" | "neuromesh_get_context" | "activate_context" => {
                if name != "get_context_packet" {
                    warn_deprecated_get_context(name);
                }
                let start_time = std::time::Instant::now();
                let task_desc = read_task_description(arguments)?;
                let requested_mode = parse_optimization_mode(arguments.get("mode"))?;
                self.wait_for_index()?;
                let detail = ResponseDetail::parse(arguments["response_detail"].as_str());

                let mut signature = TaskSignatureExtractor::extract(&task_desc);
                apply_client_seed_signals(&mut signature, arguments);
                let auto_extract = read_auto_extract_keywords(arguments);
                let server_inferred =
                    apply_server_assisted_defaults(&mut signature, &task_desc, auto_extract);
                if requested_mode == neuromesh_core::OptimizationMode::MaxQuality {
                    if let Ok(episodes) = self
                        .memory_db
                        .find_similar_episodes(&self.graph.project_id(), &task_desc)
                    {
                        for ep in episodes.into_iter().filter(|e| e.success).take(3) {
                            for name in &ep.successful_path {
                                if name.len() < 3 {
                                    continue;
                                }
                                if !signature.identifiers.iter().any(|i| i == name) {
                                    signature.identifiers.push(name.clone());
                                }
                            }
                        }
                    }
                }
                let gate = QualityGate::evaluate(&signature, requested_mode);

                let proxy = self.graph_proxy.read().clone();
                let fallback_native = *self.graph_proxy_fallback_native.read();
                let backend_label = self.graph_backend_label();
                if let Some(proxy) = proxy {
                    let ctx = build_proxy_search_context(&signature);
                    match proxy.lock().await.build_context_packet(&ctx, 8).await {
                        Ok(proxy_packet) => {
                            let elapsed_ms = start_time.elapsed().as_millis() as u64;
                            let value = proxy_evidence_response(
                                &proxy_packet,
                                &signature,
                                &gate,
                                detail,
                                elapsed_ms,
                                &backend_label,
                            );
                            self.emit_telemetry(ToolTelemetry {
                                tokens_before: 0,
                                tokens_after: proxy_packet.packet_tokens,
                                token_reduction_pct: 0.0,
                                nodes_after: proxy_packet.files.len(),
                                latency_ms: elapsed_ms,
                                ..ToolTelemetry::new(
                                    "mcp-proxy",
                                    task_desc.chars().take(50).collect::<String>(),
                                    gate.effective_mode.to_string(),
                                )
                            });
                            return Ok(value);
                        }
                        Err(e) if fallback_native => {
                            tracing::warn!("graph proxy failed, using native graph: {e}");
                        }
                        Err(e) => {
                            return Err(NeuroMeshError::Config(format!("graph proxy failed: {e}")));
                        }
                    }
                }

                #[cfg(feature = "embeddings")]
                let mut semantic_query_vec: Option<Vec<f32>> = None;
                #[cfg(feature = "embeddings")]
                {
                    let emb_cfg = Config::load().embeddings;
                    if emb_cfg.enabled && emb_cfg.semantic_cache_enabled {
                        packet_cache_begin();
                        if let Ok(query_vec) = embed_query_cached(&emb_cfg, &task_desc) {
                            semantic_query_vec = Some(query_vec.clone());
                            let cache_key = SemanticCacheKey {
                                graph_generation: self.graph.generation(),
                                graph_digest: graph_digest(&self.graph),
                                model: emb_cfg.model,
                                dim: emb_cfg.matryoshka_dim,
                                project_id: self.graph.project_id().0.clone(),
                            };
                            if let Some(hit) = self.semantic_cache.lookup(
                                &cache_key,
                                &query_vec,
                                emb_cfg.semantic_cache_min_cosine,
                            ) {
                                packet_cache_end();
                                let new_id = PacketDetailCache::new_packet_id();
                                let elapsed_ms = start_time.elapsed().as_millis() as u64;
                                self.emit_telemetry(ToolTelemetry {
                                    tokens_before: hit.details.workspace_tokens,
                                    tokens_after: hit.details.tokens_packet,
                                    token_reduction_pct: 0.0,
                                    nodes_after: hit.details.files.len(),
                                    latency_ms: elapsed_ms,
                                    cache_hit: true,
                                    ..ToolTelemetry::new(
                                        "mcp",
                                        task_desc.chars().take(50).collect::<String>(),
                                        hit.details.effective_mode.clone(),
                                    )
                                });
                                return Ok(apply_semantic_cache_hit(
                                    &self.packet_cache,
                                    &self.graph.project_id().0,
                                    hit.response,
                                    hit.details,
                                    new_id,
                                ));
                            }
                        }
                        // Keep packet_cache scope open through activate_tiered (nested begin/end).
                    }
                }

                let view =
                    self.activator
                        .activate_tiered(&self.graph, &signature, gate.effective_mode);
                #[cfg(feature = "embeddings")]
                {
                    let emb_cfg = Config::load().embeddings;
                    if emb_cfg.enabled && emb_cfg.semantic_cache_enabled {
                        packet_cache_end();
                    }
                }
                if gate.effective_mode == neuromesh_core::OptimizationMode::MaxQuality {
                    self.prefetch_mycelium(&view);
                }

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

                let files = collect_file_entries(&view, self.expansion_engine.registry());
                let symbols = collect_symbols(&view);
                let packet_id = PacketDetailCache::new_packet_id();
                let build = ContextBuild {
                    packet_id,
                    signature: &signature,
                    gate: &gate,
                    view: &view,
                    files: &files,
                    symbols: &symbols,
                    workspace_tokens,
                    selected_raw,
                    packet_tokens: opt_tokens,
                    vs_workspace,
                    vs_selected,
                    elapsed_ms,
                    index_meta: self.graph.index_meta(),
                    server_inferred_keywords: server_inferred,
                };

                self.emit_telemetry(ToolTelemetry {
                    tokens_before: workspace_tokens,
                    tokens_after: opt_tokens,
                    token_reduction_pct: vs_workspace,
                    nodes_after: view.active_nodes.len(),
                    latency_ms: elapsed_ms,
                    ..ToolTelemetry::new(
                        "mcp",
                        task_desc.chars().take(50).collect::<String>(),
                        gate.effective_mode.to_string(),
                    )
                });

                Ok({
                    let value = cache_and_build(
                        &self.packet_cache,
                        &self.graph.project_id().0,
                        &build,
                        detail,
                    );
                    #[cfg(feature = "embeddings")]
                    {
                        let emb_cfg = Config::load().embeddings;
                        if emb_cfg.enabled && emb_cfg.semantic_cache_enabled {
                            if let Some(query_vec) = semantic_query_vec
                                .or_else(|| embed_query_cached(&emb_cfg, &task_desc).ok())
                            {
                                let cache_key = SemanticCacheKey {
                                    graph_generation: self.graph.generation(),
                                    graph_digest: graph_digest(&self.graph),
                                    model: emb_cfg.model,
                                    dim: emb_cfg.matryoshka_dim,
                                    project_id: self.graph.project_id().0.clone(),
                                };
                                let details = build.to_details();
                                self.semantic_cache.insert(
                                    emb_cfg.semantic_cache_entries,
                                    cache_key,
                                    query_vec,
                                    value.clone(),
                                    details,
                                    detail,
                                );
                            }
                        }
                    }
                    value
                })
            }

            // 2b. Expand a packet gap file (cheap skeleton, no blind Grep)
            "neuromesh_expand_gap" => {
                let start_time = std::time::Instant::now();
                let file_path = arguments["path"]
                    .as_str()
                    .or_else(|| arguments["file_path"].as_str())
                    .unwrap_or("");
                if file_path.is_empty() {
                    return Ok(json!({
                        "success": false,
                        "error": "path is required (from packet_gaps or unsure)"
                    }));
                }
                let node_id = NodeId::from_file_path(file_path);
                let content_opt = self
                    .graph
                    .get_node(&node_id)
                    .and_then(|n| self.graph.read_source(&n.file_path));
                let content_opt = match content_opt {
                    Some(content) => Some(content),
                    None => match self.read_workspace_source(file_path) {
                        Ok(content) => content,
                        Err(err) => return Ok(json!({ "success": false, "error": err })),
                    },
                };
                if let Some(content) = content_opt {
                    let res = CodeSkeletonizer::skeletonize(file_path, &content, &HashSet::new());
                    let cap = arguments["token_cap"].as_u64().unwrap_or(200) as usize;
                    let skeleton = if res.skeleton_tokens > cap {
                        res.skeleton_code
                            .lines()
                            .take(40)
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        res.skeleton_code
                    };
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    self.emit_telemetry(ToolTelemetry {
                        tokens_before: res.original_tokens,
                        tokens_after: res.skeleton_tokens.min(cap),
                        token_reduction_pct: res.token_reduction_pct,
                        nodes_after: 1,
                        latency_ms: elapsed_ms,
                        command: Some("expand_gap".into()),
                        ..ToolTelemetry::new("mcp-gap", format!("Gap: {file_path}"), "expand_gap")
                    });
                    Ok(json!({
                        "success": true,
                        "path": file_path,
                        "skeleton": skeleton,
                        "skeleton_tokens": res.skeleton_tokens.min(cap),
                        "latency_ms": elapsed_ms,
                    }))
                } else {
                    Ok(json!({ "success": false, "error": "file not found" }))
                }
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
                    .and_then(|n| self.graph.read_source(&n.file_path));
                let content_opt = match content_opt {
                    Some(content) => Some(content),
                    None => match self.read_workspace_source(file_path) {
                        Ok(content) => content,
                        Err(err) => return Ok(json!({ "error": err })),
                    },
                };

                if let Some(content) = content_opt {
                    let res = CodeSkeletonizer::skeletonize(file_path, &content, &active_symbols);
                    for fold in &res.folds {
                        self.expansion_engine
                            .registry()
                            .register_fold(std::path::PathBuf::from(file_path), fold.clone());
                    }
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;

                    self.emit_telemetry(ToolTelemetry {
                        tokens_before: res.original_tokens,
                        tokens_after: res.skeleton_tokens,
                        token_reduction_pct: res.token_reduction_pct,
                        nodes_after: 1,
                        latency_ms: elapsed_ms,
                        ..ToolTelemetry::new(
                            "mcp-skel",
                            format!("Skeleton: {file_path}"),
                            "Genetic Slicing",
                        )
                    });

                    Ok(json!({
                        "file_path": file_path,
                        "skeleton_code": res.skeleton_code,
                        "original_tokens": res.original_tokens,
                        "skeleton_tokens": res.skeleton_tokens,
                        "token_reduction_pct": format!("{:.1}%", res.token_reduction_pct),
                        "introns_folded": res.introns_folded,
                        "folds": fold_descriptors_from_skeleton(&res.folds)
                    }))
                } else {
                    Ok(json!({ "error": format!("File not found or unreadable: {}", file_path) }))
                }
            }

            "neuromesh_explain_packet"
            | "explain_packet"
            | "neuromesh_get_context_details"
            | "get_context_details" => {
                let start_time = std::time::Instant::now();
                let packet_id = arguments["packet_id"]
                    .as_str()
                    .or_else(|| arguments["id"].as_str())
                    .unwrap_or("")
                    .trim();
                if packet_id.is_empty() {
                    return Ok(json!({
                        "error": "neuromesh_explain_packet requires packet_id from get_context"
                    }));
                }
                match self.packet_cache.get(packet_id) {
                    Ok(details) => {
                        let include = read_string_list(arguments, "include");
                        let graph = if include.iter().any(|s| s == "graph") {
                            Some(json!(self.graph.stats()))
                        } else {
                            None
                        };
                        let body = explain_packet(&details, &include, graph);
                        self.emit_telemetry(ToolTelemetry {
                            latency_ms: start_time.elapsed().as_millis() as u64,
                            ..ToolTelemetry::new(
                                "mcp-explain",
                                format!("Explain: {packet_id}"),
                                "explain_packet",
                            )
                        });
                        Ok(body)
                    }
                    Err(err) => Ok(json!({ "error": err.message(packet_id) })),
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
                    self.emit_telemetry(ToolTelemetry {
                        tokens_before: fold.restored_tokens,
                        tokens_after: fold.restored_tokens,
                        nodes_after: 1,
                        expansions_count: 1,
                        cache_hit,
                        latency_ms: elapsed_ms,
                        ..ToolTelemetry::new(
                            "mcp-exp",
                            format!("Expand: {}", fold.fold_id),
                            "expand_fold",
                        )
                    });
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

                    self.emit_telemetry(ToolTelemetry {
                        tokens_after: audit.added_tokens,
                        nodes_after: 1,
                        expansions_count: 1,
                        latency_ms: elapsed_ms,
                        ..ToolTelemetry::new(
                            "mcp-exp",
                            format!("Expand: {node_id_str}"),
                            "expand_fold",
                        )
                    });

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

                self.emit_telemetry(ToolTelemetry {
                    nodes_after: nodes.len(),
                    latency_ms: elapsed_ms,
                    ..ToolTelemetry::new("mcp-sym", format!("Search: {query}"), "symbol_search")
                });

                Ok(json!({
                    "query": query,
                    "matches_count": nodes.len(),
                    "results": nodes
                }))
            }

            // 5. Get Graph Dependencies & Synaptic Weights
            "neuromesh_get_dependencies" | "get_dependency_graph" => {
                let start_time = std::time::Instant::now();
                let symbol_or_path = arguments["symbol_or_path"]
                    .as_str()
                    .or_else(|| arguments["file_path"].as_str())
                    .unwrap_or("");

                let resolved = self.graph.resolve_best(symbol_or_path);
                let Some(node) = resolved else {
                    self.emit_telemetry(ToolTelemetry {
                        latency_ms: start_time.elapsed().as_millis() as u64,
                        success: false,
                        ..ToolTelemetry::new(
                            "mcp-deps",
                            format!("Deps: {symbol_or_path}"),
                            "get_dependencies",
                        )
                    });
                    return Ok(json!({
                        "target": symbol_or_path,
                        "connected_neighbors_count": 0,
                        "dependencies": [],
                        "error": "Symbol or file not found in the project graph"
                    }));
                };

                let neighbors = self.graph.get_neighbor_views(&node.id);
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: neighbors.len(),
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new(
                        "mcp-deps",
                        format!("Deps: {symbol_or_path}"),
                        "get_dependencies",
                    )
                });
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
                let start_time = std::time::Instant::now();
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
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: result.hops.len(),
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new("mcp-trace", format!("Trace: {query}"), "trace")
                });
                Ok(json!(result))
            }

            "neuromesh_analyze_impact" => {
                let start_time = std::time::Instant::now();
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["symbol_or_path"].as_str())
                    .unwrap_or("");
                let depth = arguments["depth"].as_u64().unwrap_or(3) as usize;
                let result = self.graph.analyze_impact(query, depth);
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: result.affected_symbols.len(),
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new("mcp-impact", format!("Impact: {query}"), "analyze_impact")
                });
                Ok(json!(result))
            }

            "neuromesh_get_architecture" => {
                let start_time = std::time::Instant::now();
                let summary = self.graph.architecture_summary();
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: summary.file_count,
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new("mcp-arch", "Architecture summary", "get_architecture")
                });
                Ok(json!(summary))
            }

            // 6. Record Feedback & Trigger Synaptic STDP Plasticity Learning
            "neuromesh_record_feedback" => {
                let start_time = std::time::Instant::now();
                let success = read_bool(&arguments["task_success"], true);
                let touched_nodes = read_string_list(arguments, "touched_nodes");

                let mut path: Vec<NodeId> = Vec::new();
                let mut resolved: Vec<Value> = Vec::new();
                let mut weight_deltas: Vec<Value> = Vec::new();
                for node_name in &touched_nodes {
                    let Some(node) = self.graph.resolve_feedback_node(node_name) else {
                        resolved.push(json!({
                            "query": node_name,
                            "resolved": false
                        }));
                        continue;
                    };
                    let delta = self.graph.reinforce_node_access(&node.id, success);
                    self.graph
                        .record_neural_spike(node.id.clone(), true, success);
                    path.push(node.id.clone());
                    resolved.push(json!({
                        "query": node_name,
                        "resolved": true,
                        "node_id": node.id.as_str(),
                        "name": node.name,
                        "path": node.file_path.to_string_lossy().replace('\\', "/"),
                        "relevance_delta": delta,
                        "access_count": node.access_count.saturating_add(1),
                        "base_relevance": (node.base_relevance + delta).min(3.0)
                    }));
                    weight_deltas.push(json!({
                        "node_id": node.id.as_str(),
                        "relevance_delta": delta
                    }));
                }
                self.graph.apply_stdp_on_path(&path);
                self.graph.reinforce_path(&path, success);
                self.graph.reinforce_callee_edges(&path, success);
                self.record_mycelium_path(&path);

                let pid = self.graph.project_id();
                let learning_episodes_in_store = self
                    .memory_db
                    .list_project_episodes(&pid)
                    .map(|eps| eps.len())
                    .unwrap_or(0);
                let mut episode_id = String::new();
                if !path.is_empty() {
                    let summary = if success {
                        format!("successful edit touching {}", path.len())
                    } else {
                        format!("failed attempt touching {}", path.len())
                    };
                    let episode = neuromesh_memory::EpisodicRecord::new(
                        pid.clone(),
                        format!("feedback:{summary}"),
                        if success { "success" } else { "failure" }.into(),
                        summary,
                        path.clone(),
                        touched_nodes.clone(),
                        success,
                        0,
                    );
                    episode_id = episode.id.clone();
                    let _ = self.memory_db.save_episodic_record(&episode);
                }
                if !episode_id.is_empty() {
                    self.graph.mark_learning_episode_applied(&episode_id);
                }
                let _ = self.graph.save_persisted_if_ready();

                self.emit_telemetry(ToolTelemetry {
                    nodes_after: path.len(),
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new(
                        "mcp-fb",
                        format!("Feedback: {} nodes", path.len()),
                        "record_feedback",
                    )
                });
                Ok(json!({
                    "status": "Feedback recorded",
                    "success": success,
                    "stdp_learning_applied": true,
                    "path_nodes": path.len(),
                    "resolved_nodes": resolved,
                    "weight_deltas": weight_deltas,
                    "episode_saved_this_call": !path.is_empty(),
                    "learning_episodes_in_store": learning_episodes_in_store,
                    "persisted_to": "graph.bin",
                    "episodes_recorded": if path.is_empty() { 0 } else { 1 },
                    "updated_graph_stats": self.graph.stats()
                }))
            }

            // 7. Get Project Memory & Conventions
            "neuromesh_get_project_memory" => {
                let start_time = std::time::Instant::now();
                let pid = self.graph.project_id();
                let facts = self.memory_db.get_project_facts(&pid)?;
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: facts.len(),
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new("mcp-mem", "Project memory", "get_project_memory")
                });
                Ok(json!({
                    "project_id": pid.0,
                    "facts_count": facts.len(),
                    "project_memory": facts
                }))
            }

            // 8. Search Episodic Memory
            "search_memory" | "get_previous_solution" => {
                let start_time = std::time::Instant::now();
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["task_similarity_query"].as_str())
                    .unwrap_or("");
                let pid = self.graph.project_id();
                let episodes = self.memory_db.find_similar_episodes(&pid, query)?;
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: episodes.len(),
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new(
                        "mcp-episodic",
                        format!("Memory: {query}"),
                        "search_memory",
                    )
                });
                Ok(json!({
                    "query": query,
                    "episodes_count": episodes.len(),
                    "episodes": episodes
                }))
            }

            // 9. Get Task State
            "get_task_state" => {
                let start_time = std::time::Instant::now();
                let wm = self.working_memory.read().clone();
                self.emit_telemetry(ToolTelemetry {
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new("mcp-wm", "Task state", "get_task_state")
                });
                Ok(json!({ "working_memory": wm }))
            }

            // 9b. Read per-node learning weights (falsifiable feedback observability)
            "neuromesh_get_node_weights" => {
                let start_time = std::time::Instant::now();
                let query = arguments["query"]
                    .as_str()
                    .or_else(|| arguments["symbol"].as_str())
                    .or_else(|| arguments["path"].as_str())
                    .unwrap_or("");
                if query.is_empty() {
                    return Ok(json!({
                        "error": "query, symbol, or path is required"
                    }));
                }
                match self.graph.node_learning_profile(query) {
                    Some(profile) => {
                        self.emit_telemetry(ToolTelemetry {
                            latency_ms: start_time.elapsed().as_millis() as u64,
                            ..ToolTelemetry::new("mcp-weights", query, "get_node_weights")
                        });
                        Ok(json!(profile))
                    }
                    None => Ok(json!({
                        "error": format!("node not found for query: {query}")
                    })),
                }
            }

            // 10. Get System Stats & Biomimetic Health
            "neuromesh_get_stats" => {
                let start_time = std::time::Instant::now();
                let stats = self.graph.stats();
                let index_state = self.graph.index_state();
                self.emit_telemetry(ToolTelemetry {
                    nodes_after: stats.total_nodes,
                    latency_ms: start_time.elapsed().as_millis() as u64,
                    ..ToolTelemetry::new("mcp-stats", "Project stats", "get_stats")
                });
                Ok(json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "project_id": self.graph.project_id().0,
                    "index_state": index_state,
                    "generation": stats.generation,
                    "ready": index_state == IndexState::Ready,
                    "graph_stats": stats,
                    "biomimetic_engine": self.biomimetic_report()
                }))
            }

            _ => Ok(json!({ "error": format!("Unknown tool: {}", name) })),
        }
    }

    fn wait_for_index(&self) -> Result<()> {
        if self.graph.index_state() == IndexState::Ready {
            return Ok(());
        }
        let state = self.graph.wait_until_indexed(Duration::from_secs(5));
        if self.graph.stats().total_nodes > 0 || state == IndexState::Ready {
            return Ok(());
        }
        Err(NeuroMeshError::Config(format!(
            "indexing_in_progress: index_state={state:?}"
        )))
    }

    fn read_workspace_source(
        &self,
        file_path: &str,
    ) -> std::result::Result<Option<String>, String> {
        let Some(root) = self.graph.workspace_root() else {
            return Ok(None);
        };
        match neuromesh_index::read_workspace_file(&root, Path::new(file_path)) {
            Ok(src) => Ok(Some(src)),
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("outside workspace") {
                    Err(msg)
                } else {
                    Ok(None)
                }
            }
        }
    }
}

fn parse_optimization_mode(value: Option<&Value>) -> Result<OptimizationMode> {
    let Some(value) = value else {
        return Ok(OptimizationMode::Balanced);
    };
    if value.is_null() {
        return Ok(OptimizationMode::Balanced);
    }
    let Some(raw) = value.as_str() else {
        return Err(NeuroMeshError::Config("unknown mode".into()));
    };
    match raw.trim() {
        "" | "balanced" => Ok(OptimizationMode::Balanced),
        "max_quality" => Ok(OptimizationMode::MaxQuality),
        "max_savings" => Ok(OptimizationMode::MaxSavings),
        other => Err(NeuroMeshError::Config(format!(
            "unknown mode: {other}; expected balanced, max_quality, or max_savings"
        ))),
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
            "get_context_packet requires a prompt (query, task_description, prompt, or task)"
                .into(),
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

static DEPRECATED_GET_CONTEXT_WARNED: AtomicBool = AtomicBool::new(false);

fn warn_deprecated_get_context(name: &str) {
    if !DEPRECATED_GET_CONTEXT_WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("[neuromesh] tool {name} is deprecated; use get_context_packet");
    }
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

fn apply_client_seed_signals(signature: &mut neuromesh_core::TaskSignature, arguments: &Value) {
    for kw in read_string_list(arguments, "keywords") {
        push_unique_normalized(&mut signature.client_keywords, &kw);
    }
    for term in read_string_list(arguments, "expansion") {
        push_unique_normalized(&mut signature.client_expansion, &term);
    }
    for hint in read_string_list(arguments, "path_hints") {
        let hint = hint.trim().replace('\\', "/");
        if hint.is_empty() {
            continue;
        }
        if !signature.client_path_hints.iter().any(|h| h == &hint) {
            signature.client_path_hints.push(hint);
        }
    }
    for et in read_string_list(arguments, "entity_types") {
        let et = et.trim().to_lowercase();
        if et.is_empty() {
            continue;
        }
        if !signature.client_entity_types.iter().any(|e| e == &et) {
            signature.client_entity_types.push(et);
        }
    }
    if let Some(intent) = arguments.get("intent").and_then(Value::as_str) {
        let intent = intent.trim();
        if !intent.is_empty() {
            signature.client_intent = Some(intent.to_string());
        }
    }
    if let Some(engine) = arguments.get("engine").and_then(Value::as_str) {
        signature.engine_override = SeedEngineId::parse(engine);
    }
}

/// Native assisted by default: infer keywords/expansion (FILL-ONLY-MISSING dedupe).
fn apply_server_assisted_defaults(
    signature: &mut neuromesh_core::TaskSignature,
    prompt: &str,
    enabled: bool,
) -> bool {
    apply_auto_extract_keywords(signature, prompt, enabled)
}

fn read_auto_extract_keywords(arguments: &Value) -> bool {
    let config = Config::load();
    if let Some(v) = arguments.get("auto_extract_keywords") {
        return read_bool(v, config.seed_resolution.effective_auto_extract());
    }
    config.seed_resolution.effective_auto_extract()
}

/// Proxy CBM search uses extracted terms — reuse server infer when client fields are sparse.
fn build_proxy_search_context(signature: &TaskSignature) -> ProxySearchContext {
    use neuromesh_context::retrieval::infer_assisted_seed_signals;
    use neuromesh_task::{is_prompt_stopword, normalize_prompt_tokens};

    let mut ctx = ProxySearchContext::from_task_signature(signature);
    if signature.client_keywords.is_empty() && signature.client_expansion.is_empty() {
        let (kw, exp) = infer_assisted_seed_signals(&signature.raw_prompt);
        for k in kw {
            push_unique_normalized(&mut ctx.client_keywords, &k);
        }
        for e in exp {
            push_unique_normalized(&mut ctx.client_expansion, &e);
        }
    }
    for token in normalize_prompt_tokens(&signature.raw_prompt) {
        if token.len() < 4 || is_prompt_stopword(&token.to_lowercase()) {
            continue;
        }
        push_unique_normalized(&mut ctx.client_keywords, &token);
    }
    ctx.client_keywords.truncate(12);
    ctx
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
        let args = json!({
            "task_description": "How does start_job enqueue_job?",
            "mode": "max_quality"
        });
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
            assert!(packet.get("packet_id").is_some());
            assert!(packet.get("coverage").is_some());
            let via_text = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "text": "How does start_job enqueue_job?" }),
                )
                .await
                .expect("text alias should populate the prompt");
            assert!(via_text.get("packet_id").is_some());
            assert!(via_text.get("coverage").is_some());
        });
    }

    fn first_fold_id(packet: &Value) -> Option<String> {
        if let Some(files) = packet["files"].as_array() {
            for file in files {
                if let Some(folds) = file["folds"].as_array() {
                    for fold in folds {
                        if let Some(id) = fold["fold_id"].as_str().or_else(|| fold.as_str()) {
                            return Some(id.to_string());
                        }
                    }
                }
            }
        }
        packet["evidence_packet"]["fold_ids"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
    }

    fn handler_for(graph: Arc<NeuralProjectGraph>) -> McpToolHandler {
        let registry = Arc::new(ReversibleContextRegistry::new());
        McpToolHandler::new(
            graph,
            Arc::new(ContextActivator::new(registry.clone())),
            Arc::new(ExpansionEngine::new(registry)),
            Arc::new(MemoryDatabase::open_in_memory().unwrap()),
            Arc::new(RwLock::new(WorkingMemory::default())),
        )
    }

    fn fold_sample_graph() -> Arc<NeuralProjectGraph> {
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
        graph
    }

    fn job_sample_graph() -> Arc<NeuralProjectGraph> {
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
        graph
    }

    #[test]
    fn expand_fold_accepts_query_from_next_actions() {
        let handler = handler_for(fold_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does handle_tool_call work?" }),
                )
                .await
                .expect("packet");
            let fold_id = first_fold_id(&packet).expect("fold id from packet");
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

    #[test]
    fn minimal_response_never_contains_fold_original_body() {
        let handler = handler_for(fold_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does handle_tool_call work?" }),
                )
                .await
                .unwrap();
            let dumped = serde_json::to_string(&packet).unwrap();
            assert!(
                !dumped.contains("original_body"),
                "minimal packet leaked fold bodies: {dumped}"
            );
            assert!(
                first_fold_id(&packet).is_some(),
                "expected fold descriptors"
            );
            let skeleton = handler
                .handle_tool_call(
                    "neuromesh_get_file_skeleton",
                    &json!({
                        "file_path": "src/tools.rs",
                        "active_symbols": ["handle_tool_call"]
                    }),
                )
                .await
                .unwrap();
            let skel = serde_json::to_string(&skeleton).unwrap();
            assert!(
                !skel.contains("original_body"),
                "skeleton leaked fold bodies: {skel}"
            );
            assert!(
                !skel.contains("let w = 4"),
                "skeleton must not ship the folded body: {skel}"
            );
        });
    }

    #[test]
    fn minimal_response_metadata_stays_under_token_budget() {
        let handler = handler_for(fold_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does handle_tool_call work?" }),
                )
                .await
                .unwrap();
            let tokens = crate::response::metadata_tokens(&packet);
            assert!(
                tokens <= crate::response::MINIMAL_METADATA_BUDGET,
                "minimal metadata {tokens} exceeds budget"
            );
        });
    }

    #[test]
    fn partial_coverage_keeps_missing_seeds_and_next_action() {
        let handler = handler_for(job_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does start_job Twig View::render enqueue_job?" }),
                )
                .await
                .unwrap();
            assert_eq!(packet["coverage"], "partial");
            let missing = packet["missing"].as_array().expect("missing seeds");
            let joined = missing
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                joined.contains("Twig") || joined.contains("View"),
                "missing={missing:?}"
            );
            assert_eq!(packet["next"]["tool"], "neuromesh_search_symbols");
            assert!(packet.get("seeds").is_none());
            assert!(packet.get("next_actions").is_none());
            assert!(packet.get("inactive_hints").is_none());
        });
    }

    #[test]
    fn complete_coverage_omits_empty_diagnostics() {
        let handler = handler_for(job_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does start_job enqueue_job?" }),
                )
                .await
                .unwrap();
            assert_eq!(packet["coverage"], "no_recorded_gap");
            assert!(packet.get("missing").is_none());
            assert!(packet.get("next").is_none());
            assert!(packet.get("membrane_state").is_none());
            assert!(packet.get("physarum_used").is_none());
        });
    }

    #[test]
    fn diagnostic_details_can_be_fetched_by_packet_id() {
        let handler = handler_for(job_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does start_job enqueue_job?" }),
                )
                .await
                .unwrap();
            let packet_id = packet["packet_id"].as_str().unwrap();
            let details = handler
                .handle_tool_call(
                    "neuromesh_explain_packet",
                    &json!({ "packet_id": packet_id }),
                )
                .await
                .unwrap();
            assert_eq!(details["packet_id"], packet_id);
            assert!(details.get("seeds").is_some());
            assert!(details.get("budget").is_some());
            assert!(details.get("membrane").is_some());
            assert!(details.get("graph").is_none());
            let dumped = serde_json::to_string(&details).unwrap();
            assert!(!dumped.contains("original_body"));
        });
    }

    #[test]
    fn expired_packet_id_returns_clear_error() {
        let handler = handler_for(job_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does start_job enqueue_job?" }),
                )
                .await
                .unwrap();
            let packet_id = packet["packet_id"].as_str().unwrap().to_string();
            handler.expire_packet_for_test(&packet_id);
            let err = handler
                .handle_tool_call(
                    "neuromesh_explain_packet",
                    &json!({ "packet_id": packet_id }),
                )
                .await
                .unwrap();
            let message = err["error"].as_str().unwrap_or("");
            assert!(
                message.contains("expired"),
                "expected expired error, got {err}"
            );
        });
    }

    #[test]
    fn expand_fold_still_restores_original_body() {
        let handler = handler_for(fold_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let packet = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({ "task": "How does handle_tool_call work?" }),
                )
                .await
                .unwrap();
            let fold_id = first_fold_id(&packet).expect("fold");
            let expanded = handler
                .handle_tool_call("neuromesh_expand_fold", &json!({ "fold_id": fold_id }))
                .await
                .unwrap();
            assert_eq!(expanded["success"], true);
            assert!(expanded["original_body"]
                .as_str()
                .unwrap_or("")
                .contains("let w = 4"));
        });
    }

    #[test]
    fn skeleton_rejects_absolute_path_outside_workspace() {
        let root = std::env::temp_dir().join(format!(
            "nm-skel-abs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("lib.rs"), "pub fn ok() {}\n").unwrap();
        let graph = fold_sample_graph();
        graph.set_workspace(&root);
        let handler = handler_for(graph);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            #[cfg(unix)]
            let outside = "/etc/passwd";
            #[cfg(windows)]
            let outside = r"C:\Windows\win.ini";
            let err = handler
                .handle_tool_call(
                    "neuromesh_get_file_skeleton",
                    &json!({ "file_path": outside, "active_symbols": [] }),
                )
                .await
                .unwrap();
            let dumped = serde_json::to_string(&err).unwrap();
            assert!(
                dumped.contains("outside workspace"),
                "expected confinement error, got {dumped}"
            );
            assert!(
                !dumped.contains("root:"),
                "must never return outside file content: {dumped}"
            );
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skeleton_rejects_parent_directory_traversal() {
        let root = std::env::temp_dir().join(format!(
            "nm-skel-trav-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("lib.rs"), "pub fn ok() {}\n").unwrap();
        let graph = fold_sample_graph();
        graph.set_workspace(&root);
        let handler = handler_for(graph);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = handler
                .handle_tool_call(
                    "neuromesh_get_file_skeleton",
                    &json!({
                        "file_path": "../../../../etc/passwd",
                        "active_symbols": []
                    }),
                )
                .await
                .unwrap();
            let dumped = serde_json::to_string(&err).unwrap();
            assert!(
                dumped.contains("outside workspace"),
                "expected traversal rejection, got {dumped}"
            );
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn skeleton_rejects_symlink_escaping_workspace() {
        let root = std::env::temp_dir().join(format!(
            "nm-skel-link-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let link = root.join("passwd-link");
        let _ = std::os::unix::fs::symlink("/etc/passwd", &link);
        let graph = fold_sample_graph();
        graph.set_workspace(&root);
        let handler = handler_for(graph);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = handler
                .handle_tool_call(
                    "neuromesh_get_file_skeleton",
                    &json!({ "file_path": "passwd-link", "active_symbols": [] }),
                )
                .await
                .unwrap();
            let dumped = serde_json::to_string(&err).unwrap();
            assert!(
                dumped.contains("outside workspace") || dumped.contains("not found"),
                "expected symlink escape rejection, got {dumped}"
            );
            assert!(!dumped.contains("root:"));
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skeleton_allows_canonical_file_inside_workspace() {
        let root = std::env::temp_dir().join(format!(
            "nm-skel-ok-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("lib.rs"), "pub fn ok() { 1 }\n").unwrap();
        let graph = fold_sample_graph();
        graph.set_workspace(&root);
        let handler = handler_for(graph);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let ok = handler
                .handle_tool_call(
                    "neuromesh_get_file_skeleton",
                    &json!({ "file_path": "src/lib.rs", "active_symbols": ["ok"] }),
                )
                .await
                .unwrap();
            assert!(
                ok.get("error").is_none(),
                "inside-workspace file must be readable: {ok}"
            );
            assert!(ok["skeleton_code"].as_str().unwrap_or("").contains("ok"));
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_mode_is_tool_error() {
        let handler = handler_for(job_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = handler
                .handle_tool_call(
                    "neuromesh_get_context",
                    &json!({
                        "task": "How does start_job enqueue_job?",
                        "mode": "invalid-mode"
                    }),
                )
                .await;
            assert!(err.is_err(), "invalid mode must not silently fall back");
            let msg = err.unwrap_err().to_string();
            assert!(msg.contains("unknown mode"), "{msg}");
        });
    }

    #[test]
    fn get_stats_exposes_index_readiness() {
        let handler = handler_for(job_sample_graph());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let stats = handler
                .handle_tool_call("neuromesh_get_stats", &json!({}))
                .await
                .unwrap();
            assert_eq!(stats["index_state"], "ready");
            assert_eq!(stats["ready"], true);
            assert!(stats.get("generation").is_some());
        });
    }
}
