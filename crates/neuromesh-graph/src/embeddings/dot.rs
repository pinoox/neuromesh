use simsimd::SpatialSimilarity;

/// Dot product for L2-normalized f32 vectors (cosine similarity when unit length).
#[inline]
pub fn dot_f32_f32(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    f32::dot(a, b)
        .map(|d| d as f32)
        .unwrap_or_else(|| a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>())
}

/// Dot product between f32 query and int8 document with per-row `scale` (= max abs of original vector).
#[inline]
pub fn dot_f32_i8(query: &[f32], doc: &[i8], row_scale: f32) -> f32 {
    if query.len() != doc.len() || query.is_empty() || row_scale <= f32::EPSILON {
        return 0.0;
    }
    const NORM: f32 = 127.0;
    let dot_i8: f32 = query
        .iter()
        .zip(doc.iter())
        .map(|(q, v)| q * (*v as f32))
        .sum();
    dot_i8 * row_scale / NORM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_matches_scalar() {
        let a = vec![0.6f32, 0.8, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        let simd = dot_f32_f32(&a, &b);
        let scalar: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert!((simd - scalar).abs() < 1e-5);
    }

    #[test]
    fn dot_i8_approximates_f32() {
        let v = vec![0.5f32, -0.3, 0.8];
        let mut q = vec![0i8; 3];
        let row_scale = crate::embeddings::quantize::quantize_unit_vector(&v, &mut q);
        let score_i8 = dot_f32_i8(&v, &q, row_scale);
        let score_f32 = dot_f32_f32(&v, &v);
        assert!((score_i8 - score_f32).abs() < 0.02);
    }
}
