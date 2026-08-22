use chrono::{DateTime, Utc};
use neuromesh_core::NodeId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyceliumConfig {
    pub nutrient_decay_rate: f32, // Evaporation of nutrient trails
    pub reinforcement_boost: f32, // Nutrient deposited on access
    pub branch_factor: usize,     // Top-K predicted nodes to prefetch
    pub min_gradient: f32,        // Minimum gradient to trigger prefetch
}

impl Default for MyceliumConfig {
    fn default() -> Self {
        Self {
            nutrient_decay_rate: 0.03,
            reinforcement_boost: 1.0,
            branch_factor: 3,
            min_gradient: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyphalTip {
    pub target_node: NodeId,
    pub nutrient_gradient: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MyceliumStats {
    pub total_hyphal_trails: usize,
    pub total_prefetches: u64,
    pub prefetch_hits: u64,
    pub hit_rate_pct: f32,
}

struct InnerMycelium {
    // Transition matrix: source -> (target -> nutrient_level)
    nutrient_matrix: HashMap<NodeId, HashMap<NodeId, f32>>,
    // Hot cache of pre-warmed contents: NodeId -> (content, timestamp)
    hot_tier: HashMap<NodeId, (String, DateTime<Utc>)>,
    stats: MyceliumStats,
}

#[derive(Clone)]
pub struct MyceliumCache {
    config: MyceliumConfig,
    inner: Arc<RwLock<InnerMycelium>>,
}

impl MyceliumCache {
    pub fn new(config: MyceliumConfig) -> Self {
        Self {
            config,
            inner: Arc::new(RwLock::new(InnerMycelium {
                nutrient_matrix: HashMap::new(),
                hot_tier: HashMap::new(),
                stats: MyceliumStats::default(),
            })),
        }
    }

    /// Record transition from current node to next accessed node (nutrient deposition)
    pub fn record_transition(&self, from: &NodeId, to: &NodeId) {
        if from == to {
            return;
        }

        let mut data = self.inner.write();
        let targets = data.nutrient_matrix.entry(from.clone()).or_default();
        let entry = targets.entry(to.clone()).or_insert(0.0);
        *entry += self.config.reinforcement_boost;
    }

    /// Predicts downstream nodes (hyphal tips) based on nutrient gradients
    pub fn predict_next_nodes(&self, current_node: &NodeId) -> Vec<HyphalTip> {
        let data = self.inner.read();
        let targets_opt = data.nutrient_matrix.get(current_node);

        if let Some(targets) = targets_opt {
            let total_nutrient: f32 = targets.values().sum();
            if total_nutrient <= 0.0 {
                return Vec::new();
            }

            let mut tips: Vec<HyphalTip> = targets
                .iter()
                .filter(|(_, &n)| n >= self.config.min_gradient)
                .map(|(id, &n)| HyphalTip {
                    target_node: id.clone(),
                    nutrient_gradient: n,
                    confidence: (n / total_nutrient).clamp(0.0, 1.0),
                })
                .collect();

            // Sort by nutrient gradient descending
            tips.sort_by(|a, b| {
                b.nutrient_gradient
                    .partial_cmp(&a.nutrient_gradient)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            tips.truncate(self.config.branch_factor);
            tips
        } else {
            Vec::new()
        }
    }

    /// Pre-warms a predicted node's content into the hot tier
    pub fn prewarm_node(&self, node_id: NodeId, content: String) {
        let mut data = self.inner.write();
        data.hot_tier.insert(node_id, (content, Utc::now()));
        data.stats.total_prefetches += 1;
    }

    /// Retrieve content from hot tier if available
    pub fn get_prewarmed(&self, node_id: &NodeId) -> Option<String> {
        let mut data = self.inner.write();
        if let Some((content, _)) = data.hot_tier.get(node_id) {
            let result = content.clone();
            data.stats.prefetch_hits += 1;
            if data.stats.total_prefetches > 0 {
                data.stats.hit_rate_pct =
                    (data.stats.prefetch_hits as f32 / data.stats.total_prefetches as f32) * 100.0;
            }
            Some(result)
        } else {
            None
        }
    }

    /// Simulates biological nutrient trail decay
    pub fn decay_trails(&self) {
        let mut data = self.inner.write();
        let decay = self.config.nutrient_decay_rate;

        for targets in data.nutrient_matrix.values_mut() {
            for val in targets.values_mut() {
                *val *= 1.0 - decay;
            }
            targets.retain(|_, &mut v| v > 0.05);
        }

        data.nutrient_matrix
            .retain(|_, targets| !targets.is_empty());
    }

    pub fn stats(&self) -> MyceliumStats {
        let data = self.inner.read();
        let mut s = data.stats.clone();
        s.total_hyphal_trails = data.nutrient_matrix.values().map(|t| t.len()).sum();
        s
    }

    pub fn clear(&self) {
        let mut data = self.inner.write();
        data.nutrient_matrix.clear();
        data.hot_tier.clear();
        data.stats = MyceliumStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mycelium_predictive_branching() {
        let cache = MyceliumCache::new(MyceliumConfig::default());
        let n1 = NodeId::new("Header.vue");
        let n2 = NodeId::new("Navigation.vue");
        let n3 = NodeId::new("Search.vue");

        // Simulate co-access patterns
        cache.record_transition(&n1, &n2);
        cache.record_transition(&n1, &n2);
        cache.record_transition(&n1, &n3);

        let predictions = cache.predict_next_nodes(&n1);
        assert_eq!(predictions.len(), 2);
        assert_eq!(predictions[0].target_node, n2);
        assert!(predictions[0].confidence > predictions[1].confidence);

        // Prewarm and retrieve
        cache.prewarm_node(n2.clone(), "<template><nav></nav></template>".into());
        let prewarmed = cache.get_prewarmed(&n2);
        assert!(prewarmed.is_some());
        assert_eq!(cache.stats().prefetch_hits, 1);
    }
}
