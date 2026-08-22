use chrono::{DateTime, Utc};
use neuromesh_core::{ContextEdge, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for Spike-Timing-Dependent Plasticity (STDP)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdpConfig {
    /// LTP learning rate amplitude (A+)
    pub ltp_amplitude: f32,
    /// LTD depression rate amplitude (A-)
    pub ltd_amplitude: f32,
    /// Positive time window constant (tau+ in seconds, e.g. 120s)
    pub tau_plus_seconds: f32,
    /// Negative time window constant (tau- in seconds, e.g. 120s)
    pub tau_minus_seconds: f32,
    /// Maximum allowable synaptic weight
    pub max_synaptic_weight: f32,
    /// Minimum synaptic weight floor
    pub min_synaptic_weight: f32,
    /// Homeostatic target sum for incoming synapses per node
    pub homeostatic_target_sum: f32,
}

impl Default for StdpConfig {
    fn default() -> Self {
        Self {
            ltp_amplitude: 0.20,
            ltd_amplitude: 0.15,
            tau_plus_seconds: 180.0,
            tau_minus_seconds: 180.0,
            max_synaptic_weight: 1.0,
            min_synaptic_weight: 0.05,
            homeostatic_target_sum: 3.5,
        }
    }
}

/// A recorded neural spike event (node activation or modification)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralSpike {
    pub node_id: NodeId,
    pub timestamp: DateTime<Utc>,
    pub was_modified: bool,
    pub was_useful: bool,
}

/// Biologically-inspired Synaptic Plasticity Engine
#[derive(Debug, Clone)]
pub struct SynapticPlasticityEngine {
    config: StdpConfig,
    spike_history: HashMap<NodeId, Vec<NeuralSpike>>,
}

impl SynapticPlasticityEngine {
    pub fn new(config: StdpConfig) -> Self {
        Self {
            config,
            spike_history: HashMap::new(),
        }
    }

    /// Record a spike event when a node is activated or touched by the agent
    pub fn record_spike(&mut self, node_id: NodeId, was_modified: bool, was_useful: bool) {
        let spike = NeuralSpike {
            node_id: node_id.clone(),
            timestamp: Utc::now(),
            was_modified,
            was_useful,
        };
        self.spike_history.entry(node_id).or_default().push(spike);
    }

    /// Applies Spike-Timing-Dependent Plasticity (STDP) to an edge connecting pre-synaptic and post-synaptic nodes
    pub fn apply_stdp(&self, edge: &mut ContextEdge) {
        let pre_spikes = self.spike_history.get(&edge.source);
        let post_spikes = self.spike_history.get(&edge.target);

        if let (Some(pre_list), Some(post_list)) = (pre_spikes, post_spikes) {
            let mut total_delta: f32 = 0.0;

            for pre in pre_list {
                for post in post_list {
                    let dt = (post.timestamp - pre.timestamp).num_milliseconds() as f32 / 1000.0;

                    if dt > 0.0 {
                        // Pre fired before Post: Causal connection -> Long-Term Potentiation (LTP)
                        let ltp =
                            self.config.ltp_amplitude * (-dt / self.config.tau_plus_seconds).exp();
                        let boost = if post.was_useful || post.was_modified {
                            ltp * 1.5
                        } else {
                            ltp
                        };
                        total_delta += boost;
                    } else if dt < 0.0 {
                        // Post fired before Pre: Anti-causal connection -> Long-Term Depression (LTD)
                        let ltd =
                            self.config.ltd_amplitude * (dt / self.config.tau_minus_seconds).exp();
                        total_delta -= ltd;
                    }
                }
            }

            edge.pheromone_weight = (edge.pheromone_weight + total_delta).clamp(
                self.config.min_synaptic_weight,
                self.config.max_synaptic_weight,
            );
            if total_delta > 0.0 {
                edge.reinforcement_count += 1;
            } else if total_delta < 0.0 {
                edge.failure_count += 1;
            }
            edge.last_reinforced = Utc::now();
        }
    }

    /// Enforces biological synaptic homeostasis to prevent hyper-excitation
    pub fn apply_homeostasis(&self, edges: &mut [ContextEdge]) {
        let mut node_incoming_sum: HashMap<NodeId, f32> = HashMap::new();

        for edge in edges.iter() {
            let entry = node_incoming_sum.entry(edge.target.clone()).or_insert(0.0);
            *entry += edge.pheromone_weight;
        }

        for edge in edges.iter_mut() {
            if let Some(&sum) = node_incoming_sum.get(&edge.target) {
                if sum > self.config.homeostatic_target_sum && sum > 0.0 {
                    let scale = self.config.homeostatic_target_sum / sum;
                    edge.pheromone_weight = (edge.pheromone_weight * scale).clamp(
                        self.config.min_synaptic_weight,
                        self.config.max_synaptic_weight,
                    );
                }
            }
        }
    }

    pub fn clear_spikes(&mut self) {
        self.spike_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::{EdgeId, EdgeType, ProjectId};

    #[test]
    fn test_stdp_causal_potentiation() {
        let mut engine = SynapticPlasticityEngine::new(StdpConfig::default());
        let project_id = ProjectId::new("test");
        let n1 = NodeId::new("file:a.rs");
        let n2 = NodeId::new("file:b.rs");

        // Pre spike at t0
        engine.record_spike(n1.clone(), false, true);

        // Wait a tiny bit and post spike at t1 > t0
        std::thread::sleep(std::time::Duration::from_millis(10));
        engine.record_spike(n2.clone(), true, true);

        let mut edge = ContextEdge {
            id: EdgeId::new(&n1, &n2, &EdgeType::Calls),
            project_id,
            source: n1,
            target: n2,
            edge_type: EdgeType::Calls,
            pheromone_weight: 0.5,
            reinforcement_count: 0,
            failure_count: 0,
            last_reinforced: Utc::now(),
        };
        let initial_weight = edge.pheromone_weight;

        engine.apply_stdp(&mut edge);
        assert!(
            edge.pheromone_weight > initial_weight,
            "Causal firing must potentiate edge (LTP)"
        );
    }
}
