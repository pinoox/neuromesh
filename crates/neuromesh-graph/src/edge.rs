use chrono::Utc;
use neuromesh_core::{ContextEdge, EdgeConfidence, EdgeId, EdgeType, NodeId, ProjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneConfig {
    pub default_weight: f32,
    pub min_weight: f32,
    pub max_weight: f32,
    pub evaporation_rate: f32,
    pub success_reinforcement: f32,
    pub failure_penalty: f32,
}

impl Default for PheromoneConfig {
    fn default() -> Self {
        Self {
            default_weight: 0.5,
            min_weight: 0.05,
            max_weight: 1.0,
            evaporation_rate: 0.02,
            success_reinforcement: 0.15,
            failure_penalty: 0.10,
        }
    }
}

pub struct PheromoneEngine {
    config: PheromoneConfig,
}

impl PheromoneEngine {
    pub fn new(config: PheromoneConfig) -> Self {
        Self { config }
    }

    pub fn create_edge(
        &self,
        project_id: ProjectId,
        source: NodeId,
        target: NodeId,
        edge_type: EdgeType,
    ) -> ContextEdge {
        let id = EdgeId::new(&source, &target, &edge_type);
        ContextEdge {
            id,
            project_id,
            source,
            target,
            edge_type,
            pheromone_weight: self.config.default_weight,
            reinforcement_count: 0,
            failure_count: 0,
            last_reinforced: Utc::now(),
            confidence: EdgeConfidence::Proven,
        }
    }

    pub fn create_edge_with_confidence(
        &self,
        project_id: ProjectId,
        source: NodeId,
        target: NodeId,
        edge_type: EdgeType,
        confidence: EdgeConfidence,
    ) -> ContextEdge {
        let mut edge = self.create_edge(project_id, source, target, edge_type);
        edge.confidence = confidence;
        if confidence == EdgeConfidence::Likely {
            edge.pheromone_weight = (edge.pheromone_weight * 0.75).max(self.config.min_weight);
        }
        edge
    }

    /// Reinforces an edge after a successful task
    pub fn reinforce_success(&self, edge: &mut ContextEdge, depth: usize) {
        let depth_decay = 1.0 / (depth as f32 + 1.0);
        let boost = self.config.success_reinforcement * depth_decay;
        edge.pheromone_weight = (edge.pheromone_weight + boost).min(self.config.max_weight);
        edge.reinforcement_count += 1;
        edge.last_reinforced = Utc::now();
    }

    /// Penalizes an edge after an irrelevant or failed activation
    pub fn penalize_failure(&self, edge: &mut ContextEdge) {
        edge.pheromone_weight =
            (edge.pheromone_weight - self.config.failure_penalty).max(self.config.min_weight);
        edge.failure_count += 1;
        edge.last_reinforced = Utc::now();
    }

    /// Simulates biological pheromone evaporation
    pub fn evaporate(&self, edge: &mut ContextEdge) {
        let evaporated = edge.pheromone_weight * (1.0 - self.config.evaporation_rate);
        edge.pheromone_weight = evaporated.max(self.config.min_weight);
    }
}
