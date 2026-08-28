use crate::activation::{SpreadingActivation, SpreadingActivationConfig};
use crate::edge::{PheromoneConfig, PheromoneEngine};
use crate::intern::{
    capture_inbound_pending, index_tokens, infer_workspace_root, insert_indexed_node,
    rebuild_indexes, remove_file_nodes_locked, GraphData, GraphSnapshot, LegacyGraphData,
    PendingRel,
};
use crate::node::NodeFactory;
use crate::physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
use crate::query::{
    path_hint_matches, tokenize, ArchitecturePackage, ArchitectureSummary, ImpactResult,
    NeighborView, SearchHit, TraceDirection, TraceHop, TraceResult,
};
use crate::synapse::{StdpConfig, SynapticPlasticityEngine};
use chrono::Utc;
use neuromesh_core::{
    hmvc_app_prefix, is_core_source_path, is_json_schema_path, is_low_priority_source_path,
    is_name_collision_decoy, name_match_specificity, ContextEdge, ContextNode, EdgeConfidence,
    EdgeId, EdgeType, IndexMeta, NodeId, NodeType, ProjectId, UnresolvedRef,
};
use neuromesh_index::{FileFingerprint, IndexedFile, ScanReport};
use neuromesh_parser::AstAnalysisResult;
use parking_lot::{Condvar, Mutex, RwLock};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Bump when parser/linker output changes; older snapshots re-parse on load.
pub const GRAPH_PARSER_EPOCH: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub file_nodes: usize,
    pub symbol_nodes: usize,
    pub average_pheromone_weight: f32,
    pub high_conductance_synapses: usize,
    pub atrophied_synapses: usize,
    #[serde(default)]
    pub resolved_calls: usize,
    #[serde(default)]
    pub resolved_imports: usize,
    #[serde(default)]
    pub unresolved_count: usize,
    #[serde(default)]
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborLearningWeight {
    pub node_id: String,
    pub name: String,
    pub path: String,
    pub edge_type: String,
    pub pheromone_weight: f32,
    pub reinforcement_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLearningProfile {
    pub node_id: String,
    pub name: String,
    pub path: String,
    pub access_count: u64,
    pub base_relevance: f32,
    pub learning_bonus: f32,
    pub neighbors: Vec<NeighborLearningWeight>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    Loading,
    Indexing,
    Ready,
    Failed,
}

struct IndexGate {
    state: Mutex<IndexState>,
    cv: Condvar,
}

impl IndexGate {
    fn new(state: IndexState) -> Self {
        Self {
            state: Mutex::new(state),
            cv: Condvar::new(),
        }
    }

    fn get(&self) -> IndexState {
        *self.state.lock()
    }

    fn set(&self, state: IndexState) {
        *self.state.lock() = state;
        if matches!(state, IndexState::Ready | IndexState::Failed) {
            self.cv.notify_all();
        }
    }

    fn wait_ready(&self, timeout: Duration) -> IndexState {
        let mut guard = self.state.lock();
        if matches!(*guard, IndexState::Ready | IndexState::Failed) {
            return *guard;
        }
        self.cv.wait_for(&mut guard, timeout);
        *guard
    }
}

#[derive(Clone)]
pub struct NeuralProjectGraph {
    project_id: Arc<RwLock<ProjectId>>,
    inner: Arc<RwLock<GraphData>>,
    pheromone_engine: Arc<PheromoneEngine>,
    activation_engine: Arc<SpreadingActivation>,
    synaptic_engine: Arc<RwLock<SynapticPlasticityEngine>>,
    physarum_solver: Arc<PhysarumSolver>,
    index_gate: Arc<IndexGate>,
}

impl NeuralProjectGraph {
    pub fn new(project_id: ProjectId) -> Self {
        Self {
            project_id: Arc::new(RwLock::new(project_id)),
            inner: Arc::new(RwLock::new(GraphData::default())),
            pheromone_engine: Arc::new(PheromoneEngine::new(PheromoneConfig::default())),
            activation_engine: Arc::new(SpreadingActivation::new(
                SpreadingActivationConfig::default(),
            )),
            synaptic_engine: Arc::new(RwLock::new(SynapticPlasticityEngine::new(
                StdpConfig::default(),
            ))),
            physarum_solver: Arc::new(PhysarumSolver::new(PhysarumConfig::default())),
            index_gate: Arc::new(IndexGate::new(IndexState::Ready)),
        }
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id.read().clone()
    }

    pub fn set_project_id(&self, new_id: ProjectId) {
        *self.project_id.write() = new_id;
    }

    pub fn index_state(&self) -> IndexState {
        self.index_gate.get()
    }

    pub fn mark_index_loading(&self) {
        self.index_gate.set(IndexState::Loading);
    }

    pub fn mark_index_indexing(&self) {
        self.index_gate.set(IndexState::Indexing);
    }

    pub fn mark_index_ready(&self) {
        self.index_gate.set(IndexState::Ready);
    }

    pub fn mark_index_failed(&self) {
        self.index_gate.set(IndexState::Failed);
    }

    /// Wait until the first index finishes. A persist-loaded graph is already Ready.
    pub fn wait_until_indexed(&self, timeout: Duration) -> IndexState {
        let state = self.index_gate.get();
        if matches!(state, IndexState::Ready | IndexState::Failed) {
            return state;
        }
        self.index_gate.wait_ready(timeout)
    }

    pub fn clear(&self, new_project_id: Option<ProjectId>) {
        if let Some(new_id) = new_project_id {
            *self.project_id.write() = new_id;
        }
        let mut data = self.inner.write();
        data.mesh.clear();
        data.name_to_nodes.clear();
        data.file_to_nodes.clear();
        data.token_to_nodes.clear();
        data.pending.clear();
        data.unresolved.clear();
        data.impl_index.clear();
        data.export_index.clear();
        data.file_hashes.clear();
        data.file_fingerprints.clear();
        data.source_overlay.clear();
        data.stale_files.clear();
        data.generation = data.generation.saturating_add(1);
        data.indexed_at = Some(chrono::Utc::now());
    }

    pub fn add_file_node(&self, file: &IndexedFile, content: Option<String>) -> ContextNode {
        let current_pid = self.project_id.read().clone();
        let node = NodeFactory::create_file_node(
            current_pid,
            file.relative_path.clone(),
            file.token_count,
            file.blake3_hash.clone(),
            content,
        );

        let mut data = self.inner.write();
        insert_indexed_node(&mut data, node.clone());
        if let Some(stem) = file.relative_path.file_stem().and_then(|s| s.to_str()) {
            index_tokens(&mut data, &node.id, stem);
        }

        node
    }

    pub fn add_symbol_node(
        &self,
        file_path: &Path,
        symbol_name: &str,
        node_type: NodeType,
        signature: Option<String>,
        line_range: std::ops::Range<usize>,
        token_cost: usize,
    ) -> ContextNode {
        self.add_symbol_node_with_parent(
            file_path,
            symbol_name,
            node_type,
            signature,
            line_range,
            token_cost,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_symbol_node_with_parent(
        &self,
        file_path: &Path,
        symbol_name: &str,
        node_type: NodeType,
        signature: Option<String>,
        line_range: std::ops::Range<usize>,
        token_cost: usize,
        parent: Option<String>,
    ) -> ContextNode {
        let current_pid = self.project_id.read().clone();
        let node = NodeFactory::create_symbol_node(
            current_pid,
            file_path.to_path_buf(),
            node_type,
            symbol_name.to_string(),
            signature,
            line_range,
            token_cost,
            parent.clone(),
        );

        let mut data = self.inner.write();
        insert_indexed_node(&mut data, node.clone());

        node
    }

    pub fn add_edge(&self, source: NodeId, target: NodeId, edge_type: EdgeType) -> ContextEdge {
        self.add_edge_with_confidence(source, target, edge_type, EdgeConfidence::Proven)
    }

    pub fn add_edge_with_confidence(
        &self,
        source: NodeId,
        target: NodeId,
        edge_type: EdgeType,
        confidence: EdgeConfidence,
    ) -> ContextEdge {
        if source == target {
            let current_pid = self.project_id.read().clone();
            return self.pheromone_engine.create_edge_with_confidence(
                current_pid,
                source,
                target,
                edge_type,
                confidence,
            );
        }
        let current_pid = self.project_id.read().clone();
        let edge = self.pheromone_engine.create_edge_with_confidence(
            current_pid,
            source.clone(),
            target.clone(),
            edge_type,
            confidence,
        );

        let mut data = self.inner.write();
        data.mesh.insert_edge(edge.clone());

        edge
    }

    /// Ingests AST analysis results for an indexed file and creates symbols and edges
    pub fn ingest_ast(&self, file: &IndexedFile, ast: &AstAnalysisResult) {
        let content = std::fs::read_to_string(&file.full_path).ok();
        self.ingest_file(file, ast, content.as_deref());
    }

    pub fn ingest_file(&self, file: &IndexedFile, ast: &AstAnalysisResult, content: Option<&str>) {
        self.ingest_file_keep(file, ast, content, true);
    }

    fn ingest_file_keep(
        &self,
        file: &IndexedFile,
        ast: &AstAnalysisResult,
        content: Option<&str>,
        keep_source: bool,
    ) {
        let rel = file.relative_path.to_string_lossy().replace('\\', "/");
        let current_pid = self.project_id.read().clone();
        let file_node = NodeFactory::create_file_node(
            current_pid.clone(),
            file.relative_path.clone(),
            file.token_count,
            file.blake3_hash.clone(),
            None,
        );
        let mut symbol_nodes = Vec::with_capacity(ast.symbols.len() + ast.design_tokens.len());
        for sym in &ast.symbols {
            let token_cost = sym
                .line_range
                .end
                .saturating_sub(sym.line_range.start)
                .max(1)
                * 8;
            symbol_nodes.push((
                NodeFactory::create_symbol_node(
                    current_pid.clone(),
                    file.relative_path.clone(),
                    sym.symbol_type,
                    sym.name.clone(),
                    sym.signature.clone(),
                    sym.line_range.clone(),
                    token_cost,
                    sym.parent.clone(),
                ),
                EdgeType::Contains,
                sym.exported,
            ));
        }
        let existing_names: HashSet<String> =
            ast.symbols.iter().map(|s| s.name.to_lowercase()).collect();
        for token in &ast.design_tokens {
            if existing_names.contains(&token.to_lowercase()) {
                continue;
            }
            symbol_nodes.push((
                NodeFactory::create_symbol_node(
                    current_pid.clone(),
                    file.relative_path.clone(),
                    NodeType::StyleToken,
                    token.clone(),
                    Some(format!("Token: {token}")),
                    1..2,
                    5,
                    None,
                ),
                EdgeType::References,
                false,
            ));
        }

        let mut data = self.inner.write();
        if data
            .file_hashes
            .get(&rel)
            .is_some_and(|stored| stored == &file.blake3_hash)
        {
            return;
        }
        let inbound = capture_inbound_pending(&data, &file.relative_path);
        remove_file_nodes_locked(&mut data, &file.relative_path);

        let file_id = file_node.id.clone();
        insert_indexed_node(&mut data, file_node);
        if let Some(stem) = file.relative_path.file_stem().and_then(|s| s.to_str()) {
            index_tokens(&mut data, &file_id, stem);
        }

        let mut local_symbols: HashMap<String, NodeId> = HashMap::new();
        for (node, edge_type, exported) in symbol_nodes {
            let id = node.id.clone();
            let name_key = node.name.to_lowercase();
            insert_indexed_node(&mut data, node);
            local_symbols.insert(name_key.clone(), id.clone());
            if exported {
                data.export_index
                    .entry(name_key)
                    .or_default()
                    .push(id.clone());
            }
            if file_id != id {
                let edge = self.pheromone_engine.create_edge_with_confidence(
                    current_pid.clone(),
                    file_id.clone(),
                    id,
                    edge_type,
                    EdgeConfidence::Proven,
                );
                let _ = data.mesh.insert_edge(edge);
            }
        }

        for export_name in &ast.exports {
            if let Some(id) = local_symbols.get(&export_name.to_lowercase()) {
                let entry = data
                    .export_index
                    .entry(export_name.to_lowercase())
                    .or_default();
                if !entry.iter().any(|existing| existing == id) {
                    entry.push(id.clone());
                }
            }
        }

        for pending in &ast.relationships {
            data.pending.push(PendingRel {
                source_file: file.relative_path.clone(),
                source_symbol: pending.source_symbol.clone(),
                target_symbol: pending.target_symbol.clone(),
                relationship: pending.relationship,
                target_file_hint: pending.target_file_hint.clone(),
                receiver_hint: pending.receiver_hint.clone(),
            });
        }
        data.pending.extend(inbound);
        data.file_hashes
            .insert(rel.clone(), file.blake3_hash.clone());
        data.file_fingerprints
            .insert(rel.clone(), file.fingerprint());
        data.indexed_at = Some(chrono::Utc::now());
        if keep_source {
            if let Some(src) = content {
                data.source_overlay.insert(rel, src.to_string());
            }
        } else {
            data.source_overlay.remove(&rel);
        }
    }

    /// Resolve queued import/call edges after symbols exist. Safe to call after every file
    /// and again at the end of a workspace scan.
    pub fn finalize_links(&self) {
        let pending = {
            let mut data = self.inner.write();
            data.unresolved.clear();
            std::mem::take(&mut data.pending)
        };

        let mut leftover = Vec::new();
        let mut unresolved = Vec::new();
        let mut imports = Vec::new();
        let mut rest = Vec::new();
        for rel in pending {
            if rel.relationship == EdgeType::Imports || rel.relationship == EdgeType::DependsOn {
                imports.push(rel);
            } else {
                rest.push(rel);
            }
        }
        for rel in imports.into_iter().chain(rest) {
            let file_id =
                NodeId::from_file_path(&rel.source_file.to_string_lossy().replace('\\', "/"));
            let imported_files = self.imported_files_of(&file_id);

            let linked = match rel.relationship {
                EdgeType::Imports => {
                    match self.resolve_ranked(
                        &rel.target_symbol,
                        rel.target_file_hint.as_deref(),
                        None,
                    ) {
                        Some((target, confidence)) if target != file_id => {
                            self.add_edge_with_confidence(
                                file_id.clone(),
                                target,
                                EdgeType::Imports,
                                confidence,
                            );
                            true
                        }
                        Some(_) => true,
                        None => {
                            if let Some(hint) = &rel.target_file_hint {
                                if let Some(target_file) = self.resolve_file_hint(hint) {
                                    self.add_edge_with_confidence(
                                        file_id,
                                        target_file,
                                        EdgeType::DependsOn,
                                        EdgeConfidence::Likely,
                                    );
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                    }
                }
                EdgeType::Calls => {
                    let source = self
                        .resolve_unique(
                            &rel.source_symbol,
                            Some(&rel.source_file.to_string_lossy()),
                        )
                        .unwrap_or_else(|| file_id.clone());
                    // Overlay templates (`hello` → `theme/default/hello.twig`) must
                    // bind the file before the stem can steal another symbol.
                    // Inbound relink stores the callee's source path as a hint;
                    // that is not a template overlay (stem `lib` ≠ `persist_me`).
                    let hinted_file = rel
                        .target_file_hint
                        .as_deref()
                        .filter(|hint| template_stem_hint(hint, &rel.target_symbol))
                        .and_then(|hint| self.resolve_file_hint(hint))
                        .filter(|target| *target != source);
                    if let Some(target) = hinted_file {
                        self.add_edge_with_confidence(
                            source,
                            target,
                            EdgeType::Calls,
                            EdgeConfidence::Likely,
                        );
                        true
                    } else if let Some((target, confidence)) = self.resolve_call_ranked(
                        &rel.target_symbol,
                        &rel.source_file,
                        &imported_files,
                        rel.receiver_hint.as_deref(),
                    ) {
                        if target != source {
                            self.add_edge_with_confidence(
                                source,
                                target,
                                EdgeType::Calls,
                                confidence,
                            );
                        }
                        true
                    } else if let Some((target, _)) = self.resolve_ranked(
                        &rel.target_symbol,
                        Some(&rel.source_file.to_string_lossy()),
                        Some(&imported_files),
                    ) {
                        if target != source {
                            self.add_edge_with_confidence(
                                source.clone(),
                                target,
                                EdgeType::Calls,
                                EdgeConfidence::Likely,
                            );
                        }
                        unresolved.push(UnresolvedRef {
                            name: rel.target_symbol.clone(),
                            from: rel.source_symbol.clone(),
                            from_file: rel.source_file.clone(),
                            reason: "ambiguous call kept as likely".into(),
                            relationship: EdgeType::Calls,
                        });
                        true
                    } else if let Some(hint) = rel.target_file_hint.as_deref() {
                        if let Some(target) = self.resolve_file_hint(hint) {
                            if target != source {
                                self.add_edge_with_confidence(
                                    source,
                                    target,
                                    EdgeType::Calls,
                                    EdgeConfidence::Likely,
                                );
                            }
                            true
                        } else {
                            unresolved.push(UnresolvedRef {
                                name: rel.target_symbol.clone(),
                                from: rel.source_symbol.clone(),
                                from_file: rel.source_file.clone(),
                                reason: "no unique or impl-scoped target".into(),
                                relationship: EdgeType::Calls,
                            });
                            false
                        }
                    } else {
                        unresolved.push(UnresolvedRef {
                            name: rel.target_symbol.clone(),
                            from: rel.source_symbol.clone(),
                            from_file: rel.source_file.clone(),
                            reason: "no unique or impl-scoped target".into(),
                            relationship: EdgeType::Calls,
                        });
                        false
                    }
                }
                other => {
                    if let Some((target, confidence)) = self.resolve_ranked(
                        &rel.target_symbol,
                        rel.target_file_hint.as_deref(),
                        None,
                    ) {
                        self.add_edge_with_confidence(file_id, target, other, confidence);
                        true
                    } else if let Some(hint) = rel.target_file_hint.as_deref() {
                        if let Some(target) = self.resolve_file_hint(hint) {
                            self.add_edge_with_confidence(
                                file_id,
                                target,
                                other,
                                EdgeConfidence::Likely,
                            );
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            };

            if !linked && rel.relationship != EdgeType::Calls {
                leftover.push(rel);
            }
        }

        let mut data = self.inner.write();
        data.pending = leftover;
        data.unresolved = unresolved;
        data.generation = data.generation.saturating_add(1);
        data.indexed_at = Some(chrono::Utc::now());
    }

    fn imported_files_of(&self, file_id: &NodeId) -> HashSet<PathBuf> {
        let mut files = HashSet::new();
        for (neighbor, edge) in self.get_connected_neighbors(file_id) {
            if edge.edge_type == EdgeType::Imports || edge.edge_type == EdgeType::DependsOn {
                if let Some(node) = self.get_node(&neighbor) {
                    files.insert(node.file_path.clone());
                }
            }
        }
        files
    }

    pub fn file_id_for_path(&self, path: &Path) -> Option<NodeId> {
        let normalized = path.to_string_lossy().replace('\\', "/");
        let data = self.inner.read();
        for (stored, ids) in &data.file_to_nodes {
            if stored.to_string_lossy().replace('\\', "/") != normalized {
                continue;
            }
            if let Some(id) = ids.iter().find(|id| {
                data.mesh
                    .node(id)
                    .is_some_and(|n| n.node_type == NodeType::File)
            }) {
                return Some(id.clone());
            }
        }
        None
    }

    pub fn get_node(&self, id: &NodeId) -> Option<ContextNode> {
        let data = self.inner.read();
        data.mesh.node(id).cloned()
    }

    pub fn get_all_nodes(&self) -> Vec<ContextNode> {
        let data = self.inner.read();
        data.mesh.nodes().cloned().collect()
    }

    pub fn get_all_nodes_for_viz(&self) -> Vec<ContextNode> {
        let data = self.inner.read();
        data.mesh
            .nodes()
            .map(ContextNode::without_content)
            .collect()
    }

    pub fn get_nodes_map(&self) -> HashMap<NodeId, ContextNode> {
        let data = self.inner.read();
        data.mesh.nodes_map()
    }

    pub fn get_edges_map(&self) -> HashMap<EdgeId, ContextEdge> {
        let data = self.inner.read();
        data.mesh.edges_map()
    }

    pub fn find_nodes_by_name(&self, query: &str) -> Vec<ContextNode> {
        self.search_symbols(query, 32)
            .into_iter()
            .filter_map(|hit| self.get_node(&hit.id))
            .collect()
    }

    pub fn search_symbols(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let limit = limit.clamp(1, 80);
        let query_lower = query.to_lowercase();
        let query_tokens = tokenize(query);
        let data = self.inner.read();
        let mut scored: HashMap<NodeId, (f32, String)> = HashMap::new();

        let mut exact_type_hit = false;
        if let Some(ids) = data.name_to_nodes.get(&query_lower) {
            for id in ids {
                let class_like = data.mesh.node(id).is_some_and(|n| {
                    matches!(
                        n.node_type,
                        NodeType::Class | NodeType::Component | NodeType::Api
                    )
                });
                if class_like {
                    exact_type_hit = true;
                }
                // Exact names outrank every fuzzy token. Class/interface/trait
                // nodes get a higher floor so `HttpKernel` cannot lose to
                // `HttpUtils` / `getKernel` once the corpus is large.
                let base = if class_like { 240.0 } else { 120.0 };
                scored.insert(id.clone(), (base, "exact_name".into()));
            }
        }

        if query_lower.len() >= 3 {
            for (name, ids) in data.name_to_nodes.range(query_lower.clone()..) {
                if !name.starts_with(&query_lower) {
                    break;
                }
                if name == &query_lower {
                    continue;
                }
                let spec = name_match_specificity(&query_lower, name);
                let score = 80.0 + 12.0 * spec;
                for id in ids {
                    scored.entry(id.clone()).or_insert((score, "prefix".into()));
                }
            }
            if !exact_type_hit {
                for (name, ids) in &data.name_to_nodes {
                    if name == &query_lower || name.starts_with(&query_lower) {
                        continue;
                    }
                    if name.contains(&query_lower) {
                        let spec = name_match_specificity(&query_lower, name);
                        let score = 48.0 + 28.0 * spec;
                        for id in ids {
                            scored
                                .entry(id.clone())
                                .or_insert((score, "substring".into()));
                        }
                    }
                }
            }
        }

        if !exact_type_hit {
            for token in &query_tokens {
                if let Some(ids) = data.token_to_nodes.get(token) {
                    for id in ids {
                        scored.entry(id.clone()).or_insert((74.0, "token".into()));
                    }
                }
            }
        }

        if query.contains('/') || query.contains('\\') || query.contains('.') {
            let needle = query_lower.replace('\\', "/");
            for (path, ids) in &data.file_to_nodes {
                let path_s = path.to_string_lossy().replace('\\', "/").to_lowercase();
                if path_s.contains(&needle) || needle.contains(&path_s) && path_s.len() > 4 {
                    for id in ids {
                        scored.entry(id.clone()).or_insert((70.0, "path".into()));
                    }
                }
            }
        }

        let mut hits: Vec<SearchHit> = scored
            .into_iter()
            .filter_map(|(id, (score, reason))| {
                data.mesh.node(&id).map(|node| {
                    SearchHit::from_node(node, score + ranking_bonus(node, query), reason)
                })
            })
            .collect();

        hits.sort_by(|a, b| {
            let exact = |reason: &str| reason == "exact_name";
            exact(&b.match_reason)
                .cmp(&exact(&a.match_reason))
                .then_with(|| type_search_rank(&b.node_type).cmp(&type_search_rank(&a.node_type)))
                .then_with(|| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.name.len().cmp(&b.name.len()))
        });
        hits.dedup_by(|a, b| a.id == b.id);
        hits.truncate(limit);
        hits
    }

    pub fn resolve_best(&self, query: &str) -> Option<ContextNode> {
        if query.is_empty() {
            return None;
        }
        if let Some(node) = self.get_node(&NodeId::new(query)) {
            return Some(node);
        }
        if query.contains('/') || query.contains('\\') || query.contains('.') {
            let file_id = NodeId::from_file_path(&query.replace('\\', "/"));
            if let Some(node) = self.get_node(&file_id) {
                return Some(node);
            }
        }
        if let Some((owner, name)) = query.rsplit_once("::") {
            let owner = owner.trim();
            let name = name.trim();
            if !owner.is_empty() && !name.is_empty() {
                let typed = {
                    let data = self.inner.read();
                    let name_l = name.to_lowercase();
                    data.name_to_nodes.get(&name_l).and_then(|ids| {
                        ids.iter().find_map(|id| {
                            data.mesh.node(id).and_then(|n| {
                                let parent_ok = n
                                    .parent
                                    .as_deref()
                                    .is_some_and(|p| p.eq_ignore_ascii_case(owner));
                                let stem_ok = file_stem_equals(&n.file_path, owner);
                                (parent_ok || stem_ok).then(|| id.clone())
                            })
                        })
                    })
                };
                if let Some(id) = typed {
                    return self.get_node(&id);
                }
                if let Some(id) = self.resolve_unique(name, Some(owner)) {
                    return self.get_node(&id);
                }
                let key = format!("{}::{}", owner.to_lowercase(), name.to_lowercase());
                let impl_id = {
                    let data = self.inner.read();
                    data.impl_index
                        .get(&key)
                        .and_then(|ids| ids.first().cloned())
                };
                if let Some(id) = impl_id {
                    return self.get_node(&id);
                }
            }
        }
        self.search_symbols(query, 5)
            .into_iter()
            .next()
            .and_then(|hit| self.get_node(&hit.id))
    }

    pub fn resolve_unique(&self, name: &str, file_hint: Option<&str>) -> Option<NodeId> {
        let name_lower = name.to_lowercase();
        let data = self.inner.read();
        let ids = data
            .name_to_nodes
            .get(&name_lower)
            .cloned()
            .unwrap_or_default();
        if ids.is_empty() {
            return None;
        }
        if ids.len() == 1 {
            return ids.into_iter().next();
        }
        if let Some(hint) = file_hint {
            let hinted: Vec<NodeId> = ids
                .iter()
                .filter(|id| {
                    data.mesh
                        .node(id)
                        .is_some_and(|n| path_hint_matches(&n.file_path, hint))
                })
                .cloned()
                .collect();
            if hinted.len() == 1 {
                return hinted.into_iter().next();
            }
            return None;
        }
        None
    }

    pub fn resolve_file_hint(&self, hint: &str) -> Option<NodeId> {
        let data = self.inner.read();
        let hint_norm = normalize_path_hint(hint);
        let path_like = looks_like_file_path_hint(hint);
        let mut matches = Vec::new();
        for (path, ids) in &data.file_to_nodes {
            let path_s = normalize_path_hint(&path.to_string_lossy());
            let ok = if path_like {
                path_s == hint_norm
                    || path_s.ends_with(&format!("/{hint_norm}"))
                    || path_s.ends_with(&hint_norm)
            } else {
                path_hint_matches(path, hint)
            };
            if !ok {
                continue;
            }
            for id in ids {
                if data
                    .mesh
                    .node(id)
                    .is_some_and(|n| n.node_type == NodeType::File)
                {
                    matches.push(id.clone());
                }
            }
        }
        if matches.len() == 1 {
            return matches.into_iter().next();
        }
        if path_like && matches.is_empty() {
            if let Some(stem) = Path::new(&hint_norm)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
            {
                for (path, ids) in &data.file_to_nodes {
                    let path_s = normalize_path_hint(&path.to_string_lossy());
                    if path_s.ends_with(&format!("/{stem}"))
                        || path_s.ends_with(&stem)
                        || path_s.contains(&format!("/{stem}."))
                    {
                        for id in ids {
                            if data
                                .mesh
                                .node(id)
                                .is_some_and(|n| n.node_type == NodeType::File)
                            {
                                matches.push(id.clone());
                            }
                        }
                    }
                }
            }
        }
        if matches.len() == 1 {
            return matches.into_iter().next();
        }
        if path_like && matches.len() > 1 {
            let suffix: Vec<NodeId> = matches
                .into_iter()
                .filter(|id| {
                    data.mesh.node(id).is_some_and(|n| {
                        normalize_path_hint(&n.file_path.to_string_lossy()).ends_with(&hint_norm)
                    })
                })
                .collect();
            if suffix.len() == 1 {
                return suffix.into_iter().next();
            }
        }
        None
    }

    fn resolve_call_target(
        &self,
        name: &str,
        source_file: &Path,
        imported_files: &HashSet<PathBuf>,
    ) -> Option<NodeId> {
        let name_lower = name.to_lowercase();
        let data = self.inner.read();
        let ids = data
            .name_to_nodes
            .get(&name_lower)
            .cloned()
            .unwrap_or_default();
        if ids.is_empty() {
            return None;
        }

        let same_file: Vec<NodeId> = ids
            .iter()
            .filter(|id| {
                data.mesh
                    .node(id)
                    .is_some_and(|n| n.file_path == source_file && n.node_type != NodeType::File)
            })
            .cloned()
            .collect();
        if same_file.len() == 1 {
            return same_file.into_iter().next();
        }

        let imported: Vec<NodeId> = ids
            .iter()
            .filter(|id| {
                data.mesh
                    .node(id)
                    .is_some_and(|n| imported_files.contains(&n.file_path))
            })
            .cloned()
            .collect();
        if imported.len() == 1 {
            return imported.into_iter().next();
        }

        let src_pkg = package_name(source_file);
        let same_crate: Vec<NodeId> = ids
            .iter()
            .filter(|id| {
                data.mesh.node(id).is_some_and(|n| {
                    n.node_type != NodeType::File && package_name(&n.file_path) == src_pkg
                })
            })
            .cloned()
            .collect();
        if same_crate.len() == 1 {
            return same_crate.into_iter().next();
        }

        let src_ext = path_ext(source_file);
        let same_lang: Vec<NodeId> = ids
            .iter()
            .filter(|id| {
                data.mesh.node(id).is_some_and(|n| {
                    n.node_type != NodeType::File
                        && !is_fixture_path(&n.file_path)
                        && path_ext(&n.file_path) == src_ext
                })
            })
            .cloned()
            .collect();
        if same_lang.len() == 1 {
            return same_lang.into_iter().next();
        }

        if ids.len() == 1 {
            return ids.into_iter().next();
        }
        None
    }

    pub fn resolve_ranked(
        &self,
        name: &str,
        file_hint: Option<&str>,
        imported_files: Option<&HashSet<PathBuf>>,
    ) -> Option<(NodeId, EdgeConfidence)> {
        let file_hint = file_hint.filter(|hint| !hint.contains("::"));
        if let Some(hint) = file_hint {
            if let Some(found) = self.resolve_export(name, hint) {
                return Some(found);
            }
        }
        if let Some(id) = self.resolve_unique(name, file_hint) {
            return Some((id, EdgeConfidence::Proven));
        }
        let name_lower = name.to_lowercase();
        let data = self.inner.read();
        let ids = data
            .name_to_nodes
            .get(&name_lower)
            .cloned()
            .unwrap_or_default();
        if ids.is_empty() {
            return None;
        }
        if let Some(imported) = imported_files {
            let hinted: Vec<NodeId> = ids
                .iter()
                .filter(|id| {
                    data.mesh
                        .node(id)
                        .is_some_and(|n| imported.contains(&n.file_path))
                })
                .cloned()
                .collect();
            if hinted.len() == 1 {
                return Some((hinted.into_iter().next().unwrap(), EdgeConfidence::Proven));
            }
            if hinted.len() > 1 {
                drop(data);
                return self.pick_dominant_candidate(&hinted, name);
            }
        }
        if let Some(hint) = file_hint {
            let hinted: Vec<NodeId> = ids
                .iter()
                .filter(|id| {
                    data.mesh
                        .node(id)
                        .is_some_and(|n| path_hint_matches(&n.file_path, hint))
                })
                .cloned()
                .collect();
            if !hinted.is_empty() {
                drop(data);
                return self.pick_dominant_candidate(&hinted, name);
            }
        }
        if !ids.is_empty() {
            drop(data);
            return self.pick_dominant_candidate(&ids, name);
        }
        None
    }

    fn pick_dominant_candidate(
        &self,
        ids: &[NodeId],
        query: &str,
    ) -> Option<(NodeId, EdgeConfidence)> {
        if ids.is_empty() {
            return None;
        }
        if ids.len() == 1 {
            return Some((ids[0].clone(), EdgeConfidence::Proven));
        }
        let mut ranked: Vec<(f32, NodeId)> = ids
            .iter()
            .map(|id| {
                let mut score = 0.0;
                if let Some(node) = self.get_node(id) {
                    score += ranking_bonus(&node, query);
                    if is_crate_path(&node.file_path) {
                        score += 12.0;
                    }
                    if node
                        .signature
                        .as_deref()
                        .is_some_and(|sig| sig.contains("pub "))
                    {
                        score += 6.0;
                    }
                    if let Some(range) = &node.line_range {
                        score += range.end.saturating_sub(range.start) as f32;
                    }
                    score += self.get_connected_neighbors(id).len() as f32 * 4.0;
                }
                (score, id.clone())
            })
            .collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let best = ranked[0].1.clone();
        let dominant = ranked.len() == 1 || ranked[0].0 >= ranked[1].0 + 8.0;
        Some((
            best,
            if dominant {
                EdgeConfidence::Proven
            } else {
                EdgeConfidence::Likely
            },
        ))
    }

    fn resolve_call_ranked(
        &self,
        name: &str,
        source_file: &Path,
        imported_files: &HashSet<PathBuf>,
        receiver_hint: Option<&str>,
    ) -> Option<(NodeId, EdgeConfidence)> {
        if let Some(hint) = receiver_hint {
            if let Some(field) = hint.strip_prefix("field:") {
                if let Some(id) =
                    self.resolve_method_on_field(name, field, source_file, imported_files)
                {
                    return Some((id, EdgeConfidence::Proven));
                }
            }
            let type_name = hint
                .strip_prefix("impl:")
                .or_else(|| hint.strip_prefix("type:"))
                .unwrap_or(hint);
            let keys = [
                format!("{}::{}", type_name.to_lowercase(), name.to_lowercase()),
                format!(
                    "{}::{}",
                    pinia_store_type_from_alias(type_name).to_lowercase(),
                    name.to_lowercase()
                ),
            ];
            let data = self.inner.read();
            for key in &keys {
                if let Some(ids) = data.impl_index.get(key) {
                    if ids.len() == 1 {
                        return Some((ids[0].clone(), EdgeConfidence::Proven));
                    }
                    if let Some(id) = ids.iter().find(|id| {
                        data.mesh
                            .node(id)
                            .is_some_and(|n| n.file_path == source_file)
                    }) {
                        return Some((id.clone(), EdgeConfidence::Proven));
                    }
                    if !ids.is_empty() {
                        return Some((ids[0].clone(), EdgeConfidence::Likely));
                    }
                }
            }
        }

        if let Some(id) = self.resolve_call_target(name, source_file, imported_files) {
            return Some((id, EdgeConfidence::Proven));
        }
        if let Some(prefix) = hmvc_app_prefix(source_file) {
            let prefix_slash = format!("{prefix}/");
            let name_lower = name.to_lowercase();
            let in_app: Vec<NodeId> = {
                let data = self.inner.read();
                data.name_to_nodes
                    .get(&name_lower)
                    .into_iter()
                    .flatten()
                    .filter(|id| {
                        data.mesh.node(id).is_some_and(|n| {
                            n.file_path
                                .to_string_lossy()
                                .replace('\\', "/")
                                .contains(&prefix_slash)
                        })
                    })
                    .cloned()
                    .collect()
            };
            if !in_app.is_empty() {
                return self.pick_dominant_candidate(&in_app, name);
            }
        }
        self.resolve_ranked(
            name,
            Some(&source_file.to_string_lossy()),
            Some(imported_files),
        )
    }

    fn resolve_method_on_field(
        &self,
        method: &str,
        field: &str,
        source_file: &Path,
        imported_files: &HashSet<PathBuf>,
    ) -> Option<NodeId> {
        let data = self.inner.read();
        let mut type_names: Vec<String> = data
            .mesh
            .nodes()
            .filter(|n| n.node_type == NodeType::Class || n.node_type == NodeType::Symbol)
            .filter(|n| field_matches_type(field, &n.name))
            .filter(|n| imported_files.contains(&n.file_path) || n.file_path == source_file)
            .map(|n| n.name.clone())
            .collect();
        type_names.sort();
        type_names.dedup();
        let mut hits: Vec<NodeId> = Vec::new();
        for ty in &type_names {
            let key = format!("{}::{}", ty.to_lowercase(), method.to_lowercase());
            if let Some(ids) = data.impl_index.get(&key) {
                hits.extend(ids.iter().cloned());
            }
        }
        hits.sort_by(|a, b| a.0.cmp(&b.0));
        hits.dedup();
        if hits.len() == 1 {
            return hits.into_iter().next();
        }
        let imported_hits: Vec<NodeId> = hits
            .iter()
            .filter(|id| {
                data.mesh
                    .node(id)
                    .is_some_and(|n| imported_files.contains(&n.file_path))
            })
            .cloned()
            .collect();
        if imported_hits.len() == 1 {
            return imported_hits.into_iter().next();
        }
        None
    }

    fn resolve_export(&self, name: &str, file_hint: &str) -> Option<(NodeId, EdgeConfidence)> {
        let name_lower = name.to_lowercase();
        let data = self.inner.read();
        let ids = data.export_index.get(&name_lower)?;
        let hinted: Vec<NodeId> = ids
            .iter()
            .filter(|id| {
                data.mesh
                    .node(id)
                    .is_some_and(|n| path_hint_matches(&n.file_path, file_hint))
            })
            .cloned()
            .collect();
        match hinted.len() {
            1 => Some((hinted.into_iter().next().unwrap(), EdgeConfidence::Proven)),
            n if n > 1 => Some((hinted.into_iter().next().unwrap(), EdgeConfidence::Likely)),
            _ => None,
        }
    }

    pub fn unresolved_refs(&self) -> Vec<UnresolvedRef> {
        self.inner.read().unresolved.clone()
    }

    pub fn index_meta(&self) -> IndexMeta {
        let data = self.inner.read();
        let file_count = data
            .mesh
            .nodes()
            .filter(|n| n.node_type == NodeType::File)
            .count();
        IndexMeta {
            generation: data.generation,
            file_count,
            indexed_at: data.indexed_at.unwrap_or_else(chrono::Utc::now),
            stale_files: data.stale_files.clone(),
        }
    }

    pub fn file_hash_matches(&self, rel: &str, hash: &str) -> bool {
        self.inner
            .read()
            .file_hashes
            .get(rel)
            .is_some_and(|stored| stored == hash)
    }

    pub fn remove_file_nodes(&self, path: &Path) {
        let mut data = self.inner.write();
        remove_file_nodes_locked(&mut data, path);
    }

    pub fn persist_path(workspace: &Path) -> PathBuf {
        neuromesh_core::ensure_project_data_dir(workspace)
            .unwrap_or_else(|_| neuromesh_core::project_data_dir(workspace))
            .join("graph.bin")
    }

    fn persist_json_path(workspace: &Path) -> PathBuf {
        neuromesh_core::ensure_project_data_dir(workspace)
            .unwrap_or_else(|_| neuromesh_core::project_data_dir(workspace))
            .join("graph.json")
    }

    pub fn load_persisted(&self, workspace: &Path) -> bool {
        self.set_workspace(workspace);
        let loaded = if self
            .load_from(&Self::persist_path(workspace))
            .unwrap_or(false)
        {
            true
        } else {
            self.load_from(&Self::persist_json_path(workspace))
                .unwrap_or(false)
        };
        if loaded && self.stats().total_nodes > 0 {
            self.mark_index_ready();
        }
        loaded
    }

    pub fn save_persisted(&self, workspace: &Path) -> neuromesh_core::Result<()> {
        self.set_workspace(workspace);
        self.save_to(&Self::persist_path(workspace))
    }

    pub fn save_persisted_if_ready(&self) -> neuromesh_core::Result<()> {
        if let Some(workspace) = self.persist_workspace() {
            self.save_persisted(&workspace)?;
        }
        Ok(())
    }

    pub fn set_workspace(&self, workspace: &Path) {
        self.inner.write().workspace_root = Some(workspace.to_path_buf());
    }

    pub fn workspace_root(&self) -> Option<PathBuf> {
        self.inner.read().workspace_root.clone()
    }

    /// Workspace directory for graph/memory persistence (never silently use wrong cwd).
    pub fn persist_workspace(&self) -> Option<PathBuf> {
        self.workspace_root()
    }

    pub fn parser_epoch(&self) -> u32 {
        self.inner.read().parser_epoch
    }

    pub fn learning_episode_applied(&self, episode_id: &str) -> bool {
        self.inner
            .read()
            .applied_learning_episodes
            .contains(episode_id)
    }

    pub fn mark_learning_episode_applied(&self, episode_id: &str) {
        self.inner
            .write()
            .applied_learning_episodes
            .insert(episode_id.to_string());
    }

    pub fn needs_parser_relink(&self) -> bool {
        self.parser_epoch() < GRAPH_PARSER_EPOCH
    }

    pub fn file_fingerprints(&self) -> HashMap<String, FileFingerprint> {
        self.inner.read().file_fingerprints.clone()
    }

    pub fn read_source(&self, path: &Path) -> Option<String> {
        let rel = path.to_string_lossy().replace('\\', "/");
        let (overlay, root) = {
            let data = self.inner.read();
            (
                data.source_overlay.get(&rel).cloned(),
                data.workspace_root.clone(),
            )
        };
        if let Some(src) = overlay {
            return Some(src);
        }
        let root = root?;
        neuromesh_index::read_workspace_file(&root, path).ok()
    }

    pub fn ingest_scan_report(&self, report: &ScanReport) {
        let present: HashSet<String> = if report.present.is_empty() {
            report
                .files
                .iter()
                .map(|(file, _)| file.relative_path.to_string_lossy().replace('\\', "/"))
                .collect()
        } else {
            report.present.iter().cloned().collect()
        };
        self.prune_absent_files(&present);
        self.ingest_workspace_inner(&report.files, false);
    }

    pub fn reindex_incremental(
        &self,
        workspace: &Path,
        project_id: ProjectId,
        max_files: Option<usize>,
    ) {
        self.mark_index_indexing();
        self.set_workspace(workspace);
        let walker = neuromesh_index::ProjectWalker::new(workspace.to_path_buf(), project_id)
            .with_optional_max_files(max_files);
        match walker.scan_report_with(&self.file_fingerprints()) {
            Ok(report) => {
                self.ingest_scan_report(&report);
                self.inner.write().parser_epoch = GRAPH_PARSER_EPOCH;
                let _ = self.save_persisted(workspace);
                self.mark_index_ready();
            }
            Err(_) => self.mark_index_failed(),
        }
    }

    pub fn apply_file_event(&self, event: neuromesh_index::FileChangeEvent) {
        match event {
            neuromesh_index::FileChangeEvent::Modified(file, content)
            | neuromesh_index::FileChangeEvent::Created(file, content) => {
                let ast = neuromesh_parser::CodeIntelligenceEngine::analyze(
                    &file.relative_path,
                    &content,
                    file.language,
                );
                self.ingest_file_keep(&file, &ast, Some(&content), false);
                self.finalize_links();
            }
            neuromesh_index::FileChangeEvent::Deleted(path) => {
                self.remove_file_nodes(&path);
            }
        }
    }

    pub fn ingest_workspace(&self, scanned: &[(IndexedFile, String)]) {
        let present: HashSet<String> = scanned
            .iter()
            .map(|(file, _)| file.relative_path.to_string_lossy().replace('\\', "/"))
            .collect();
        self.prune_absent_files(&present);
        self.ingest_workspace_inner(scanned, true);
    }

    fn ingest_workspace_inner(&self, scanned: &[(IndexedFile, String)], keep_source: bool) {
        if let Some((file, _)) = scanned.first() {
            if let Some(root) = infer_workspace_root(file) {
                self.set_workspace(&root);
            }
        }
        let parsed: Vec<_> = scanned
            .par_iter()
            .filter_map(|(file, content)| {
                let rel = file.relative_path.to_string_lossy().replace('\\', "/");
                if self.file_hash_matches(&rel, &file.blake3_hash) {
                    return None;
                }
                let ast = neuromesh_parser::CodeIntelligenceEngine::analyze(
                    &file.relative_path,
                    content,
                    file.language,
                );
                Some((file, ast, content.as_str()))
            })
            .collect();
        for (file, ast, content) in parsed {
            self.ingest_file_keep(file, &ast, Some(content), keep_source);
        }
        self.apply_manifest_hints(scanned);
        self.finalize_links();
        if !keep_source {
            self.inner.write().source_overlay.clear();
        }
    }

    fn apply_manifest_hints(&self, scanned: &[(IndexedFile, String)]) {
        let hints = crate::manifest::ManifestHints::from_scanned(scanned);
        if hints.is_empty() {
            return;
        }
        let mut data = self.inner.write();
        for rel in &mut data.pending {
            if let Some(hint) = rel.target_file_hint.as_ref() {
                if let Some(rewritten) = hints.rewrite(hint) {
                    rel.target_file_hint = Some(rewritten);
                }
            }
        }
    }

    pub fn prune_absent_files(&self, present_rels: &HashSet<String>) {
        let stale: Vec<String> = {
            let data = self.inner.read();
            data.file_hashes
                .keys()
                .filter(|key| !present_rels.contains(*key))
                .cloned()
                .collect()
        };
        for rel in &stale {
            self.remove_file_nodes(&PathBuf::from(rel));
        }
        let mut data = self.inner.write();
        data.stale_files = stale;
    }

    pub fn save_to(&self, path: &Path) -> neuromesh_core::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let snapshot = {
            let data = self.inner.read();
            GraphSnapshot {
                version: 3,
                nodes: data
                    .mesh
                    .nodes()
                    .map(ContextNode::without_content)
                    .collect(),
                edges: data.mesh.snapshot_edges(),
                pending: data.pending.clone(),
                unresolved: data.unresolved.clone(),
                file_hashes: data.file_hashes.clone(),
                file_fingerprints: data.file_fingerprints.clone(),
                export_index: data.export_index.clone(),
                generation: data.generation,
                indexed_at: data.indexed_at,
                stale_files: data.stale_files.clone(),
                workspace_root: data.workspace_root.clone(),
                parser_epoch: data.parser_epoch.max(GRAPH_PARSER_EPOCH),
                applied_learning_episodes: data.applied_learning_episodes.clone(),
            }
        };
        if snapshot_structurally_unchanged(path, &snapshot) {
            return Ok(());
        }
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            std::fs::write(path, serde_json::to_string(&snapshot)?)?;
        } else {
            let bytes = bincode::serialize(&snapshot)
                .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
            std::fs::write(path, bytes)?;
        }
        Ok(())
    }

    pub fn load_from(&self, path: &Path) -> neuromesh_core::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let raw = std::fs::read(path)?;
        if let Ok(snapshot) = bincode::deserialize::<GraphSnapshot>(&raw) {
            self.install_snapshot(snapshot);
            return Ok(true);
        }
        let text = String::from_utf8_lossy(&raw);
        if let Ok(snapshot) = serde_json::from_str::<GraphSnapshot>(&text) {
            self.install_snapshot(snapshot);
            return Ok(true);
        }
        if let Ok(legacy) = serde_json::from_str::<LegacyGraphData>(&text) {
            self.install_snapshot(GraphSnapshot {
                version: 1,
                nodes: legacy.nodes.into_values().collect(),
                edges: legacy.edges.into_values().collect(),
                pending: legacy.pending,
                unresolved: legacy.unresolved,
                file_hashes: legacy.file_hashes,
                file_fingerprints: HashMap::new(),
                export_index: legacy.export_index,
                generation: legacy.generation,
                indexed_at: legacy.indexed_at,
                stale_files: legacy.stale_files,
                workspace_root: None,
                parser_epoch: 0,
                applied_learning_episodes: HashSet::new(),
            });
            return Ok(true);
        }
        Ok(false)
    }

    fn install_snapshot(&self, snapshot: GraphSnapshot) {
        let relink_needed = snapshot.parser_epoch < GRAPH_PARSER_EPOCH;
        let mut data = self.inner.write();
        data.mesh.load_lists(snapshot.nodes, snapshot.edges);
        data.pending = snapshot.pending;
        data.unresolved = snapshot.unresolved;
        data.file_hashes = snapshot.file_hashes;
        data.file_fingerprints = snapshot.file_fingerprints;
        data.export_index = snapshot.export_index;
        data.generation = snapshot.generation;
        data.indexed_at = snapshot.indexed_at;
        data.stale_files = snapshot.stale_files;
        data.applied_learning_episodes = snapshot.applied_learning_episodes;
        data.parser_epoch = snapshot.parser_epoch;
        if snapshot.workspace_root.is_some() {
            data.workspace_root = snapshot.workspace_root;
        }
        if relink_needed {
            data.file_hashes.clear();
            data.parser_epoch = 0;
        }
        data.source_overlay.clear();
        rebuild_indexes(&mut data);
    }

    pub fn apply_stdp_on_path(&self, node_ids: &[NodeId]) {
        let id_set: HashSet<NodeId> = node_ids.iter().cloned().collect();
        let mut data = self.inner.write();
        let engine = self.synaptic_engine.read();
        for edge in data.mesh.edges_mut() {
            if id_set.contains(&edge.source) && id_set.contains(&edge.target) {
                engine.apply_stdp(edge);
            }
        }
    }

    pub fn shortest_path(&self, start: &NodeId, goal: &NodeId) -> Option<Vec<NodeId>> {
        if start == goal {
            return Some(vec![start.clone()]);
        }
        let data = self.inner.read();
        let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
        let mut q = VecDeque::from([start.clone()]);
        let mut seen = HashSet::from([start.clone()]);
        while let Some(id) = q.pop_front() {
            for (neighbor, _) in data.mesh.neighbors(&id) {
                if seen.insert(neighbor.clone()) {
                    prev.insert(neighbor.clone(), id.clone());
                    if neighbor == *goal {
                        let mut path = vec![goal.clone()];
                        let mut cur = goal.clone();
                        while let Some(p) = prev.get(&cur) {
                            path.push(p.clone());
                            if p == start {
                                break;
                            }
                            cur = p.clone();
                        }
                        path.reverse();
                        return Some(path);
                    }
                    q.push_back(neighbor);
                }
            }
        }
        None
    }

    pub fn spread_energies(
        &self,
        seed_energies: &HashMap<NodeId, f32>,
        decay: f32,
        max_hops: usize,
        min_cutoff: f32,
    ) -> HashMap<NodeId, f32> {
        let data = self.inner.read();
        let mut current = seed_energies.clone();
        let mut final_energies = seed_energies.clone();
        for hop in 0..max_hops {
            let mut next = HashMap::new();
            for (node_id, &energy) in &current {
                if energy < min_cutoff {
                    continue;
                }
                for (neighbor_id, edge) in data.mesh.neighbors(node_id) {
                    let spread =
                        energy * decay * edge.pheromone_weight * edge.edge_type.attenuation();
                    if spread >= min_cutoff {
                        let entry = next.entry(neighbor_id).or_insert(0.0_f32);
                        *entry = (*entry).max(spread);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            for (node_id, &energy) in &next {
                let current_val = final_energies.entry(node_id.clone()).or_insert(0.0);
                *current_val = (*current_val).max(energy * (1.0 - (hop as f32 * 0.15)));
            }
            current = next;
        }
        final_energies
    }

    pub fn steiner_union(&self, seeds: &HashSet<NodeId>) -> HashSet<NodeId> {
        let mut selected = seeds.clone();
        let list: Vec<NodeId> = seeds.iter().cloned().collect();
        if list.len() < 2 {
            return selected;
        }
        let origin = &list[0];
        for other in list.iter().skip(1) {
            if let Some(path) = self.shortest_path(origin, other) {
                selected.extend(path);
            }
        }
        selected
    }

    pub fn get_connected_neighbors(&self, node_id: &NodeId) -> Vec<(NodeId, ContextEdge)> {
        self.inner.read().mesh.neighbors(node_id)
    }

    pub fn get_neighbor_views(&self, node_id: &NodeId) -> Vec<NeighborView> {
        let neighbors = self.get_connected_neighbors(node_id);
        neighbors
            .into_iter()
            .filter_map(|(id, edge)| {
                self.get_node(&id).map(|node| {
                    let direction = if edge.source == *node_id {
                        "outgoing"
                    } else {
                        "incoming"
                    };
                    NeighborView {
                        node: SearchHit::from_node(
                            &node,
                            edge.pheromone_weight,
                            format!("{:?}", edge.edge_type),
                        ),
                        edge,
                        direction: direction.into(),
                    }
                })
            })
            .collect()
    }

    pub fn neighborhood(&self, seeds: &HashSet<NodeId>, hops: usize) -> HashSet<NodeId> {
        self.inner.read().mesh.neighborhood(seeds, hops)
    }

    pub fn subgraph_maps(
        &self,
        nodes: &HashSet<NodeId>,
    ) -> (HashMap<NodeId, ContextNode>, HashMap<EdgeId, ContextEdge>) {
        self.inner.read().mesh.subgraph(nodes)
    }

    pub fn trace_symbol(
        &self,
        query: &str,
        direction: TraceDirection,
        depth: usize,
    ) -> TraceResult {
        let Some((origin, match_reason, origin_reliable)) = self.resolve_trace_origin(query) else {
            return TraceResult {
                origin: None,
                origin_reliable: true,
                hops: Vec::new(),
                callers: Vec::new(),
                callees: Vec::new(),
            };
        };
        let depth = depth.clamp(1, 6);
        let origin_hit = SearchHit::from_node(&origin, 1.0, match_reason);
        let mut hops = Vec::new();
        let mut callers = Vec::new();
        let mut callees = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(origin.id.clone());
        let mut frontier = VecDeque::from([(origin.id.clone(), 0usize)]);

        while let Some((id, hop)) = frontier.pop_front() {
            if hop >= depth {
                continue;
            }
            let neighbors = self.get_connected_neighbors(&id);
            for (neighbor_id, edge) in neighbors {
                let is_call = matches!(edge.edge_type, EdgeType::Calls | EdgeType::UsedBy);
                let outbound = edge.source == id;
                let include = match direction {
                    TraceDirection::Outbound => {
                        outbound && (is_call || edge.edge_type == EdgeType::Imports)
                    }
                    TraceDirection::Inbound => {
                        !outbound && (is_call || edge.edge_type == EdgeType::Imports)
                    }
                    TraceDirection::Both => {
                        is_call
                            || edge.edge_type == EdgeType::Imports
                            || edge.edge_type == EdgeType::Contains
                    }
                };
                if !include {
                    continue;
                }
                if !visited.insert(neighbor_id.clone()) {
                    continue;
                }
                if let Some(neighbor) = self.get_node(&neighbor_id) {
                    if let Some(from) = self.get_node(&id) {
                        let to_hit = SearchHit::from_node(
                            &neighbor,
                            1.0 - hop as f32 * 0.12,
                            format!("{:?}", edge.edge_type),
                        );
                        if outbound {
                            callees.push(to_hit.clone());
                        } else {
                            callers.push(to_hit.clone());
                        }
                        hops.push(TraceHop {
                            from: SearchHit::from_node(&from, 1.0, "from"),
                            to: to_hit,
                            edge_type: edge.edge_type,
                            depth: hop + 1,
                        });
                    }
                    frontier.push_back((neighbor_id, hop + 1));
                }
            }
        }

        TraceResult {
            origin: Some(origin_hit),
            origin_reliable,
            hops,
            callers,
            callees,
        }
    }

    /// Resolve a trace seed with explicit match quality (exact vs fuzzy).
    pub fn resolve_trace_origin(&self, query: &str) -> Option<(ContextNode, String, bool)> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        if let Some(node) = self.get_node(&NodeId::new(query)) {
            return Some((node, "exact_id".into(), true));
        }
        if query.contains('/') || query.contains('\\') || query.contains('.') {
            let file_id = NodeId::from_file_path(&query.replace('\\', "/"));
            if let Some(node) = self.get_node(&file_id) {
                return Some((node, "path".into(), true));
            }
        }
        if let Some((owner, name)) = query.rsplit_once("::") {
            let owner = owner.trim();
            let name = name.trim();
            if !owner.is_empty() && !name.is_empty() {
                if let Some(node) = self
                    .resolve_unique(name, Some(owner))
                    .and_then(|id| self.get_node(&id))
                {
                    return Some((node, "exact_name".into(), true));
                }
            }
        }
        let hits = self.search_symbols(query, 8);
        let best = hits.first()?;
        let node = self.get_node(&best.id)?;
        let reliable = matches!(
            best.match_reason.as_str(),
            "exact_name" | "prefix" | "exact_id"
        ) || (best.match_reason == "path"
            && (query.contains('/') || query.contains('\\') || query.contains('.')));
        let reason = if reliable {
            best.match_reason.clone()
        } else {
            "fuzzy".into()
        };
        Some((node, reason, reliable))
    }

    /// Re-apply persisted episodic feedback after a cold process start.
    pub fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]) {
        for (node_ids, success) in paths {
            if node_ids.is_empty() {
                continue;
            }
            for node_id in *node_ids {
                if self.get_node(node_id).is_some() {
                    self.reinforce_node_access(node_id, *success);
                    self.record_neural_spike(node_id.clone(), true, *success);
                }
            }
            self.apply_stdp_on_path(node_ids);
            self.reinforce_path(node_ids, *success);
            self.reinforce_callee_edges(node_ids, *success);
        }
    }

    pub fn analyze_impact(&self, query: &str, depth: usize) -> ImpactResult {
        let trace = self.trace_symbol(query, TraceDirection::Both, depth);
        let mut affected_files = Vec::new();
        let mut affected_symbols = Vec::new();
        if let Some(origin) = &trace.origin {
            affected_files.push(origin.file_path.to_string_lossy().replace('\\', "/"));
            affected_symbols.push(origin.clone());
        }
        for hop in &trace.hops {
            affected_symbols.push(hop.to.clone());
            let path = hop.to.file_path.to_string_lossy().replace('\\', "/");
            if !affected_files.contains(&path) {
                affected_files.push(path);
            }
        }
        let radius = affected_files.len();
        let risk = if radius >= 12 {
            "high"
        } else if radius >= 5 {
            "medium"
        } else {
            "low"
        };
        ImpactResult {
            origin: trace.origin,
            affected_symbols,
            affected_files,
            risk: risk.into(),
            radius,
        }
    }

    pub fn architecture_summary(&self) -> ArchitectureSummary {
        let data = self.inner.read();
        let mut lang_counts: HashMap<String, usize> = HashMap::new();
        let mut package_files: HashMap<String, usize> = HashMap::new();
        let mut package_symbols: HashMap<String, usize> = HashMap::new();
        let mut degree: HashMap<NodeId, usize> = HashMap::new();
        let mut resolved_calls = 0usize;
        let mut resolved_imports = 0usize;

        for edge in data.mesh.edges() {
            *degree.entry(edge.source.clone()).or_insert(0) += 1;
            *degree.entry(edge.target.clone()).or_insert(0) += 1;
            match edge.edge_type {
                EdgeType::Calls => resolved_calls += 1,
                EdgeType::Imports => resolved_imports += 1,
                _ => {}
            }
        }

        let mut file_count = 0usize;
        let mut symbol_count = 0usize;
        let mut entry_points = Vec::new();

        for node in data.mesh.nodes() {
            if node.node_type == NodeType::File {
                file_count += 1;
                let ext = node
                    .file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("other")
                    .to_string();
                *lang_counts.entry(ext).or_insert(0) += 1;
                let pkg = package_name(&node.file_path);
                *package_files.entry(pkg).or_insert(0) += 1;
                let name = node
                    .file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if matches!(
                    name,
                    "main.rs"
                        | "lib.rs"
                        | "mod.rs"
                        | "index.ts"
                        | "index.js"
                        | "main.py"
                        | "main.go"
                        | "main.dart"
                        | "Program.cs"
                        | "AndroidManifest.xml"
                        | "MainActivity.kt"
                        | "MainActivity.java"
                        | "urls.py"
                        | "page.tsx"
                        | "page.ts"
                        | "route.ts"
                        | "web.php"
                        | "app.php"
                        | "pinx"
                        | "vite.config.ts"
                        | "vite.config.js"
                        | "tauri.conf.json"
                        | "wp-config.php"
                        | "+page.svelte"
                        | "routes.rb"
                        | "go.mod"
                        | "angular.json"
                        | "nest-cli.json"
                        | "App.csproj"
                ) {
                    entry_points.push(SearchHit::from_node(node, 1.0, "entry"));
                }
            } else {
                symbol_count += 1;
                let pkg = package_name(&node.file_path);
                *package_symbols.entry(pkg).or_insert(0) += 1;
            }
        }

        let mut languages: Vec<(String, usize)> = lang_counts.into_iter().collect();
        languages.sort_by_key(|b| std::cmp::Reverse(b.1));

        let mut packages: Vec<ArchitecturePackage> = package_files
            .into_iter()
            .map(|(name, file_count)| ArchitecturePackage {
                symbol_count: package_symbols.get(&name).copied().unwrap_or(0),
                name,
                file_count,
            })
            .collect();
        packages.sort_by_key(|b| std::cmp::Reverse(b.file_count));
        packages.truncate(16);

        let mut hotspots: Vec<SearchHit> = degree
            .into_iter()
            .filter_map(|(id, deg)| {
                data.mesh
                    .node(&id)
                    .map(|n| SearchHit::from_node(n, deg as f32, "degree"))
            })
            .collect();
        hotspots.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hotspots.truncate(12);
        entry_points.truncate(12);

        ArchitectureSummary {
            languages,
            packages,
            entry_points,
            hotspots,
            file_count,
            symbol_count,
            edge_count: data.mesh.edge_count(),
            resolved_calls,
            resolved_imports,
        }
    }

    pub fn spreading_activation(&self, seeds: &HashMap<NodeId, f32>) -> HashMap<NodeId, f32> {
        self.activation_engine.activate(self, seeds)
    }

    /// Bio-inspired Physarum Polycephalum Minimal Steiner Context Solver
    pub fn solve_physarum_context(&self, seed_nodes: &HashSet<NodeId>) -> PhysarumResult {
        self.solve_physarum_local(seed_nodes, 3)
    }

    pub fn solve_physarum_local(
        &self,
        seed_nodes: &HashSet<NodeId>,
        hops: usize,
    ) -> PhysarumResult {
        let neighborhood = self.neighborhood(seed_nodes, hops);
        let (nodes_map, edges_map) = self.subgraph_maps(&neighborhood);
        self.physarum_solver
            .optimize_subgraph(&nodes_map, &edges_map, seed_nodes)
    }

    /// Neighborhood Physarum for `get_context`. Skips huge subgraphs so the
    /// hot path stays under the 20ms tube SLA; `iterations_converged == 0` means skipped.
    pub fn solve_physarum_tube(&self, seed_nodes: &HashSet<NodeId>, hops: usize) -> PhysarumResult {
        const MAX_NODES: usize = 250;
        const MAX_EDGES: usize = 400;
        if seed_nodes.len() < 2 {
            return PhysarumResult {
                active_nodes: seed_nodes.clone(),
                node_flux: HashMap::new(),
                active_edges: HashSet::new(),
                edge_conductance: HashMap::new(),
                pruning_ratio: 0.0,
                iterations_converged: 0,
            };
        }
        let neighborhood = self.neighborhood(seed_nodes, hops);
        let (nodes_map, edges_map) = self.subgraph_maps(&neighborhood);
        if nodes_map.len() > MAX_NODES || edges_map.len() > MAX_EDGES {
            return PhysarumResult {
                active_nodes: seed_nodes.clone(),
                node_flux: HashMap::new(),
                active_edges: HashSet::new(),
                edge_conductance: HashMap::new(),
                pruning_ratio: 0.0,
                iterations_converged: 0,
            };
        }
        PhysarumSolver::new(PhysarumConfig::hot_path())
            .optimize_subgraph(&nodes_map, &edges_map, seed_nodes)
    }

    /// Record a neural firing event (e.g. symbol read or written by AI agent)
    pub fn record_neural_spike(&self, node_id: NodeId, was_modified: bool, was_useful: bool) {
        self.synaptic_engine
            .write()
            .record_spike(node_id, was_modified, was_useful);
    }

    /// Resolve human-friendly node names (`CheckoutView`, paths, sym ids) to graph nodes.
    pub fn resolve_feedback_node(&self, query: &str) -> Option<ContextNode> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(node) = self.get_node(&NodeId::new(trimmed)) {
            return Some(node);
        }
        self.resolve_best(trimmed)
    }

    /// Max learning bonus for a file node and any symbols in the same path.
    pub fn file_learning_boost(&self, file_id: &NodeId) -> f32 {
        let Some(file_node) = self.get_node(file_id) else {
            return 0.0;
        };
        let path = file_node.file_path.clone();
        let mut best = node_learning_bonus(&file_node);
        for node in self.get_all_nodes() {
            if node.file_path == path {
                best = best.max(node_learning_bonus(&node));
            }
        }
        best
    }

    /// File nodes whose reinforced symbols or file metadata exceed `min_bonus`.
    pub fn high_learning_files(&self, min_bonus: f32, limit: usize) -> Vec<(NodeId, f32)> {
        let mut file_bonus: HashMap<NodeId, f32> = HashMap::new();
        for node in self.get_all_nodes() {
            let bonus = node_learning_bonus(&node);
            let file_id = if node.node_type == NodeType::File {
                node.id.clone()
            } else if let Some(fid) = self.file_id_for_path(&node.file_path) {
                fid
            } else {
                continue;
            };
            let slot = file_bonus.entry(file_id).or_insert(0.0);
            *slot = slot.max(bonus);
        }
        let mut out: Vec<(NodeId, f32)> = file_bonus
            .into_iter()
            .filter(|(_, bonus)| *bonus >= min_bonus)
            .collect();
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(limit);
        out
    }

    /// Bump per-node learning signals so the next search/packet can prefer touched symbols.
    pub fn reinforce_node_access(&self, node_id: &NodeId, success: bool) -> f32 {
        let mut data = self.inner.write();
        let Some(node) = data.mesh.node_mut(node_id) else {
            return 0.0;
        };
        let before = node.base_relevance;
        node.access_count = node.access_count.saturating_add(1);
        node.last_accessed = Utc::now();
        if success {
            node.base_relevance = (node.base_relevance + 0.08).min(3.0);
        } else {
            node.base_relevance = (node.base_relevance - 0.12).max(0.1);
        }
        node.base_relevance - before
    }

    /// Observable learning state for a symbol/path (falsifiable feedback checks).
    pub fn node_learning_profile(&self, query: &str) -> Option<NodeLearningProfile> {
        let node = self.resolve_feedback_node(query)?;
        let bonus = node_learning_bonus(&node);
        let neighbors: Vec<NeighborLearningWeight> = self
            .get_connected_neighbors(&node.id)
            .into_iter()
            .take(12)
            .filter_map(|(neighbor_id, edge)| {
                let neighbor = self.get_node(&neighbor_id)?;
                Some(NeighborLearningWeight {
                    node_id: neighbor_id.as_str().to_string(),
                    name: neighbor.name.clone(),
                    path: neighbor.file_path.to_string_lossy().replace('\\', "/"),
                    edge_type: format!("{:?}", edge.edge_type),
                    pheromone_weight: edge.pheromone_weight,
                    reinforcement_count: edge.reinforcement_count,
                })
            })
            .collect();
        Some(NodeLearningProfile {
            node_id: node.id.as_str().to_string(),
            name: node.name.clone(),
            path: node.file_path.to_string_lossy().replace('\\', "/"),
            access_count: node.access_count,
            base_relevance: node.base_relevance,
            learning_bonus: bonus,
            neighbors,
        })
    }

    /// Reinforce 1-hop call/import edges around touched nodes so related files enter packets.
    pub fn reinforce_callee_edges(&self, node_ids: &[NodeId], success: bool) {
        let mut edge_ids: HashSet<EdgeId> = HashSet::new();
        for id in node_ids {
            for (_, edge) in self.get_connected_neighbors(id) {
                if matches!(
                    edge.edge_type,
                    EdgeType::Calls | EdgeType::Imports | EdgeType::References | EdgeType::UsedBy
                ) {
                    edge_ids.insert(edge.id.clone());
                }
            }
        }
        let mut data = self.inner.write();
        for edge_id in edge_ids {
            if let Some(edge) = data.mesh.edge_mut(&edge_id) {
                if success {
                    self.pheromone_engine.reinforce_success(edge, 1);
                } else {
                    self.pheromone_engine.penalize_failure(edge);
                }
            }
        }
    }

    /// Count inbound call/import edges to a symbol (for dead-code evidence).
    pub fn inbound_caller_count(&self, node_id: &NodeId) -> usize {
        self.get_connected_neighbors(node_id)
            .into_iter()
            .filter(|(_, edge)| {
                edge.target == *node_id
                    && matches!(
                        edge.edge_type,
                        EdgeType::Calls | EdgeType::UsedBy | EdgeType::References
                    )
            })
            .count()
    }

    pub fn is_likely_dead_symbol(&self, node_id: &NodeId) -> bool {
        let Some(node) = self.get_node(node_id) else {
            return false;
        };
        if !matches!(
            node.node_type,
            NodeType::Function | NodeType::Symbol | NodeType::Api
        ) {
            return false;
        }
        self.inbound_caller_count(node_id) == 0
    }

    /// Applies STDP only on edges whose endpoints have recorded spikes.
    pub fn apply_stdp_learning(&self) {
        let spiked: HashSet<NodeId> = {
            let engine = self.synaptic_engine.read();
            engine.spiked_nodes()
        };
        if spiked.is_empty() {
            return;
        }
        self.apply_stdp_on_path(&spiked.into_iter().collect::<Vec<_>>());
    }

    pub fn reinforce_path(&self, node_ids: &[NodeId], success: bool) {
        let mut data = self.inner.write();
        for window in node_ids.windows(2) {
            let u = &window[0];
            let v = &window[1];

            // Find matching edge
            let edge_ids = data.mesh.outgoing_to(u, v);
            for edge_id in edge_ids {
                if let Some(edge) = data.mesh.edge_mut(&edge_id) {
                    if success {
                        self.pheromone_engine.reinforce_success(edge, 1);
                    } else {
                        self.pheromone_engine.penalize_failure(edge);
                    }
                }
            }
        }
    }

    pub fn stats(&self) -> GraphStats {
        let data = self.inner.read();
        let total_nodes = data.mesh.node_count();
        let total_edges = data.mesh.edge_count();
        let file_nodes = data
            .mesh
            .nodes()
            .filter(|n| n.node_type == NodeType::File)
            .count();
        let symbol_nodes = total_nodes.saturating_sub(file_nodes);

        let total_weight: f32 = data.mesh.edges().map(|e| e.pheromone_weight).sum();
        let average_pheromone_weight = if total_edges > 0 {
            total_weight / total_edges as f32
        } else {
            0.5
        };

        let high_conductance_synapses = data
            .mesh
            .edges()
            .filter(|e| e.pheromone_weight >= 0.70)
            .count();
        let atrophied_synapses = data
            .mesh
            .edges()
            .filter(|e| e.pheromone_weight <= 0.15)
            .count();
        let resolved_calls = data
            .mesh
            .edges()
            .filter(|e| e.edge_type == EdgeType::Calls)
            .count();
        let resolved_imports = data
            .mesh
            .edges()
            .filter(|e| e.edge_type == EdgeType::Imports)
            .count();

        GraphStats {
            total_nodes,
            total_edges,
            file_nodes,
            symbol_nodes,
            average_pheromone_weight,
            high_conductance_synapses,
            atrophied_synapses,
            resolved_calls,
            resolved_imports,
            unresolved_count: data.unresolved.len(),
            generation: data.generation,
        }
    }

    pub fn total_tokens(&self) -> usize {
        let data = self.inner.read();
        data.mesh
            .nodes()
            .filter(|n| n.node_type == NodeType::File)
            .map(|n| n.token_cost)
            .sum()
    }
}

fn type_search_rank(node_type: &NodeType) -> u8 {
    match node_type {
        NodeType::Class | NodeType::Component | NodeType::Api | NodeType::DbModel => 3,
        NodeType::Function | NodeType::Symbol => 2,
        NodeType::File => 0,
        _ => 1,
    }
}

/// Observable learning signal from access history and reinforced relevance.
pub fn node_learning_bonus(node: &ContextNode) -> f32 {
    let access = (node.access_count as f32).ln_1p() * 5.0;
    let relevance = (node.base_relevance - 1.0).max(0.0) * 10.0;
    access + relevance
}

fn ranking_bonus(node: &ContextNode, query: &str) -> f32 {
    let mut bonus = match node.node_type {
        NodeType::Function
        | NodeType::Class
        | NodeType::Component
        | NodeType::Api
        | NodeType::DbModel => 8.0,
        NodeType::Symbol if node.name.eq_ignore_ascii_case(query) => 8.0,
        NodeType::StyleToken => 6.0,
        NodeType::File => 1.0,
        _ => 0.0,
    };
    if node.name.eq_ignore_ascii_case(query) {
        bonus += 16.0;
    } else if query.len() >= 4 {
        let spec = name_match_specificity(query, &node.name);
        if spec > 0.0 {
            bonus += 10.0 * spec;
        }
    }
    let decoy = is_name_collision_decoy(&node.file_path);
    if !decoy {
        if path_echoes_symbol(&node.file_path, query) {
            bonus += 12.0;
        }
        if file_stem_equals(&node.file_path, query) {
            bonus += 30.0;
        }
        if is_core_source_path(&node.file_path)
            && (node.name.eq_ignore_ascii_case(query)
                || file_stem_equals(&node.file_path, query)
                || name_match_specificity(query, &node.name) >= 0.5)
        {
            bonus += 10.0;
        }
    }
    if is_json_schema_path(&node.file_path) {
        bonus -= 20.0;
    }
    if is_fixture_path(&node.file_path) {
        bonus -= if decoy { 40.0 } else { 24.0 };
    }
    bonus += node_learning_bonus(node);
    bonus
}

fn normalize_path_hint(value: &str) -> String {
    value.replace('\\', "/").replace('-', "_").to_lowercase()
}

/// Relative file paths (`theme/default/hello.twig`) must not match every
/// path that merely contains `theme` or `hello`.
fn looks_like_file_path_hint(hint: &str) -> bool {
    let hint = hint.replace('\\', "/");
    let Some((_, ext)) = hint.rsplit_once('.') else {
        return false;
    };
    !ext.is_empty()
        && ext.len() <= 8
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && (hint.contains('/') || ext.eq_ignore_ascii_case("twig"))
}

fn template_stem_hint(hint: &str, target_symbol: &str) -> bool {
    if !looks_like_file_path_hint(hint) {
        return false;
    }
    let hint = hint.replace('\\', "/");
    let Some(stem) = Path::new(&hint).file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let stem_l = stem.to_lowercase();
    let target_l = target_symbol
        .trim()
        .trim_end_matches(".twig")
        .to_lowercase();
    !target_l.is_empty() && (stem_l == target_l || stem_l.starts_with(&format!("{target_l}.")))
}

fn file_stem_equals(path: &Path, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return false;
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(query))
}

/// True when a file stem or parent directory repeats the symbol name
/// (`Searcher` ↔ `src/searcher/mod.rs`).
pub fn path_echoes_symbol(path: &Path, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return false;
    }
    let q_lower = query.to_lowercase();
    let q_snake = to_snake_case(query).to_lowercase();
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    lower.split('/').any(|seg| {
        let stem = seg.rsplit_once('.').map(|(name, _)| name).unwrap_or(seg);
        if matches!(stem, "mod" | "lib" | "index") {
            return false;
        }
        stem == q_lower || stem == q_snake
    })
}

fn is_fixture_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    is_low_priority_source_path(path)
        || lower.contains("/editors/")
        || lower.starts_with("editors/")
}

fn is_crate_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    lower.contains("/crates/") || lower.starts_with("crates/")
}

fn path_ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn field_matches_type(field: &str, type_name: &str) -> bool {
    let field = field.to_lowercase();
    if field.is_empty() {
        return false;
    }
    let snake = to_snake_case(type_name);
    snake == field
        || snake.ends_with(&format!("_{field}"))
        || type_name.to_lowercase().ends_with(&field)
}

fn package_name(path: &Path) -> String {
    let parts: Vec<String> = path
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    if let Some(idx) = parts
        .iter()
        .position(|p| p == "crates" || p == "packages" || p == "apps")
    {
        if let Some(name) = parts.get(idx + 1) {
            return name.clone();
        }
    }
    parts.first().cloned().unwrap_or_else(|| "root".into())
}

fn snapshot_structurally_unchanged(path: &Path, new: &GraphSnapshot) -> bool {
    if !path.exists() {
        return false;
    }
    let Ok(raw) = std::fs::read(path) else {
        return false;
    };
    let old = if let Ok(s) = bincode::deserialize::<GraphSnapshot>(&raw) {
        s
    } else {
        let text = String::from_utf8_lossy(&raw);
        match serde_json::from_str::<GraphSnapshot>(&text) {
            Ok(s) => s,
            Err(_) => return false,
        }
    };
    structural_digest(&old) == structural_digest(new)
}

fn structural_digest(snapshot: &GraphSnapshot) -> String {
    let mut node_ids: Vec<&str> = snapshot.nodes.iter().map(|n| n.id.as_str()).collect();
    node_ids.sort_unstable();
    let mut edge_ids: Vec<&str> = snapshot.edges.iter().map(|e| e.id.as_str()).collect();
    edge_ids.sort_unstable();
    let mut hashes: Vec<String> = snapshot
        .file_hashes
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    hashes.sort();
    let mut learning: Vec<String> = snapshot
        .nodes
        .iter()
        .map(|n| {
            format!(
                "{}:{}:{:.4}",
                n.id.as_str(),
                n.access_count,
                n.base_relevance
            )
        })
        .collect();
    learning.sort();
    let mut edge_learning: Vec<String> = snapshot
        .edges
        .iter()
        .map(|e| {
            format!(
                "{}:{:.4}:{}",
                e.id.as_str(),
                e.pheromone_weight,
                e.reinforcement_count
            )
        })
        .collect();
    edge_learning.sort();
    let mut applied: Vec<String> = snapshot.applied_learning_episodes.iter().cloned().collect();
    applied.sort();
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        node_ids.join("\n"),
        edge_ids.join("\n"),
        hashes.join("\n"),
        learning.join("\n"),
        edge_learning.join("\n"),
        applied.join("\n"),
        snapshot.parser_epoch
    );
    neuromesh_index::ContentHasher::hash_str(&payload)
}

fn pinia_store_type_from_alias(alias: &str) -> String {
    let alias = alias.trim();
    if alias.is_empty() {
        return String::new();
    }
    let mut chars = alias.chars();
    let first = chars.next().unwrap().to_uppercase().collect::<String>();
    let rest = chars.as_str();
    format!("use{first}{rest}Store")
}
