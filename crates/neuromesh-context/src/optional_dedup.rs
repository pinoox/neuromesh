//! Optional-file cosine dedup and module-cluster score bonus.

use crate::emission::EmissionPipeline;
use neuromesh_core::{EmissionDropStage, NodeId, NodeType};
use neuromesh_embed::cosine_similarity;
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashMap;

/// Paths under test/mock trees must not be deduped against implementation files.
pub fn is_test_or_mock_path(path: &str) -> bool {
    let p = path.replace('\\', "/").to_lowercase();
    p.starts_with("test/")
        || p.starts_with("tests/")
        || p.contains("/test/")
        || p.contains("/tests/")
        || p.contains("__tests__")
        || p.contains("__mocks__")
        || p.contains("/mock/")
        || p.ends_with("_test.rs")
        || p.contains(".test.")
        || p.contains(".spec.")
        || p.contains(".mock.")
        || p.contains("_mock.")
}

fn file_mean_vector(graph: &NeuralProjectGraph, file_id: &NodeId) -> Option<Vec<f32>> {
    let file_node = graph.get_node(file_id)?;
    if file_node.node_type != NodeType::File {
        return None;
    }
    let file_path = file_node.file_path.clone();
    let index = graph.embedding_index();
    if !index.is_loaded() {
        return None;
    }
    let dim = index.dim;
    let mut sum = vec![0.0f32; dim];
    let mut count = 0usize;
    for (i, node_id) in index.node_ids.iter().enumerate() {
        let Some(node) = graph.get_node(node_id) else {
            continue;
        };
        if node.file_path != file_path || node.node_type == NodeType::File {
            continue;
        }
        let start = i * dim;
        let end = start + dim;
        if end > index.vectors.len() {
            continue;
        }
        for (j, v) in index.vectors[start..end].iter().enumerate() {
            sum[j] += v;
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let n = count as f32;
    for v in &mut sum {
        *v /= n;
    }
    let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut sum {
            *v /= norm;
        }
    }
    Some(sum)
}

fn parent_dir(path: &std::path::Path) -> String {
    path.parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".into())
}

fn module_centroid_for_dir<'a>(
    index: &'a neuromesh_graph::EmbeddingIndex,
    dir: &str,
) -> Option<&'a [f32]> {
    index
        .module_centroids
        .iter()
        .find(|c| c.dir == dir)
        .map(|c| c.vector.as_slice())
}

/// Small routing bonus from directory centroids (metadata-only; capped).
pub fn apply_module_cluster_bonus(
    graph: &NeuralProjectGraph,
    seed_files: &std::collections::HashSet<NodeId>,
    optional: &mut [NodeId],
    scores: &mut HashMap<NodeId, f32>,
) {
    let index = graph.embedding_index();
    if index.module_centroids.is_empty() {
        return;
    }
    let seed_dirs: Vec<String> = seed_files
        .iter()
        .filter_map(|id| graph.get_node(id))
        .map(|n| parent_dir(&n.file_path))
        .collect();
    if seed_dirs.is_empty() {
        return;
    }
    for id in optional.iter() {
        let Some(node) = graph.get_node(id) else {
            continue;
        };
        let dir = parent_dir(&node.file_path);
        let Some(centroid) = module_centroid_for_dir(&index, &dir) else {
            continue;
        };
        let mut best = 0.0f32;
        for seed_dir in &seed_dirs {
            if let Some(seed_centroid) = module_centroid_for_dir(&index, seed_dir) {
                best = best.max(cosine_similarity(seed_centroid, centroid));
            }
        }
        if best > 0.0 {
            let bonus = (0.05 * best).min(0.05);
            let entry = scores.entry(id.clone()).or_insert(8.0);
            *entry += bonus;
        }
    }
}

/// Greedy optional-file dedup by mean symbol-vector cosine (test/mock paths exempt).
pub fn dedup_optional_files(
    graph: &NeuralProjectGraph,
    optional: &mut Vec<NodeId>,
    scores: &HashMap<NodeId, f32>,
    pipeline: &mut EmissionPipeline,
    threshold: f32,
) {
    if optional.len() <= 2 {
        return;
    }
    let mut kept: Vec<(NodeId, Vec<f32>, String)> = Vec::new();
    let mut next = Vec::new();
    for id in optional.drain(..) {
        let path = graph
            .get_node(&id)
            .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if is_test_or_mock_path(&path) {
            next.push(id);
            continue;
        }
        let Some(vec) = file_mean_vector(graph, &id) else {
            next.push(id);
            continue;
        };
        let dominated = kept.iter().any(|(kept_id, kept_vec, kept_path)| {
            if is_test_or_mock_path(kept_path) {
                return false;
            }
            let kept_score = scores.get(kept_id).copied().unwrap_or(0.0);
            let cand_score = scores.get(&id).copied().unwrap_or(0.0);
            if cand_score > kept_score {
                return false;
            }
            cosine_similarity(kept_vec, &vec) >= threshold
        });
        if dominated {
            pipeline.record_drop(&id, EmissionDropStage::SemanticDuplicate);
        } else {
            kept.push((id.clone(), vec, path));
            next.push(id);
        }
    }
    *optional = next;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_exempt() {
        assert!(!is_test_or_mock_path("src/auth/handler.rs"));
        assert!(is_test_or_mock_path("src/__tests__/auth.test.ts"));
        assert!(is_test_or_mock_path("tests/integration/foo.rs"));
        assert!(is_test_or_mock_path("src/mock/server.mock.ts"));
        assert!(!is_test_or_mock_path("src/middleware/auth.rs"));
    }
}
