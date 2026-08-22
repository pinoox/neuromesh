use crate::activation::{SpreadingActivation, SpreadingActivationConfig};
use crate::edge::{PheromoneConfig, PheromoneEngine};
use crate::node::NodeFactory;
use crate::physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
use crate::synapse::{StdpConfig, SynapticPlasticityEngine};
use neuromesh_core::{ContextEdge, ContextNode, EdgeId, EdgeType, NodeId, NodeType, ProjectId};
use neuromesh_index::IndexedFile;
use neuromesh_parser::AstAnalysisResult;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct GraphData {
    nodes: HashMap<NodeId, ContextNode>,
    edges: HashMap<EdgeId, ContextEdge>,
    outgoing: HashMap<NodeId, Vec<EdgeId>>,
    incoming: HashMap<NodeId, Vec<EdgeId>>,
    name_to_nodes: HashMap<String, Vec<NodeId>>,
    file_to_nodes: HashMap<PathBuf, Vec<NodeId>>,
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

        node
    }

    pub fn add_edge(&self, source: NodeId, target: NodeId, edge_type: EdgeType) -> ContextEdge {
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
        let file_node = self.add_file_node(file, None);

        // 1. Add symbols
        for sym in &ast.symbols {
            let sym_node = self.add_symbol_node(
                &file.relative_path,
                &sym.name,
                sym.symbol_type,
                sym.signature.clone(),
                sym.line_range.clone(),
                15,
            );

            // File contains symbol
            self.add_edge(
                file_node.id.clone(),
                sym_node.id.clone(),
                EdgeType::Contains,
            );
        }

        // 2. Add design tokens as nodes
        for token in &ast.design_tokens {
            let token_node = self.add_symbol_node(
                &file.relative_path,
                token,
                NodeType::StyleToken,
                Some(format!("Token: {}", token)),
                1..2,
                5,
            );
            self.add_edge(
                file_node.id.clone(),
                token_node.id.clone(),
                EdgeType::References,
            );
        }

        // 3. Resolve imports and cross-file edges
        for rel in &ast.relationships {
            let target_nodes = self.find_nodes_by_name(&rel.target_symbol);
            for target in target_nodes {
                self.add_edge(file_node.id.clone(), target.id, rel.relationship);
            }
        }
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
        let data = self.inner.read();
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        if let Some(ids) = data.name_to_nodes.get(&query_lower) {
            for id in ids {
                if let Some(node) = data.nodes.get(id) {
                    results.push(node.clone());
                }
            }
        } else {
            // Partial match
            for (name, ids) in &data.name_to_nodes {
                if name.contains(&query_lower) || query_lower.contains(name) {
                    for id in ids {
                        if let Some(node) = data.nodes.get(id) {
                            if !results.iter().any(|r: &ContextNode| r.id == node.id) {
                                results.push(node.clone());
                            }
                        }
                    }
                }
            }
        }

        results
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

    pub fn spreading_activation(&self, seeds: &HashMap<NodeId, f32>) -> HashMap<NodeId, f32> {
        self.activation_engine.activate(self, seeds)
    }

    /// Bio-inspired Physarum Polycephalum Minimal Steiner Context Solver
    pub fn solve_physarum_context(&self, seed_nodes: &HashSet<NodeId>) -> PhysarumResult {
        let nodes_map = self.get_nodes_map();
        let edges_map = self.get_edges_map();
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

        GraphStats {
            total_nodes,
            total_edges,
            file_nodes,
            symbol_nodes,
            average_pheromone_weight,
            high_conductance_synapses,
            atrophied_synapses,
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
