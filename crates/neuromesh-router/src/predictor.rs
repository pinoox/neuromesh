use neuromesh_core::NodeId;
use neuromesh_graph::NeuralProjectGraph;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedContextItem {
    pub node_id: NodeId,
    pub name: String,
    pub probability: f32,
}

pub struct ContextPredictor;

impl ContextPredictor {
    pub fn predict_next(
        graph: &NeuralProjectGraph,
        current_node_id: &NodeId,
    ) -> Vec<PredictedContextItem> {
        let neighbors = graph.get_connected_neighbors(current_node_id);
        let mut predictions = Vec::new();

        for (neighbor_id, edge) in neighbors {
            if let Some(node) = graph.get_node(&neighbor_id) {
                let weight = edge.pheromone_weight;
                let attenuation = edge.edge_type.attenuation();
                let probability = (weight * attenuation).clamp(0.0, 1.0);

                predictions.push(PredictedContextItem {
                    node_id: neighbor_id,
                    name: node.name,
                    probability,
                });
            }
        }

        predictions.sort_by(|a, b| {
            b.probability
                .partial_cmp(&a.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        predictions
    }
}
