use chrono::Utc;
use neuromesh_core::{ContextNode, ContextScoreBreakdown, NodeId, NodeType, Thresholds};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};

/// Deterministic ordering: score DESC, path ASC.
pub fn cmp_score_path(
    a_score: f32,
    a_path: &str,
    b_score: f32,
    b_path: &str,
) -> std::cmp::Ordering {
    b_score
        .partial_cmp(&a_score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a_path.cmp(b_path))
}

pub fn file_path_str(graph: &NeuralProjectGraph, id: &NodeId) -> String {
    graph
        .get_node(id)
        .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn decay_factor(node: &ContextNode, half_life_days: f32) -> f32 {
    let age_seconds = (Utc::now() - node.last_accessed).num_seconds().max(0) as f32;
    let half_life = half_life_days * 86400.0;
    if half_life <= 0.0 {
        return 1.0;
    }
    (-0.693 * age_seconds / half_life).exp().clamp(0.1, 1.0)
}

pub fn file_matches_focus_terms(
    graph: &NeuralProjectGraph,
    file_id: &NodeId,
    focus_terms: &HashSet<String>,
) -> bool {
    let Some(node) = graph.get_node(file_id) else {
        return false;
    };
    let path_l = node
        .file_path
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    focus_terms.iter().any(|term| {
        if term.len() < 4 {
            return false;
        }
        let stem = node
            .file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        stem == *term || path_l.contains(term)
    })
}

/// Focus-aware, decay-weighted learned component for a file node.
pub fn focus_aware_learned_score(
    graph: &NeuralProjectGraph,
    file_id: &NodeId,
    learning_index: &HashMap<NodeId, f32>,
    focus_terms: &HashSet<String>,
    thresholds: &Thresholds,
) -> f32 {
    let raw = learning_index
        .get(file_id)
        .copied()
        .unwrap_or_else(|| graph.file_learning_boost(file_id));
    if raw <= 0.0 {
        return 0.0;
    }
    let decay = graph
        .get_node(file_id)
        .as_ref()
        .map(|n| decay_factor(n, thresholds.learning_decay_half_life_days))
        .unwrap_or(1.0);
    let scaled = if file_matches_focus_terms(graph, file_id, focus_terms) {
        raw * decay
    } else {
        raw * decay * thresholds.learning_relevance_cap_unrelated
    };
    scaled.min(thresholds.max_learned_influence)
}

pub fn compute_unified_file_score(
    graph: &NeuralProjectGraph,
    file_id: &NodeId,
    base_utility: f32,
    learning_index: &HashMap<NodeId, f32>,
    focus_terms: &HashSet<String>,
    thresholds: &Thresholds,
    pheromone_score: f32,
) -> ContextScoreBreakdown {
    let learned =
        focus_aware_learned_score(graph, file_id, learning_index, focus_terms, thresholds);
    let min_rel = graph.file_min_base_relevance(file_id).unwrap_or(1.0);
    let negative_penalty = if min_rel < 1.0 {
        (1.0 - min_rel) * 12.0
    } else {
        0.0
    };
    let semantic_score = if file_matches_focus_terms(graph, file_id, focus_terms) {
        base_utility * 0.15
    } else {
        0.0
    };
    let graph_score = graph
        .get_node(file_id)
        .filter(|n| n.node_type == NodeType::File)
        .map(|n| (n.base_relevance - 1.0).max(0.0) * 2.0)
        .unwrap_or(0.0);
    let utility_score = base_utility;
    let final_score = (utility_score + semantic_score + graph_score + learned + pheromone_score
        - negative_penalty)
        .clamp(0.0, 64.0);
    ContextScoreBreakdown {
        utility_score,
        semantic_score,
        graph_score,
        learned_score: learned,
        pheromone_score,
        negative_penalty,
        final_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;

    #[test]
    fn cmp_score_path_is_deterministic() {
        assert_eq!(
            cmp_score_path(10.0, "b.ts", 10.0, "a.ts"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            cmp_score_path(20.0, "z.ts", 10.0, "a.ts"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn unrelated_focus_caps_learned_score() {
        let graph = NeuralProjectGraph::new(ProjectId::new("t"));
        let thresholds = Thresholds::default();
        let file_id = NodeId::new("f");
        let mut index = HashMap::new();
        index.insert(file_id.clone(), 40.0);
        let focus: HashSet<String> = ["checkout".into()].into_iter().collect();
        let learned = focus_aware_learned_score(&graph, &file_id, &index, &focus, &thresholds);
        assert!(learned <= 40.0 * thresholds.learning_relevance_cap_unrelated + 0.01);
    }
}
