use neuromesh_core::NodeId;

/// L2-normalize in place, then keep the first `dim` components (Matryoshka).
pub fn truncate_and_normalize(vector: &mut Vec<f32>, dim: usize) {
    if vector.is_empty() {
        return;
    }
    vector.truncate(dim.min(vector.len()));
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in vector.iter_mut() {
            *x /= norm;
        }
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn ann_search(
    query: &[f32],
    node_ids: &[NodeId],
    matrix: &[f32],
    dim: usize,
    top_k: usize,
    min_cosine: f32,
) -> Vec<(NodeId, f32)> {
    if query.is_empty() || dim == 0 || node_ids.is_empty() {
        return Vec::new();
    }
    let expected = node_ids.len().saturating_mul(dim);
    if matrix.len() < expected {
        return Vec::new();
    }

    let mut scored: Vec<(NodeId, f32)> = node_ids
        .iter()
        .enumerate()
        .map(|(idx, id)| {
            let start = idx * dim;
            let slice = &matrix[start..start + dim];
            (id.clone(), cosine_similarity(query, slice))
        })
        .filter(|(_, score)| *score >= min_cosine)
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::NodeId;

    #[test]
    fn cosine_prefers_aligned_vector() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        let c = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn ann_returns_top_match() {
        let ids = vec![NodeId::new("a"), NodeId::new("b"), NodeId::new("c")];
        let matrix = vec![
            1.0, 0.0, //
            0.7, 0.7, //
            0.0, 1.0, //
        ];
        let query = vec![1.0, 0.0];
        let hits = ann_search(&query, &ids, &matrix, 2, 2, 0.5);
        assert_eq!(hits.first().map(|(id, _)| id.as_str()), Some("a"));
    }
}
