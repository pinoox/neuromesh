use crate::embeddings::dot::{dot_f32_f32, dot_f32_i8};
use crate::embeddings::quantize::{dequant_slice, DEFAULT_QUANT_SCALE};
use neuromesh_core::NodeId;
use serde::{Deserialize, Serialize};

pub const SIDECAR_VERSION: u32 = 5;
pub const MIN_SIDECAR_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCentroid {
    pub dir: String,
    pub vector: Vec<f32>,
    pub symbol_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingSidecar {
    pub version: u32,
    pub model_id: String,
    pub dim: usize,
    pub graph_generation: u64,
    pub graph_digest: String,
    pub node_ids: Vec<NodeId>,
    /// Legacy f32 flat matrix (v4); empty on v5 write.
    #[serde(default)]
    pub vectors: Vec<f32>,
    #[serde(default)]
    pub module_centroids: Vec<ModuleCentroid>,
    /// Per-symbol passage hash for incremental rebuild (v4+).
    #[serde(default)]
    pub content_hashes: Vec<String>,
    /// Quantized symbol vectors (v5+).
    #[serde(default)]
    pub vectors_i8: Vec<i8>,
    /// Per-symbol dequant scale (v5+); empty uses `quant_scale` for all rows.
    #[serde(default)]
    pub quant_scales: Vec<f32>,
    #[serde(default = "default_quant_scale")]
    pub quant_scale: f32,
}

fn default_quant_scale() -> f32 {
    DEFAULT_QUANT_SCALE
}

impl EmbeddingSidecar {
    pub fn is_compatible(&self, model_id: &str, dim: usize, generation: u64, digest: &str) -> bool {
        self.version >= MIN_SIDECAR_VERSION
            && self.version <= SIDECAR_VERSION
            && self.model_id == model_id
            && self.dim == dim
            && self.graph_generation == generation
            && self.graph_digest == digest
    }
}

#[derive(Debug, Clone, Default)]
pub struct EmbeddingIndex {
    pub model_id: String,
    pub dim: usize,
    pub graph_generation: u64,
    pub graph_digest: String,
    pub node_ids: Vec<NodeId>,
    pub vectors: Vec<f32>,
    pub module_centroids: Vec<ModuleCentroid>,
    pub content_hashes: Vec<String>,
    pub vectors_i8: Vec<i8>,
    pub quant_scales: Vec<f32>,
    pub quant_scale: f32,
}

impl EmbeddingIndex {
    pub fn from_sidecar(sidecar: EmbeddingSidecar) -> Self {
        Self {
            model_id: sidecar.model_id,
            dim: sidecar.dim,
            graph_generation: sidecar.graph_generation,
            graph_digest: sidecar.graph_digest,
            node_ids: sidecar.node_ids,
            vectors: if sidecar.vectors_i8.is_empty() {
                sidecar.vectors
            } else {
                Vec::new()
            },
            module_centroids: sidecar.module_centroids,
            content_hashes: sidecar.content_hashes,
            vectors_i8: sidecar.vectors_i8,
            quant_scales: sidecar.quant_scales,
            quant_scale: if sidecar.quant_scale > f32::EPSILON {
                sidecar.quant_scale
            } else {
                DEFAULT_QUANT_SCALE
            },
        }
    }

    pub fn is_loaded(&self) -> bool {
        !self.node_ids.is_empty() && self.dim > 0
    }

    pub fn uses_int8(&self) -> bool {
        !self.vectors_i8.is_empty()
    }

    fn row_scale(&self, idx: usize) -> f32 {
        self.quant_scales
            .get(idx)
            .copied()
            .filter(|s| *s > f32::EPSILON)
            .unwrap_or(self.quant_scale)
    }

    fn score_at(&self, query: &[f32], idx: usize) -> Option<f32> {
        let start = idx * self.dim;
        if query.len() != self.dim {
            return None;
        }
        if self.uses_int8() {
            let end = start + self.dim;
            if end > self.vectors_i8.len() {
                return None;
            }
            Some(dot_f32_i8(
                query,
                &self.vectors_i8[start..end],
                self.row_scale(idx),
            ))
        } else {
            let end = start + self.dim;
            if end > self.vectors.len() {
                return None;
            }
            Some(dot_f32_f32(query, &self.vectors[start..end]))
        }
    }

    fn collect_scored(
        &self,
        query: &[f32],
        indices: impl IntoIterator<Item = usize>,
        min_cosine: f32,
    ) -> Vec<(NodeId, f32)> {
        indices
            .into_iter()
            .filter_map(|idx| {
                let id = self.node_ids.get(idx)?.clone();
                let score = self.score_at(query, idx)?;
                if score >= min_cosine {
                    Some((id, score))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn ann_search(&self, query: &[f32], top_k: usize, min_cosine: f32) -> Vec<(NodeId, f32)> {
        if query.len() != self.dim {
            return Vec::new();
        }
        let mut scored = self.collect_scored(query, 0..self.node_ids.len(), min_cosine);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    pub fn ann_search_subset(
        &self,
        query: &[f32],
        candidate_indices: &[usize],
        top_k: usize,
        min_cosine: f32,
    ) -> Vec<(NodeId, f32)> {
        if query.len() != self.dim || candidate_indices.is_empty() {
            return Vec::new();
        }
        let mut scored = self.collect_scored(query, candidate_indices.iter().copied(), min_cosine);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Dequantize symbol vector at `idx` into `out` (for optional dedup / diagnostics).
    pub fn dequant_symbol(&self, idx: usize, out: &mut [f32]) -> bool {
        if out.len() != self.dim {
            return false;
        }
        let start = idx * self.dim;
        if self.uses_int8() {
            let end = start + self.dim;
            if end > self.vectors_i8.len() {
                return false;
            }
            dequant_slice(&self.vectors_i8[start..end], self.row_scale(idx), out);
            true
        } else {
            let end = start + self.dim;
            if end > self.vectors.len() {
                return false;
            }
            out.copy_from_slice(&self.vectors[start..end]);
            true
        }
    }
}

pub fn load_sidecar(path: &std::path::Path) -> neuromesh_core::Result<Option<EmbeddingSidecar>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(path)?;
    let sidecar = bincode::deserialize(&raw)
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    Ok(Some(sidecar))
}

pub fn save_sidecar(
    path: &std::path::Path,
    sidecar: &EmbeddingSidecar,
) -> neuromesh_core::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = bincode::serialize(sidecar)
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::quantize::quantize_unit_vector;

    #[test]
    fn sidecar_v5_round_trip() {
        let dim = 4;
        let v = vec![0.5f32, -0.3, 0.8, 0.1];
        let mut v_norm = v.clone();
        let norm: f32 = v_norm.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut v_norm {
            *x /= norm;
        }
        let mut qi8 = vec![0i8; dim];
        let row_scale = quantize_unit_vector(&v_norm, &mut qi8);

        let sidecar = EmbeddingSidecar {
            version: SIDECAR_VERSION,
            model_id: "minilm_multilingual_q".into(),
            dim,
            graph_generation: 7,
            graph_digest: "abc".into(),
            node_ids: vec![NodeId::new("a")],
            vectors: Vec::new(),
            module_centroids: Vec::new(),
            content_hashes: vec!["h1".into()],
            vectors_i8: qi8,
            quant_scales: vec![row_scale],
            quant_scale: DEFAULT_QUANT_SCALE,
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("embeddings.bin");
        save_sidecar(&path, &sidecar).expect("save");
        let loaded = load_sidecar(&path).expect("load").expect("some");
        assert_eq!(loaded.version, SIDECAR_VERSION);
        assert_eq!(loaded.vectors_i8, sidecar.vectors_i8);

        let index = EmbeddingIndex::from_sidecar(loaded);
        assert!(index.uses_int8());
        let hits = index.ann_search(&v_norm, 1, 0.0);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn ann_subset_matches_full_for_single_candidate() {
        let dim = 3;
        let index = EmbeddingIndex {
            node_ids: vec![NodeId::new("a"), NodeId::new("b")],
            vectors: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            dim,
            quant_scale: DEFAULT_QUANT_SCALE,
            ..Default::default()
        };
        let query = vec![1.0, 0.0, 0.0];
        let full = index.ann_search(&query, 1, 0.0);
        let sub = index.ann_search_subset(&query, &[0], 1, 0.0);
        assert_eq!(full, sub);
    }
}
