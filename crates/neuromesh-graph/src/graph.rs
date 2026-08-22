use crate::activation::{SpreadingActivation, SpreadingActivationConfig};
use crate::edge::{PheromoneConfig, PheromoneEngine};
use crate::node::NodeFactory;
use crate::physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
use crate::query::{
    path_hint_matches, tokenize, ArchitecturePackage, ArchitectureSummary, ImpactResult,
    NeighborView, SearchHit, TraceDirection, TraceHop, TraceResult,
};
use crate::synapse::{StdpConfig, SynapticPlasticityEngine};
use neuromesh_core::{ContextEdge, ContextNode, EdgeId, EdgeType, NodeId, NodeType, ProjectId};
use neuromesh_index::IndexedFile;
use neuromesh_parser::AstAnalysisResult;
use parking_lot::RwLock;
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
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingRel {
    source_file: PathBuf,
    source_symbol: String,
    target_symbol: String,
    relationship: EdgeType,
    target_file_hint: Option<String>,
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
        let current_pid = self.project_id.read().clone();
        let node = NodeFactory::create_symbol_node(
            current_pid,
            file_path.to_path_buf(),
            node_type,
            symbol_name.to_string(),
            signature,
            line_range,
            token_cost,
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

        node
    }

    pub fn add_edge(&self, source: NodeId, target: NodeId, edge_type: EdgeType) -> ContextEdge {
        if source == target {
            let current_pid = self.project_id.read().clone();
            return self
                .pheromone_engine
                .create_edge(current_pid, source, target, edge_type);
        }
        let current_pid = self.project_id.read().clone();
        let edge = self.pheromone_engine.create_edge(
            current_pid,
            source.clone(),
            target.clone(),
            edge_type,
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
        let file_node = self.add_file_node(file, content.map(|c| c.to_string()));
        let mut local_symbols: HashMap<String, NodeId> = HashMap::new();

        for sym in &ast.symbols {
            let token_cost = sym
                .line_range
                .end
                .saturating_sub(sym.line_range.start)
                .max(1)
                * 8;
            let sym_node = self.add_symbol_node(
                &file.relative_path,
                &sym.name,
                sym.symbol_type,
                sym.signature.clone(),
                sym.line_range.clone(),
                token_cost,
            );
            local_symbols.insert(sym.name.to_lowercase(), sym_node.id.clone());
            self.add_edge(
                file_node.id.clone(),
                sym_node.id.clone(),
                EdgeType::Contains,
            );
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
                });
            }
        }
    }

    /// Resolve queued import/call edges after symbols exist. Safe to call after every file
    /// and again at the end of a workspace scan.
    pub fn finalize_links(&self) {
        let pending = {
            let mut data = self.inner.write();
            std::mem::take(&mut data.pending)
        };

        let mut leftover = Vec::new();
        for rel in pending {
            let file_id = NodeId::from_file_path(&rel.source_file.to_string_lossy().replace('\\', "/"));
            let imported_files = self.imported_files_of(&file_id);

            let linked = match rel.relationship {
                EdgeType::Imports => {
                    if let Some(target) =
                        self.resolve_unique(&rel.target_symbol, rel.target_file_hint.as_deref())
                    {
                        if target != file_id {
                            self.add_edge(file_id.clone(), target, EdgeType::Imports);
                        }
                        true
                    } else if let Some(hint) = &rel.target_file_hint {
                        if let Some(target_file) = self.resolve_file_hint(hint) {
                            self.add_edge(file_id, target_file, EdgeType::DependsOn);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                EdgeType::Calls => {
                    let source = self
                        .resolve_unique(&rel.source_symbol, Some(&rel.source_file.to_string_lossy()))
                        .unwrap_or_else(|| file_id.clone());
                    if let Some(target) = self.resolve_call_target(
                        &rel.target_symbol,
                        &rel.source_file,
                        &imported_files,
                    ) {
                        if target != source {
                            self.add_edge(source, target, EdgeType::Calls);
                        }
                        true
                    } else {
                        false
                    }
                }
                other => {
                    if let Some(target) =
                        self.resolve_unique(&rel.target_symbol, rel.target_file_hint.as_deref())
                    {
                        self.add_edge(file_id, target, other);
                        true
                    } else {
                        false
                    }
                }
            };

            if !linked {
                leftover.push(rel);
            }
        }

        self.inner.write().pending = leftover;
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

    pub fn get_node(&self, id: &NodeId) -> Option<ContextNode> {
        let data = self.inner.read();
        data.nodes.get(id).cloned()
    }

    pub fn get_all_nodes(&self) -> Vec<ContextNode> {
        let data = self.inner.read();
        data.nodes.values().cloned().collect()
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
                    let bonus = match node.node_type {
                        NodeType::Function | NodeType::Class | NodeType::Component => 4.0,
                        NodeType::File => 1.0,
                        _ => 0.0,
                    };
                    SearchHit::from_node(node, score + bonus, reason)
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
        let ids = data.name_to_nodes.get(&name_lower).cloned().unwrap_or_default();
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
            if !hinted.is_empty() {
                return hinted.into_iter().next();
            }
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
        let ids = data.name_to_nodes.get(&name_lower).cloned().unwrap_or_default();
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

        if ids.len() == 1 {
            return ids.into_iter().next();
        }
        None
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
                        node: SearchHit::from_node(&node, edge.pheromone_weight, format!("{:?}", edge.edge_type)),
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

    pub fn trace_symbol(&self, query: &str, direction: TraceDirection, depth: usize) -> TraceResult {
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
                    TraceDirection::Outbound => outbound && (is_call || edge.edge_type == EdgeType::Imports),
                    TraceDirection::Inbound => !outbound && (is_call || edge.edge_type == EdgeType::Imports),
                    TraceDirection::Both => is_call || edge.edge_type == EdgeType::Imports || edge.edge_type == EdgeType::Contains,
                };
                if !include {
                    continue;
                }
                if !visited.insert(neighbor_id.clone()) {
                    continue;
                }
                if let Some(neighbor) = self.get_node(&neighbor_id) {
                    if let Some(from) = self.get_node(&id) {
                        let to_hit = SearchHit::from_node(&neighbor, 1.0 - hop as f32 * 0.12, format!("{:?}", edge.edge_type));
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
                let name = node.file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if matches!(name, "main.rs" | "lib.rs" | "mod.rs" | "index.ts" | "index.js" | "main.py" | "main.go")
                {
                    entry_points.push(SearchHit::from_node(node, 1.0, "entry"));
                }
            } else {
                symbol_count += 1;
                let pkg = package_name(&node.file_path);
                *package_symbols.entry(pkg).or_insert(0) += 1;
            }
        }

        let mut languages: Vec<(String, usize)> = lang_counts.into_iter().collect();
        languages.sort_by(|a, b| b.1.cmp(&a.1));

        let mut packages: Vec<ArchitecturePackage> = package_files
            .into_iter()
            .map(|(name, file_count)| ArchitecturePackage {
                symbol_count: package_symbols.get(&name).copied().unwrap_or(0),
                name,
                file_count,
            })
            .collect();
        packages.sort_by(|a, b| b.file_count.cmp(&a.file_count));
        packages.truncate(16);

        let mut hotspots: Vec<SearchHit> = degree
            .into_iter()
            .filter_map(|(id, deg)| {
                data.nodes.get(&id).map(|n| SearchHit::from_node(n, deg as f32, "degree"))
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

    pub fn solve_physarum_local(&self, seed_nodes: &HashSet<NodeId>, hops: usize) -> PhysarumResult {
        let neighborhood = self.neighborhood(seed_nodes, hops);
        let (nodes_map, edges_map) = self.subgraph_maps(&neighborhood);
        self.physarum_solver
            .optimize_subgraph(&nodes_map, &edges_map, seed_nodes)
    }

    /// Record a neural firing event (e.g. symbol read or written by AI agent)
    pub fn record_neural_spike(&self, node_id: NodeId, was_modified: bool, was_useful: bool) {
        self.synaptic_engine
            .write()
            .record_spike(node_id, was_modified, was_useful);
    }

    /// Applies Spike-Timing-Dependent Plasticity (STDP) across active paths
    pub fn apply_stdp_learning(&self) {
        let mut data = self.inner.write();
        let engine = self.synaptic_engine.read();

        for edge in data.edges.values_mut() {
            engine.apply_stdp(edge);
        }

        // Apply homeostasis
        let mut edge_vec: Vec<ContextEdge> = data.edges.values().cloned().collect();
        engine.apply_homeostasis(&mut edge_vec);
        for edge in edge_vec {
            data.edges.insert(edge.id.clone(), edge);
        }
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

fn package_name(path: &Path) -> String {
    let parts: Vec<String> = path
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    if let Some(idx) = parts.iter().position(|p| p == "crates" || p == "packages" || p == "apps")
    {
        if let Some(name) = parts.get(idx + 1) {
            return name.clone();
        }
    }
    parts.first().cloned().unwrap_or_else(|| "root".into())
}
