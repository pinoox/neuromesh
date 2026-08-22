use crate::graph::NeuralProjectGraph;
use crate::physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
use neuromesh_core::NodeId;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct SpreadingActivationConfig {
    pub decay_factor: f32,      // gamma (e.g. 0.65)
    pub max_hops: usize,        // max propagation depth (e.g. 3 or 4)
    pub min_energy_cutoff: f32, // energy below which propagation halts (e.g. 0.05)
    pub enable_physarum: bool,  // enable Physarum slime mold network optimization
    pub physarum_config: PhysarumConfig,
}

impl Default for SpreadingActivationConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.65,
            max_hops: 3,
            min_energy_cutoff: 0.05,
            enable_physarum: true,
            physarum_config: PhysarumConfig::default(),
        }
    }
}

pub struct SpreadingActivation {
    config: SpreadingActivationConfig,
    physarum_solver: PhysarumSolver,
}

impl SpreadingActivation {
    pub fn new(config: SpreadingActivationConfig) -> Self {
        let physarum_solver = PhysarumSolver::new(config.physarum_config.clone());
        Self {
            config,
            physarum_solver,
        }
    }

    /// Executes spreading activation over the project graph starting from seed nodes
    pub fn activate(
        &self,
        graph: &NeuralProjectGraph,
        seed_energies: &HashMap<NodeId, f32>,
    ) -> HashMap<NodeId, f32> {
        let mut current_energies: HashMap<NodeId, f32> = seed_energies.clone();
        let mut final_energies: HashMap<NodeId, f32> = seed_energies.clone();

        for hop in 0..self.config.max_hops {
            let mut next_energies: HashMap<NodeId, f32> = HashMap::new();
            let mut visited_in_hop = HashSet::new();

            for (node_id, &energy) in &current_energies {
                if energy < self.config.min_energy_cutoff {
                    continue;
                }

                // Get outgoing and incoming neighbors
                let neighbors = graph.get_connected_neighbors(node_id);
                for (neighbor_id, edge) in neighbors {
                    let weight = edge.pheromone_weight;
                    let attenuation = edge.edge_type.attenuation();
                    let spread_energy = energy * self.config.decay_factor * weight * attenuation;

                    if spread_energy >= self.config.min_energy_cutoff {
                        let entry = next_energies.entry(neighbor_id.clone()).or_insert(0.0);
                        *entry = (*entry).max(spread_energy);
                        visited_in_hop.insert(neighbor_id);
                    }
                }
            }

            if next_energies.is_empty() {
                break;
            }

            for (node_id, &energy) in &next_energies {
                let current_val = final_energies.entry(node_id.clone()).or_insert(0.0);
                *current_val = (*current_val).max(energy * (1.0 - (hop as f32 * 0.15)));
            }

            current_energies = next_energies;
        }

        // Apply Physarum only on modest graphs. Full-graph SOR on 10k+ edges times out.
        if self.config.enable_physarum
            && seed_energies.len() > 1
            && graph.stats().total_edges <= 8_000
        {
            let seed_set: HashSet<NodeId> = seed_energies.keys().cloned().collect();
            let nodes_map = graph.get_nodes_map();
            let edges_map = graph.get_edges_map();

            let physarum_res = self
                .physarum_solver
                .optimize_subgraph(&nodes_map, &edges_map, &seed_set);

            // Modulate energies by Physarum flux intensity & prune atrophied branches
            let mut modulated_energies = HashMap::new();
            for (node_id, &energy) in &final_energies {
                if physarum_res.active_nodes.contains(node_id) {
                    let flux = physarum_res.node_flux.get(node_id).copied().unwrap_or(0.5);
                    let boosted = energy * (0.65 + 0.35 * flux);
                    modulated_energies.insert(node_id.clone(), boosted.min(1.0));
                } else if energy >= 0.85 {
                    // Preserve high-relevance direct seeds even if peripheral
                    modulated_energies.insert(node_id.clone(), energy * 0.7);
                }
            }
            return modulated_energies;
        }

        final_energies
    }

    /// Executes full Physarum network optimization directly
    pub fn optimize_with_physarum(
        &self,
        graph: &NeuralProjectGraph,
        seed_nodes: &HashSet<NodeId>,
    ) -> PhysarumResult {
        let nodes_map = graph.get_nodes_map();
        let edges_map = graph.get_edges_map();
        self.physarum_solver
            .optimize_subgraph(&nodes_map, &edges_map, seed_nodes)
    }
}
