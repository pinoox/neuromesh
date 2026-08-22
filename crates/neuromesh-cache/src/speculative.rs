use neuromesh_core::NodeId;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct TransitionNode {
    pub targets: HashMap<NodeId, f32>, // Target Node -> Frequency / Transition Weight
    pub total_exits: f32,
}

pub struct SpeculativePrefetcher {
    transitions: Arc<RwLock<HashMap<NodeId, TransitionNode>>>,
    l1_cache: Arc<RwLock<HashMap<NodeId, String>>>, // In-memory pre-warmed skeleton cache
    max_hops: usize,
    top_k: usize,
}

impl Default for SpeculativePrefetcher {
    fn default() -> Self {
        Self::new(3, 5)
    }
}

impl SpeculativePrefetcher {
    pub fn new(max_hops: usize, top_k: usize) -> Self {
        Self {
            transitions: Arc::new(RwLock::new(HashMap::new())),
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            max_hops,
            top_k,
        }
    }

    pub fn record_transition(&self, from: &NodeId, to: &NodeId) {
        if from == to {
            return;
        }

        let mut lock = self.transitions.write();
        let entry = lock.entry(from.clone()).or_default();
        let current_weight = entry.targets.entry(to.clone()).or_insert(0.0);
        *current_weight += 1.0;
        entry.total_exits += 1.0;
    }

    pub fn predict_multi_hop(&self, seed: &NodeId) -> Vec<(NodeId, f32)> {
        let lock = self.transitions.read();
        let mut predicted = HashMap::new();
        let mut current_frontier = vec![(seed.clone(), 1.0f32)];

        for _hop in 0..self.max_hops {
            let mut next_frontier = Vec::new();

            for (curr_node, curr_prob) in current_frontier {
                if let Some(trans_node) = lock.get(&curr_node) {
                    if trans_node.total_exits > 0.0 {
                        for (target, count) in &trans_node.targets {
                            if target != seed {
                                let trans_prob = count / trans_node.total_exits;
                                let path_prob = curr_prob * trans_prob;

                                let existing = predicted.entry(target.clone()).or_insert(0.0f32);
                                *existing = existing.max(path_prob);

                                next_frontier.push((target.clone(), path_prob));
                            }
                        }
                    }
                }
            }

            current_frontier = next_frontier;
        }

        let mut results: Vec<(NodeId, f32)> = predicted.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(self.top_k);
        results
    }

    pub fn prewarm(&self, node_id: &NodeId, skeleton_content: String) {
        self.l1_cache
            .write()
            .insert(node_id.clone(), skeleton_content);
    }

    pub fn get_prewarmed(&self, node_id: &NodeId) -> Option<String> {
        self.l1_cache.read().get(node_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speculative_multi_hop() {
        let prefetcher = SpeculativePrefetcher::new(3, 3);
        let n1 = NodeId::new("Header.vue");
        let n2 = NodeId::new("CartDrawer.vue");
        let n3 = NodeId::new("cartStore.ts");

        prefetcher.record_transition(&n1, &n2);
        prefetcher.record_transition(&n2, &n3);

        let preds = prefetcher.predict_multi_hop(&n1);
        assert!(!preds.is_empty());
        let pred_ids: Vec<NodeId> = preds.into_iter().map(|(id, _)| id).collect();
        assert!(pred_ids.contains(&n2));
        assert!(pred_ids.contains(&n3));
    }
}
