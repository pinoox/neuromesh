use crate::activation::{SpreadingActivation, SpreadingActivationConfig};
use crate::edge::{PheromoneConfig, PheromoneEngine};
use crate::node::NodeFactory;
use crate::physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
use crate::query::{
    path_hint_matches, tokenize, ArchitecturePackage, ArchitectureSummary, ImpactResult,
    NeighborView, SearchHit, TraceDirection, TraceHop, TraceResult,
};
use crate::synapse::{StdpConfig, SynapticPlasticityEngine};
use neuromesh_core::{
    ContextEdge, ContextNode, EdgeConfidence, EdgeId, EdgeType, IndexMeta, NodeId, NodeType,
    ProjectId, UnresolvedRef,
};
use neuromesh_index::IndexedFile;
use neuromesh_parser::AstAnalysisResult;
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

#[derive(Default, Clone, Serialize, Deserialize)]
struct GraphData {
    nodes: HashMap<NodeId, ContextNode>,
    edges: HashMap<EdgeId, ContextEdge>,
    outgoing: HashMap<NodeId, Vec<EdgeId>>,
    incoming: HashMap<NodeId, Vec<EdgeId>>,
    name_to_nodes: HashMap<String, Vec<NodeId>>,
    file_to_nodes: HashMap<PathBuf, Vec<NodeId>>,
    token_to_nodes: HashMap<String, Vec<NodeId>>,
    pending: Vec<PendingRel>,
    unresolved: Vec<UnresolvedRef>,
    impl_index: HashMap<String, Vec<NodeId>>,
    #[serde(default)]
    export_index: HashMap<String, Vec<NodeId>>,
    file_hashes: HashMap<String, String>,
    generation: u64,
    indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    stale_files: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingRel {
    source_file: PathBuf,
    source_symbol: String,
    target_symbol: String,
    relationship: EdgeType,
    target_file_hint: Option<String>,
    #[serde(default)]
    receiver_hint: Option<String>,
}

#[derive(Clone)]
pub struct NeuralProjectGraph {
    project_id: Arc<RwLock<ProjectId>>,
    inner: Arc<RwLock<GraphData>>,
    pheromone_engine: Arc<PheromoneEngine>,
    activation_engine: Arc<SpreadingActivation>,
    synaptic_engine: Arc<RwLock<SynapticPlasticityEngine>>,
    physarum_solver: Arc<PhysarumSolver>,
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
        }
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id.read().clone()
    }

    pub fn set_project_id(&self, new_id: ProjectId) {
        *self.project_id.write() = new_id;
    }

    pub fn clear(&self, new_project_id: Option<ProjectId>) {
        if let Some(new_id) = new_project_id {
            *self.project_id.write() = new_id;
        }
        let mut data = self.inner.write();
        data.nodes.clear();
        data.edges.clear();
        data.outgoing.clear();
        data.incoming.clear();
        data.name_to_nodes.clear();
        data.file_to_nodes.clear();
        data.token_to_nodes.clear();
        data.pending.clear();
        data.unresolved.clear();
        data.impl_index.clear();
        data.export_index.clear();
        data.file_hashes.clear();
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
        data.nodes.insert(node.id.clone(), node.clone());
        data.file_to_nodes
            .entry(file.relative_path.clone())
            .or_default()
            .push(node.id.clone());
        data.name_to_nodes
            .entry(node.name.clone().to_lowercase())
            .or_default()
            .push(node.id.clone());
        index_tokens(&mut data, &node.id, &node.name);
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
        data.nodes.insert(node.id.clone(), node.clone());
        data.file_to_nodes
            .entry(file_path.to_path_buf())
            .or_default()
            .push(node.id.clone());
        data.name_to_nodes
            .entry(symbol_name.to_lowercase())
            .or_default()
            .push(node.id.clone());
        index_tokens(&mut data, &node.id, symbol_name);
        if let Some(parent) = parent {
            let key = format!("{}::{}", parent.to_lowercase(), symbol_name.to_lowercase());
            data.impl_index
                .entry(key)
                .or_default()
                .push(node.id.clone());
        }

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
        data.edges.insert(edge.id.clone(), edge.clone());
        data.outgoing
            .entry(source.clone())
            .or_default()
            .push(edge.id.clone());
        data.incoming
            .entry(target.clone())
            .or_default()
            .push(edge.id.clone());

        edge
    }

    /// Ingests AST analysis results for an indexed file and creates symbols and edges
    pub fn ingest_ast(&self, file: &IndexedFile, ast: &AstAnalysisResult) {
        let content = std::fs::read_to_string(&file.full_path).ok();
        self.ingest_file(file, ast, content.as_deref());
    }

    pub fn ingest_file(&self, file: &IndexedFile, ast: &AstAnalysisResult, content: Option<&str>) {
        let rel = file.relative_path.to_string_lossy().replace('\\', "/");
        if self.file_hash_matches(&rel, &file.blake3_hash) {
            return;
        }
        self.remove_file_nodes(&file.relative_path);

        let file_node = self.add_file_node(file, content.map(|c| c.to_string()));
        {
            let mut data = self.inner.write();
            data.file_hashes.insert(rel, file.blake3_hash.clone());
            data.indexed_at = Some(chrono::Utc::now());
        }
        let mut local_symbols: HashMap<String, NodeId> = HashMap::new();

        for sym in &ast.symbols {
            let token_cost = sym
                .line_range
                .end
                .saturating_sub(sym.line_range.start)
                .max(1)
                * 8;
            let sym_node = self.add_symbol_node_with_parent(
                &file.relative_path,
                &sym.name,
                sym.symbol_type,
                sym.signature.clone(),
                sym.line_range.clone(),
                token_cost,
                sym.parent.clone(),
            );
            local_symbols.insert(sym.name.to_lowercase(), sym_node.id.clone());
            if sym.exported {
                let mut data = self.inner.write();
                data.export_index
                    .entry(sym.name.to_lowercase())
                    .or_default()
                    .push(sym_node.id.clone());
            }
            self.add_edge(
                file_node.id.clone(),
                sym_node.id.clone(),
                EdgeType::Contains,
            );
        }

        for export_name in &ast.exports {
            if let Some(id) = local_symbols.get(&export_name.to_lowercase()) {
                let mut data = self.inner.write();
                let entry = data
                    .export_index
                    .entry(export_name.to_lowercase())
                    .or_default();
                if !entry.iter().any(|existing| existing == id) {
                    entry.push(id.clone());
                }
            }
        }

        for token in &ast.design_tokens {
            if local_symbols.contains_key(&token.to_lowercase()) {
                continue;
            }
            let token_node = self.add_symbol_node(
                &file.relative_path,
                token,
                NodeType::StyleToken,
                Some(format!("Token: {}", token)),
                1..2,
                5,
            );
            local_symbols.insert(token.to_lowercase(), token_node.id.clone());
            self.add_edge(
                file_node.id.clone(),
                token_node.id.clone(),
                EdgeType::References,
            );
        }

        let _ = local_symbols;

        {
            let mut data = self.inner.write();
            for rel in &ast.relationships {
                data.pending.push(PendingRel {
                    source_file: file.relative_path.clone(),
                    source_symbol: rel.source_symbol.clone(),
                    target_symbol: rel.target_symbol.clone(),
                    relationship: rel.relationship,
                    target_file_hint: rel.target_file_hint.clone(),
                    receiver_hint: rel.receiver_hint.clone(),
                });
            }
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
                    if let Some((target, confidence)) = self.resolve_call_ranked(
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
                data.nodes
                    .get(*id)
                    .is_some_and(|n| n.node_type == NodeType::File)
            }) {
                return Some(id.clone());
            }
        }
        None
    }

    pub fn get_node(&self, id: &NodeId) -> Option<ContextNode> {
        let data = self.inner.read();
        data.nodes.get(id).cloned()
    }

    pub fn get_all_nodes(&self) -> Vec<ContextNode> {
        let data = self.inner.read();
        data.nodes.values().cloned().collect()
    }

    pub fn get_all_nodes_for_viz(&self) -> Vec<ContextNode> {
        let data = self.inner.read();
        data.nodes
            .values()
            .map(ContextNode::without_content)
            .collect()
    }

    pub fn get_nodes_map(&self) -> HashMap<NodeId, ContextNode> {
        let data = self.inner.read();
        data.nodes.clone()
    }

    pub fn get_edges_map(&self) -> HashMap<EdgeId, ContextEdge> {
        let data = self.inner.read();
        data.edges.clone()
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

        if let Some(ids) = data.name_to_nodes.get(&query_lower) {
            for id in ids {
                scored.insert(id.clone(), (100.0, "exact_name".into()));
            }
        }

        if query_lower.len() >= 3 {
            for (name, ids) in &data.name_to_nodes {
                if name == &query_lower {
                    continue;
                }
                let reason_score = if name.starts_with(&query_lower) {
                    Some((86.0, "prefix"))
                } else if name.contains(&query_lower) {
                    Some((68.0, "substring"))
                } else {
                    None
                };
                if let Some((score, reason)) = reason_score {
                    for id in ids {
                        scored.entry(id.clone()).or_insert((score, reason.into()));
                    }
                }
            }
        }

        for token in &query_tokens {
            if let Some(ids) = data.token_to_nodes.get(token) {
                for id in ids {
                    scored.entry(id.clone()).or_insert((74.0, "token".into()));
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
                data.nodes.get(&id).map(|node| {
                    SearchHit::from_node(node, score + ranking_bonus(node, query), reason)
                })
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
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
                    data.nodes
                        .get(*id)
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

    fn resolve_file_hint(&self, hint: &str) -> Option<NodeId> {
        let data = self.inner.read();
        let mut matches = Vec::new();
        for (path, ids) in &data.file_to_nodes {
            if path_hint_matches(path, hint) {
                for id in ids {
                    if data
                        .nodes
                        .get(id)
                        .is_some_and(|n| n.node_type == NodeType::File)
                    {
                        matches.push(id.clone());
                    }
                }
            }
        }
        if matches.len() == 1 {
            matches.into_iter().next()
        } else {
            None
        }
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
                data.nodes
                    .get(*id)
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
                data.nodes
                    .get(*id)
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
                data.nodes.get(*id).is_some_and(|n| {
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
                data.nodes.get(*id).is_some_and(|n| {
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
                    data.nodes
                        .get(*id)
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
                    data.nodes
                        .get(*id)
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
            let key = format!("{}::{}", type_name.to_lowercase(), name.to_lowercase());
            let data = self.inner.read();
            if let Some(ids) = data.impl_index.get(&key) {
                if ids.len() == 1 {
                    return Some((ids[0].clone(), EdgeConfidence::Proven));
                }
                if let Some(id) = ids.iter().find(|id| {
                    data.nodes
                        .get(*id)
                        .is_some_and(|n| n.file_path == source_file)
                }) {
                    return Some((id.clone(), EdgeConfidence::Proven));
                }
                if !ids.is_empty() {
                    return Some((ids[0].clone(), EdgeConfidence::Likely));
                }
            }
        }

        if let Some(id) = self.resolve_call_target(name, source_file, imported_files) {
            return Some((id, EdgeConfidence::Proven));
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
            .nodes
            .values()
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
                data.nodes
                    .get(*id)
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
                data.nodes
                    .get(*id)
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
            .nodes
            .values()
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
        let normalized = path.to_string_lossy().replace('\\', "/");
        let mut data = self.inner.write();
        let keys: Vec<PathBuf> = data
            .file_to_nodes
            .keys()
            .filter(|p| p.to_string_lossy().replace('\\', "/") == normalized)
            .cloned()
            .collect();
        if keys.is_empty() {
            data.file_hashes.remove(&normalized);
            return;
        }
        for key in keys {
            let Some(ids) = data.file_to_nodes.remove(&key) else {
                continue;
            };
            for id in ids {
                if let Some(node) = data.nodes.remove(&id) {
                    if let Some(list) = data.name_to_nodes.get_mut(&node.name.to_lowercase()) {
                        list.retain(|existing| existing != &id);
                    }
                    if let Some(parent) = &node.parent {
                        let key =
                            format!("{}::{}", parent.to_lowercase(), node.name.to_lowercase());
                        if let Some(list) = data.impl_index.get_mut(&key) {
                            list.retain(|existing| existing != &id);
                        }
                    }
                    if let Some(list) = data.export_index.get_mut(&node.name.to_lowercase()) {
                        list.retain(|existing| existing != &id);
                    }
                }
                data.outgoing.remove(&id);
                data.incoming.remove(&id);
            }
        }
        data.file_hashes.remove(&normalized);
        let alive: HashSet<NodeId> = data.nodes.keys().cloned().collect();
        data.edges
            .retain(|_, e| alive.contains(&e.source) && alive.contains(&e.target));
    }

    pub fn persist_path(workspace: &Path) -> PathBuf {
        workspace.join(".neuromesh").join("graph.json")
    }

    pub fn load_persisted(&self, workspace: &Path) -> bool {
        self.load_from(&Self::persist_path(workspace))
            .unwrap_or(false)
    }

    pub fn save_persisted(&self, workspace: &Path) -> neuromesh_core::Result<()> {
        self.save_to(&Self::persist_path(workspace))
    }

    pub fn ingest_workspace(&self, scanned: &[(IndexedFile, String)]) {
        let present: HashSet<String> = scanned
            .iter()
            .map(|(file, _)| file.relative_path.to_string_lossy().replace('\\', "/"))
            .collect();
        self.prune_absent_files(&present);
        // Parse in parallel (thread-local tree-sitter parsers). Unchanged
        // hashes skip parse. Ingest stays serial so graph writes stay single-writer.
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
            self.ingest_file(file, &ast, Some(content));
        }
        self.apply_manifest_hints(scanned);
        self.finalize_links();
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
            serde_json::to_string(&*data)?
        };
        std::fs::write(path, snapshot)?;
        Ok(())
    }

    pub fn load_from(&self, path: &Path) -> neuromesh_core::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let raw = std::fs::read_to_string(path)?;
        let loaded: GraphData = serde_json::from_str(&raw)?;
        *self.inner.write() = loaded;
        Ok(true)
    }

    pub fn apply_stdp_on_path(&self, node_ids: &[NodeId]) {
        let id_set: HashSet<NodeId> = node_ids.iter().cloned().collect();
        let mut data = self.inner.write();
        let engine = self.synaptic_engine.read();
        for edge in data.edges.values_mut() {
            if id_set.contains(&edge.source) && id_set.contains(&edge.target) {
                engine.apply_stdp(edge);
            }
        }
    }

    pub fn shortest_path(&self, start: &NodeId, goal: &NodeId) -> Option<Vec<NodeId>> {
        if start == goal {
            return Some(vec![start.clone()]);
        }
        let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
        let mut q = VecDeque::from([start.clone()]);
        let mut seen = HashSet::from([start.clone()]);
        while let Some(id) = q.pop_front() {
            for (neighbor, _) in self.get_connected_neighbors(&id) {
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
        let data = self.inner.read();
        let mut neighbors = Vec::new();

        if let Some(edge_ids) = data.outgoing.get(node_id) {
            for edge_id in edge_ids {
                if let Some(edge) = data.edges.get(edge_id) {
                    neighbors.push((edge.target.clone(), edge.clone()));
                }
            }
        }

        if let Some(edge_ids) = data.incoming.get(node_id) {
            for edge_id in edge_ids {
                if let Some(edge) = data.edges.get(edge_id) {
                    neighbors.push((edge.source.clone(), edge.clone()));
                }
            }
        }

        neighbors
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
        let mut visited = seeds.clone();
        let mut frontier: VecDeque<(NodeId, usize)> =
            seeds.iter().cloned().map(|id| (id, 0)).collect();

        while let Some((id, depth)) = frontier.pop_front() {
            if depth >= hops {
                continue;
            }
            for (neighbor, _) in self.get_connected_neighbors(&id) {
                if visited.insert(neighbor.clone()) {
                    frontier.push_back((neighbor, depth + 1));
                }
            }
        }
        visited
    }

    pub fn subgraph_maps(
        &self,
        nodes: &HashSet<NodeId>,
    ) -> (HashMap<NodeId, ContextNode>, HashMap<EdgeId, ContextEdge>) {
        let data = self.inner.read();
        let node_map = nodes
            .iter()
            .filter_map(|id| data.nodes.get(id).cloned().map(|n| (id.clone(), n)))
            .collect();
        let edge_map = data
            .edges
            .iter()
            .filter(|(_, e)| nodes.contains(&e.source) && nodes.contains(&e.target))
            .map(|(id, e)| (id.clone(), e.clone()))
            .collect();
        (node_map, edge_map)
    }

    pub fn trace_symbol(
        &self,
        query: &str,
        direction: TraceDirection,
        depth: usize,
    ) -> TraceResult {
        let origin_node = self.resolve_best(query);
        let Some(origin) = origin_node else {
            return TraceResult {
                origin: None,
                hops: Vec::new(),
                callers: Vec::new(),
                callees: Vec::new(),
            };
        };
        let depth = depth.clamp(1, 6);
        let origin_hit = SearchHit::from_node(&origin, 1.0, "origin");
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
            hops,
            callers,
            callees,
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

        for edge in data.edges.values() {
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

        for node in data.nodes.values() {
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
                        | "vite.config.ts"
                        | "vite.config.js"
                        | "tauri.conf.json"
                        | "wp-config.php"
                        | "+page.svelte"
                        | "routes.rb"
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
                data.nodes
                    .get(&id)
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
            edge_count: data.edges.len(),
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
            let edge_ids_opt = data.outgoing.get(u).cloned();
            if let Some(edge_ids) = edge_ids_opt {
                for edge_id in edge_ids {
                    if let Some(edge) = data.edges.get_mut(&edge_id) {
                        if edge.target == *v {
                            if success {
                                self.pheromone_engine.reinforce_success(edge, 1);
                            } else {
                                self.pheromone_engine.penalize_failure(edge);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn stats(&self) -> GraphStats {
        let data = self.inner.read();
        let total_nodes = data.nodes.len();
        let total_edges = data.edges.len();
        let file_nodes = data
            .nodes
            .values()
            .filter(|n| n.node_type == NodeType::File)
            .count();
        let symbol_nodes = total_nodes.saturating_sub(file_nodes);

        let total_weight: f32 = data.edges.values().map(|e| e.pheromone_weight).sum();
        let average_pheromone_weight = if total_edges > 0 {
            total_weight / total_edges as f32
        } else {
            0.5
        };

        let high_conductance_synapses = data
            .edges
            .values()
            .filter(|e| e.pheromone_weight >= 0.70)
            .count();
        let atrophied_synapses = data
            .edges
            .values()
            .filter(|e| e.pheromone_weight <= 0.15)
            .count();
        let resolved_calls = data
            .edges
            .values()
            .filter(|e| e.edge_type == EdgeType::Calls)
            .count();
        let resolved_imports = data
            .edges
            .values()
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
        data.nodes
            .values()
            .filter(|n| n.node_type == NodeType::File)
            .map(|n| n.token_cost)
            .sum()
    }
}

fn index_tokens(data: &mut GraphData, id: &NodeId, name: &str) {
    for token in tokenize(name) {
        let ids = data.token_to_nodes.entry(token).or_default();
        if !ids.iter().any(|existing| existing == id) {
            ids.push(id.clone());
        }
    }
}

fn ranking_bonus(node: &ContextNode, query: &str) -> f32 {
    let mut bonus = match node.node_type {
        NodeType::Function | NodeType::Class | NodeType::Component | NodeType::Api => 8.0,
        NodeType::File => 1.0,
        _ => 0.0,
    };
    if node.name == query {
        bonus += 16.0;
    }
    if path_echoes_symbol(&node.file_path, query) {
        bonus += 12.0;
    }
    if is_fixture_path(&node.file_path) {
        bonus -= 24.0;
    }
    bonus
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
    lower.contains("/tests/")
        || lower.contains("_tests.rs")
        || lower.contains("/test/")
        || lower.ends_with("/tests.rs")
        || lower.contains("quality_tests")
        || lower.contains("/editors/")
        || lower.starts_with("editors/")
        || lower.contains("/benches/")
        || lower.starts_with("benches/")
        || lower.contains("/examples/")
        || lower.starts_with("examples/")
        || lower.contains("/testdata/")
        || lower.contains("/test_data/")
        || lower.starts_with("testdata/")
        || lower.starts_with("test_data/")
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
