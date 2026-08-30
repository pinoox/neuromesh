pub const DEFAULT_QUANT_SCALE: f32 = 127.0;

/// Quantize a unit-normalized f32 vector; returns per-row max-abs scale.
pub fn quantize_unit_vector(v: &[f32], out: &mut [i8]) -> f32 {
    debug_assert_eq!(v.len(), out.len());
    let max_abs = v.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let scale = max_abs.max(f32::EPSILON);
    for (o, x) in out.iter_mut().zip(v) {
        *o = (x / scale * 127.0).clamp(-127.0, 127.0).round() as i8;
    }
    scale
}

/// Quantize flat matrix; returns (i8 bytes, per-row scales).
pub fn quantize_matrix(vectors_f32: &[f32], dim: usize) -> (Vec<i8>, Vec<f32>) {
    let n = vectors_f32.len() / dim.max(1);
    let mut out = vec![0i8; n * dim];
    let mut scales = Vec::with_capacity(n);
    for row in 0..n {
        let start = row * dim;
        let scale = quantize_unit_vector(
            &vectors_f32[start..start + dim],
            &mut out[start..start + dim],
        );
        scales.push(scale);
    }
    (out, scales)
}

/// Dequantize int8 slice into f32 buffer (`scale` = per-row max abs).
pub fn dequant_slice(i8_slice: &[i8], scale: f32, out: &mut [f32]) {
    debug_assert_eq!(i8_slice.len(), out.len());
    if scale <= f32::EPSILON {
        out.fill(0.0);
        return;
    }
    const NORM: f32 = 127.0;
    for (o, v) in out.iter_mut().zip(i8_slice) {
        *o = *v as f32 * scale / NORM;
    }
}

/// Pearson correlation between two score vectors.
pub fn pearson_correlation(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.len() < 2 {
        return 1.0;
    }
    let n = a.len() as f32;
    let mean_a = a.iter().sum::<f32>() / n;
    let mean_b = b.iter().sum::<f32>() / n;
    let mut num = 0.0f32;
    let mut den_a = 0.0f32;
    let mut den_b = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        let da = x - mean_a;
        let db = y - mean_b;
        num += da * db;
        den_a += da * da;
        den_b += db * db;
    }
    let den = (den_a * den_b).sqrt();
    if den <= f32::EPSILON {
        1.0
    } else {
        num / den
    }
}

/// Spearman rank correlation (Pearson on ranks).
pub fn spearman_correlation(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let rank = |scores: &[f32]| -> Vec<f32> {
        let mut order: Vec<usize> = (0..scores.len()).collect();
        order.sort_by(|i, j| {
            scores[*j]
                .partial_cmp(&scores[*i])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut ranks = vec![0.0f32; scores.len()];
        for (rank_pos, &idx) in order.iter().enumerate() {
            ranks[idx] = rank_pos as f32;
        }
        ranks
    };
    let ra = rank(a);
    let rb = rank(b);
    pearson_correlation(&ra, &rb)
}

#[cfg(test)]
mod rank_tests {
    use super::*;
    use crate::embeddings::dot::{dot_f32_f32, dot_f32_i8};

    fn random_unit_vector(dim: usize, seed: u64) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        let mut s = seed;
        for x in &mut v {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            *x = ((s >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for x in &mut v {
                *x /= norm;
            }
        }
        v
    }

    fn normalize(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for x in v {
                *x /= norm;
            }
        }
    }

    #[test]
    fn int8_rank_correlation_meets_gate() {
        const DIM: usize = 384;
        const N_VECS: usize = 500;
        const N_QUERIES: usize = 20;
        let mut matrix_f32 = Vec::with_capacity(N_VECS * DIM);
        let mut matrix_i8 = Vec::with_capacity(N_VECS * DIM);
        let mut matrix_scales = Vec::with_capacity(N_VECS);
        for i in 0..N_VECS {
            let v = random_unit_vector(DIM, i as u64 + 42);
            matrix_f32.extend_from_slice(&v);
            let start = i * DIM;
            matrix_i8.resize(start + DIM, 0);
            let scale = quantize_unit_vector(&v, &mut matrix_i8[start..start + DIM]);
            matrix_scales.push(scale);
        }

        let mut min_spearman = 1.0f32;
        for q in 0..N_QUERIES {
            let mut query = random_unit_vector(DIM, q as u64 + 9000);
            normalize(&mut query);

            let mut scores_f32 = Vec::with_capacity(N_VECS);
            let mut scores_i8 = Vec::with_capacity(N_VECS);
            for (i, &scale) in matrix_scales.iter().enumerate() {
                let start = i * DIM;
                scores_f32.push(dot_f32_f32(&query, &matrix_f32[start..start + DIM]));
                scores_i8.push(dot_f32_i8(
                    &query,
                    &matrix_i8[start..start + DIM],
                    scale,
                ));
            }

            let spearman = spearman_correlation(&scores_f32, &scores_i8);
            min_spearman = min_spearman.min(spearman);
            assert!(
                spearman >= 0.99,
                "query {q}: spearman {spearman} below 0.99"
            );
        }
        assert!(min_spearman >= 0.99, "min spearman {min_spearman}");
    }

    #[test]
    fn int8_top16_overlap_meets_gate() {
        const DIM: usize = 384;
        const N_VECS: usize = 500;
        const TOP_K: usize = 16;
        let mut matrix_f32 = Vec::with_capacity(N_VECS * DIM);
        let mut matrix_i8 = Vec::with_capacity(N_VECS * DIM);
        let mut matrix_scales = Vec::with_capacity(N_VECS);
        for i in 0..N_VECS {
            let v = random_unit_vector(DIM, i as u64 + 7);
            matrix_f32.extend_from_slice(&v);
            let start = i * DIM;
            matrix_i8.resize(start + DIM, 0);
            let scale = quantize_unit_vector(&v, &mut matrix_i8[start..start + DIM]);
            matrix_scales.push(scale);
        }

        let query = random_unit_vector(DIM, 12345);
        let mut scored_f32: Vec<(usize, f32)> = (0..N_VECS)
            .map(|i| {
                let start = i * DIM;
                (i, dot_f32_f32(&query, &matrix_f32[start..start + DIM]))
            })
            .collect();
        scored_f32.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_f32: std::collections::HashSet<usize> =
            scored_f32.iter().take(TOP_K).map(|(i, _)| *i).collect();

        let scored_i8: Vec<(usize, f32)> = matrix_scales
            .iter()
            .enumerate()
            .map(|(i, &scale)| {
                let start = i * DIM;
                (
                    i,
                    dot_f32_i8(&query, &matrix_i8[start..start + DIM], scale),
                )
            })
            .collect();
        scored_i8.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_i8: std::collections::HashSet<usize> =
            scored_i8.iter().take(TOP_K).map(|(i, _)| *i).collect();

        let overlap = top_f32.intersection(&top_i8).count();
        assert!(
            overlap >= TOP_K - 1,
            "top-{TOP_K} overlap {overlap}/{TOP_K} below gate"
        );
    }
}
