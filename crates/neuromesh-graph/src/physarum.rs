use neuromesh_core::{ContextEdge, ContextNode, EdgeId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for Physarum Polycephalum (Slime Mold) Network Optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysarumConfig {
    /// Tube conductivity decay rate (atrophy) per iteration (mu in [0.01, 0.20])
    pub decay_rate: f32,
    /// Flux responsiveness exponent (gamma in [1.0, 1.8])
    pub flux_exponent: f32,
    /// Growth scaling factor (alpha)
    pub growth_factor: f32,
    /// Number of cytoplasmic streaming iterations
    pub max_iterations: usize,
    /// Successive over-relaxation inner loops per iteration
    pub sor_iterations: usize,
    /// Minimum tube conductivity threshold to avoid total degeneration
    pub min_conductivity: f32,
    /// Maximum conductivity cap
    pub max_conductivity: f32,
    /// Threshold below which an edge is considered atrophied (pruned)
    pub prune_threshold: f32,
}

impl Default for PhysarumConfig {
    fn default() -> Self {
        Self {
            decay_rate: 0.08,
            flux_exponent: 1.25,
            growth_factor: 0.35,
            max_iterations: 15,
            sor_iterations: 25,
            min_conductivity: 0.02,
            max_conductivity: 5.0,
            prune_threshold: 0.12,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysarumResult {
    /// Optimal minimal Steiner nodes kept by the slime mold network
    pub active_nodes: HashSet<NodeId>,
    /// Flux intensity per node (cytoplasmic concentration)
    pub node_flux: HashMap<NodeId, f32>,
    /// High-bandwidth conduit edges (tubes) surviving atrophy
    pub active_edges: HashSet<EdgeId>,
    /// Edge conductivities after simulation
    pub edge_conductance: HashMap<EdgeId, f32>,
    /// Percentage of graph pruned
    pub pruning_ratio: f32,
    /// Iterations executed until convergence
    pub iterations_converged: usize,
}

/// Physarum Polycephalum Network Solver
pub struct PhysarumSolver {
    config: PhysarumConfig,
}

impl PhysarumConfig {
    /// Fast neighborhood solve for the `get_context` hot path (&lt; 20ms target).
    pub fn hot_path() -> Self {
        Self {
            decay_rate: 0.10,
            flux_exponent: 1.2,
            growth_factor: 0.40,
            max_iterations: 6,
            sor_iterations: 8,
            min_conductivity: 0.02,
            max_conductivity: 5.0,
            prune_threshold: 0.12,
        }
    }
}

impl PhysarumSolver {
    pub fn new(config: PhysarumConfig) -> Self {
        Self { config }
    }

    /// Solves the optimal minimal context subgraph connecting multiple seed nodes
    /// using slime mold cytoplasmic streaming simulation (Hagen-Poiseuille flux dynamics).
    pub fn optimize_subgraph(
        &self,
        nodes: &HashMap<NodeId, ContextNode>,
        edges: &HashMap<EdgeId, ContextEdge>,
        seed_nodes: &HashSet<NodeId>,
    ) -> PhysarumResult {
        if seed_nodes.is_empty() || nodes.is_empty() {
            return PhysarumResult {
                active_nodes: seed_nodes.clone(),
                node_flux: HashMap::new(),
                active_edges: HashSet::new(),
                edge_conductance: HashMap::new(),
                pruning_ratio: 0.0,
                iterations_converged: 0,
            };
        }

        // If only 1 seed node, no Steiner tree needed; return seed plus immediate high-weight neighbors
        if seed_nodes.len() == 1 {
            let seed = seed_nodes.iter().next().unwrap();
            let mut active = HashSet::new();
            active.insert(seed.clone());
            let mut node_flux = HashMap::new();
            node_flux.insert(seed.clone(), 1.0);

            return PhysarumResult {
                active_nodes: active,
                node_flux,
                active_edges: HashSet::new(),
                edge_conductance: HashMap::new(),
                pruning_ratio: 0.0,
                iterations_converged: 1,
            };
        }

        // Initialize adjacency map: NodeId -> Vec<(NeighborId, EdgeId, length)>
        let mut adj: HashMap<NodeId, Vec<(NodeId, EdgeId, f32)>> = HashMap::new();
        let mut conductance: HashMap<EdgeId, f32> = HashMap::new();

        for (edge_id, edge) in edges {
            let length = (1.0 / (edge.pheromone_weight.max(0.1))).clamp(0.5, 5.0);
            let initial_d = (edge.pheromone_weight * 2.0).clamp(0.2, 1.0);
            conductance.insert(edge_id.clone(), initial_d);

            adj.entry(edge.source.clone()).or_default().push((
                edge.target.clone(),
                edge_id.clone(),
                length,
            ));
            adj.entry(edge.target.clone()).or_default().push((
                edge.source.clone(),
                edge_id.clone(),
                length,
            ));
        }

        let seed_list: Vec<NodeId> = seed_nodes.iter().cloned().collect();
        let source_node = &seed_list[0];
        let sink_nodes: HashSet<NodeId> = seed_list[1..].iter().cloned().collect();

        let mut node_flux: HashMap<NodeId, f32> = HashMap::new();
        let mut edge_flux: HashMap<EdgeId, f32> = HashMap::new();
        let mut iterations_converged = self.config.max_iterations;

        // Run Physarum iterations
        for iter in 0..self.config.max_iterations {
            let mut potentials: HashMap<NodeId, f32> = HashMap::new();
            // Source has positive potential, sinks negative
            potentials.insert(source_node.clone(), 10.0);
            for sink in &sink_nodes {
                potentials.insert(sink.clone(), 0.0);
            }

            // Successive Over-Relaxation (SOR) to compute node pressures p_i
            let sor_iters = self.config.sor_iterations.max(4);
            for _ in 0..sor_iters {
                let mut max_delta: f32 = 0.0;
                for (node_id, neighbors) in &adj {
                    if node_id == source_node || sink_nodes.contains(node_id) {
                        continue;
                    }

                    let mut weighted_pot_sum = 0.0;
                    let mut total_conductivity_length = 0.0;

                    for (neighbor_id, edge_id, length) in neighbors {
                        let d = conductance.get(edge_id).copied().unwrap_or(0.1);
                        let c = d / length.max(0.1);
                        let p_neigh = potentials.get(neighbor_id).copied().unwrap_or(5.0);

                        weighted_pot_sum += c * p_neigh;
                        total_conductivity_length += c;
                    }

                    if total_conductivity_length > 0.0001 {
                        let new_pot = weighted_pot_sum / total_conductivity_length;
                        let old_pot = potentials.get(node_id).copied().unwrap_or(5.0);
                        let delta = (new_pot - old_pot).abs();
                        if delta > max_delta {
                            max_delta = delta;
                        }
                        potentials.insert(node_id.clone(), new_pot);
                    }
                }

                if max_delta < 0.001 {
                    break;
                }
            }

            // Calculate flux Q_ij = (D_ij / L_ij) * |p_i - p_j|
            let mut max_flux_change: f32 = 0.0;
            node_flux.clear();
            edge_flux.clear();

            for (edge_id, edge) in edges {
                let p_src = potentials.get(&edge.source).copied().unwrap_or(5.0);
                let p_tgt = potentials.get(&edge.target).copied().unwrap_or(5.0);
                let d = conductance.get(edge_id).copied().unwrap_or(0.1);
                let length = (1.0 / (edge.pheromone_weight.max(0.1))).clamp(0.5, 5.0);

                let flux = (d / length) * (p_src - p_tgt).abs();
                edge_flux.insert(edge_id.clone(), flux);

                let src_entry = node_flux.entry(edge.source.clone()).or_insert(0.0);
                *src_entry += flux;
                let tgt_entry = node_flux.entry(edge.target.clone()).or_insert(0.0);
                *tgt_entry += flux;

                // Slime mold tube adaptation: D_new = (1 - decay) * D_old + growth * (|Q|^gamma)
                let growth = self.config.growth_factor * flux.powf(self.config.flux_exponent);
                let new_d = ((1.0 - self.config.decay_rate) * d + growth)
                    .clamp(self.config.min_conductivity, self.config.max_conductivity);

                let change = (new_d - d).abs();
                if change > max_flux_change {
                    max_flux_change = change;
                }
                conductance.insert(edge_id.clone(), new_d);
            }

            if max_flux_change < 0.005 {
                iterations_converged = iter + 1;
                break;
            }
        }

        // Normalize node flux
        let max_node_flux = node_flux.values().cloned().fold(0.0f32, f32::max).max(0.01);
        for flux in node_flux.values_mut() {
            *flux /= max_node_flux;
        }

        // Extract surviving active edges and active Steiner nodes
        let mut active_edges = HashSet::new();
        let mut active_nodes = HashSet::new();

        let max_edge_flux = edge_flux
            .values()
            .cloned()
            .fold(0.0f32, f32::max)
            .max(0.001);

        // Always include all seed nodes
        for seed in seed_nodes {
            active_nodes.insert(seed.clone());
        }

        for (edge_id, &d) in &conductance {
            let flux = edge_flux.get(edge_id).copied().unwrap_or(0.0);
            if flux >= 0.08 * max_edge_flux && d >= self.config.prune_threshold {
                active_edges.insert(edge_id.clone());
                if let Some(edge) = edges.get(edge_id) {
                    active_nodes.insert(edge.source.clone());
                    active_nodes.insert(edge.target.clone());
                }
            }
        }

        let total_nodes = nodes.len();
        let pruned_count = total_nodes.saturating_sub(active_nodes.len());
        let pruning_ratio = if total_nodes > 0 {
            (pruned_count as f32 / total_nodes as f32) * 100.0
        } else {
            0.0
        };

        PhysarumResult {
            active_nodes,
            node_flux,
            active_edges,
            edge_conductance: conductance,
            pruning_ratio,
            iterations_converged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use neuromesh_core::{EdgeType, NodeType, ProjectId};
    use std::path::PathBuf;

    #[test]
    fn test_physarum_solver_steiner_path() {
        let project_id = ProjectId::new("test");
        let n1 = NodeId::new("file:a.rs");
        let n2 = NodeId::new("file:intermediate.rs");
        let n3 = NodeId::new("file:b.rs");
        let n_irrelevant = NodeId::new("file:irrelevant.rs");

        let mut nodes = HashMap::new();
        let make_node = |id: &NodeId, path: &str| ContextNode {
            id: id.clone(),
            project_id: project_id.clone(),
            file_path: PathBuf::from(path),
            node_type: NodeType::File,
            name: path.to_string(),
            signature: None,
            doc_summary: None,
            line_range: None,
            token_cost: 100,
            content: None,
            content_hash: "hash".into(),
            parent: None,
            base_relevance: 1.0,
            access_count: 1,
            last_accessed: Utc::now(),
        };

        nodes.insert(n1.clone(), make_node(&n1, "a.rs"));
        nodes.insert(n2.clone(), make_node(&n2, "intermediate.rs"));
        nodes.insert(n3.clone(), make_node(&n3, "b.rs"));
        nodes.insert(
            n_irrelevant.clone(),
            make_node(&n_irrelevant, "irrelevant.rs"),
        );

        let make_edge = |src: &NodeId, tgt: &NodeId, edge_type: EdgeType| ContextEdge {
            id: EdgeId::new(src, tgt, &edge_type),
            project_id: project_id.clone(),
            source: src.clone(),
            target: tgt.clone(),
            edge_type,
            pheromone_weight: 0.6,
            reinforcement_count: 1,
            failure_count: 0,
            last_reinforced: Utc::now(),
            confidence: neuromesh_core::EdgeConfidence::Proven,
        };

        let mut edges = HashMap::new();
        let e1 = make_edge(&n1, &n2, EdgeType::Imports);
        let e2 = make_edge(&n2, &n3, EdgeType::Imports);
        let e_irr = make_edge(&n1, &n_irrelevant, EdgeType::References);

        edges.insert(e1.id.clone(), e1);
        edges.insert(e2.id.clone(), e2);
        edges.insert(e_irr.id.clone(), e_irr);

        let mut seeds = HashSet::new();
        seeds.insert(n1.clone());
        seeds.insert(n3.clone());

        let solver = PhysarumSolver::new(PhysarumConfig::default());
        let res = solver.optimize_subgraph(&nodes, &edges, &seeds);

        assert!(res.active_nodes.contains(&n1));
        assert!(res.active_nodes.contains(&n2)); // Intermediate connector preserved!
        assert!(res.active_nodes.contains(&n3));
        assert!(!res.active_nodes.contains(&n_irrelevant)); // Irrelevant branch pruned!
    }
}
