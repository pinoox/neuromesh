use crate::embeddings::dot::{dot_f32_f32, dot_f32_i8};
use crate::embeddings::quantize::{dequant_slice, DEFAULT_QUANT_SCALE};
use neuromesh_core::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SIDECAR_VERSION: u32 = 6;
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
    /// Symbol tier node ids (tier-1).
    pub node_ids: Vec<NodeId>,
    /// Legacy f32 flat matrix (v4); empty on v5+ write.
    #[serde(default)]
    pub vectors: Vec<f32>,
    #[serde(default)]
    pub module_centroids: Vec<ModuleCentroid>,
    /// Per-symbol passage hash for incremental rebuild (v4+).
    #[serde(default)]
    pub content_hashes: Vec<String>,
    /// Quantized symbol vectors (v5+ tier-1).
    #[serde(default)]
    pub vectors_i8: Vec<i8>,
    /// Per-symbol dequant scale (v5+); empty uses `quant_scale` for all rows.
    #[serde(default)]
    pub quant_scales: Vec<f32>,
    #[serde(default = "default_quant_scale")]
    pub quant_scale: f32,
    /// Tier-0 file nodes (v6 hierarchical).
    #[serde(default)]
    pub file_node_ids: Vec<NodeId>,
    #[serde(default)]
    pub file_content_hashes: Vec<String>,
    #[serde(default)]
    pub file_vectors_i8: Vec<i8>,
    #[serde(default)]
    pub file_quant_scales: Vec<f32>,
    /// Maps symbol row index -> file row index (v6).
    #[serde(default)]
    pub symbol_file_index: Vec<u32>,
}

fn default_quant_scale() -> f32 {
    DEFAULT_QUANT_SCALE
}

impl EmbeddingSidecar {
    pub fn is_hierarchical(&self) -> bool {
        self.version >= 6 && !self.file_node_ids.is_empty()
    }

    pub fn is_compatible(&self, model_id: &str, dim: usize, generation: u64, digest: &str) -> bool {
        self.version >= MIN_SIDECAR_VERSION
            && self.version <= SIDECAR_VERSION
            && self.model_id == model_id
            && self.dim == dim
            && self.graph_generation == generation
            && self.graph_digest == digest
    }

    /// Hierarchical sidecars must be v6+ with a populated file tier.
    pub fn is_compatible_hierarchical(
        &self,
        model_id: &str,
        dim: usize,
        generation: u64,
        digest: &str,
    ) -> bool {
        self.is_compatible(model_id, dim, generation, digest) && self.is_hierarchical()
    }
}

#[derive(Debug, Clone, Default)]
pub struct EmbeddingIndex {
    pub model_id: String,
    pub dim: usize,
    pub graph_generation: u64,
    pub graph_digest: String,
    /// Symbol tier (tier-1).
    pub node_ids: Vec<NodeId>,
    pub vectors: Vec<f32>,
    pub module_centroids: Vec<ModuleCentroid>,
    pub content_hashes: Vec<String>,
    pub vectors_i8: Vec<i8>,
    pub quant_scales: Vec<f32>,
    pub quant_scale: f32,
    /// File tier (tier-0).
    pub file_node_ids: Vec<NodeId>,
    pub file_content_hashes: Vec<String>,
    pub file_vectors_i8: Vec<i8>,
    pub file_quant_scales: Vec<f32>,
    pub symbol_file_index: Vec<u32>,
    pub file_id_to_row: HashMap<NodeId, usize>,
    pub symbol_id_to_row: HashMap<NodeId, usize>,
}

impl EmbeddingIndex {
    pub fn from_sidecar(sidecar: EmbeddingSidecar) -> Self {
        let file_id_to_row = sidecar
            .file_node_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
        let symbol_id_to_row = sidecar
            .node_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
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
            file_node_ids: sidecar.file_node_ids,
            file_content_hashes: sidecar.file_content_hashes,
            file_vectors_i8: sidecar.file_vectors_i8,
            file_quant_scales: sidecar.file_quant_scales,
            symbol_file_index: sidecar.symbol_file_index,
            file_id_to_row,
            symbol_id_to_row,
        }
    }

    pub fn to_sidecar(&self) -> EmbeddingSidecar {
        EmbeddingSidecar {
            version: if self.is_hierarchical() {
                SIDECAR_VERSION
            } else if !self.vectors_i8.is_empty() {
                5
            } else {
                SIDECAR_VERSION
            },
            model_id: self.model_id.clone(),
            dim: self.dim,
            graph_generation: self.graph_generation,
            graph_digest: self.graph_digest.clone(),
            node_ids: self.node_ids.clone(),
            vectors: Vec::new(),
            module_centroids: self.module_centroids.clone(),
            content_hashes: self.content_hashes.clone(),
            vectors_i8: self.vectors_i8.clone(),
            quant_scales: self.quant_scales.clone(),
            quant_scale: self.quant_scale,
            file_node_ids: self.file_node_ids.clone(),
            file_content_hashes: self.file_content_hashes.clone(),
            file_vectors_i8: self.file_vectors_i8.clone(),
            file_quant_scales: self.file_quant_scales.clone(),
            symbol_file_index: self.symbol_file_index.clone(),
        }
    }

    pub fn is_hierarchical(&self) -> bool {
        !self.file_node_ids.is_empty()
    }

    pub fn is_loaded(&self) -> bool {
        self.dim > 0 && (!self.node_ids.is_empty() || !self.file_node_ids.is_empty())
    }

    pub fn uses_int8(&self) -> bool {
        !self.vectors_i8.is_empty() || !self.file_vectors_i8.is_empty()
    }

    pub fn file_count(&self) -> usize {
        self.file_node_ids.len()
    }

    pub fn symbol_count(&self) -> usize {
        self.node_ids.len()
    }

    pub fn is_symbol_embedded(&self, node_id: &NodeId) -> bool {
        self.symbol_id_to_row.contains_key(node_id)
    }

    pub fn file_row(&self, file_id: &NodeId) -> Option<usize> {
        self.file_id_to_row.get(file_id).copied()
    }

    pub fn symbol_row(&self, symbol_id: &NodeId) -> Option<usize> {
        self.symbol_id_to_row.get(symbol_id).copied()
    }

    /// Symbol tier row indices for symbols residing in any of `file_ids`.
    pub fn symbol_indices_for_files(&self, file_ids: &[NodeId]) -> Vec<usize> {
        use std::collections::HashSet;
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for file_id in file_ids {
            let Some(&file_row) = self.file_id_to_row.get(file_id) else {
                continue;
            };
            for (sym_row, &mapped) in self.symbol_file_index.iter().enumerate() {
                if mapped as usize == file_row && seen.insert(sym_row) {
                    out.push(sym_row);
                }
            }
        }
        out
    }

    fn file_row_scale(&self, idx: usize) -> f32 {
        self.file_quant_scales
            .get(idx)
            .copied()
            .filter(|s| *s > f32::EPSILON)
            .unwrap_or(self.quant_scale)
    }

    fn symbol_row_scale(&self, idx: usize) -> f32 {
        self.quant_scales
            .get(idx)
            .copied()
            .filter(|s| *s > f32::EPSILON)
            .unwrap_or(self.quant_scale)
    }

    fn score_file_at(&self, query: &[f32], idx: usize) -> Option<f32> {
        let start = idx * self.dim;
        if query.len() != self.dim {
            return None;
        }
        let end = start + self.dim;
        if end > self.file_vectors_i8.len() {
            return None;
        }
        Some(dot_f32_i8(
            query,
            &self.file_vectors_i8[start..end],
            self.file_row_scale(idx),
        ))
    }

    fn score_symbol_at(&self, query: &[f32], idx: usize) -> Option<f32> {
        if query.len() != self.dim {
            return None;
        }
        if self.uses_int8() && !self.vectors_i8.is_empty() {
            let start = idx * self.dim;
            let end = start + self.dim;
            if end > self.vectors_i8.len() {
                return None;
            }
            Some(dot_f32_i8(
                query,
                &self.vectors_i8[start..end],
                self.symbol_row_scale(idx),
            ))
        } else {
            let start = idx * self.dim;
            let end = start + self.dim;
            if end > self.vectors.len() {
                return None;
            }
            Some(dot_f32_f32(query, &self.vectors[start..end]))
        }
    }

    fn collect_symbol_scored(
        &self,
        query: &[f32],
        indices: impl IntoIterator<Item = usize>,
        min_cosine: f32,
    ) -> Vec<(NodeId, f32)> {
        indices
            .into_iter()
            .filter_map(|idx| {
                let id = self.node_ids.get(idx)?.clone();
                let score = self.score_symbol_at(query, idx)?;
                if score >= min_cosine {
                    Some((id, score))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn ann_search(&self, query: &[f32], top_k: usize, min_cosine: f32) -> Vec<(NodeId, f32)> {
        if query.len() != self.dim || self.node_ids.is_empty() {
            return Vec::new();
        }
        let mut scored = self.collect_symbol_scored(query, 0..self.node_ids.len(), min_cosine);
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
        let mut scored =
            self.collect_symbol_scored(query, candidate_indices.iter().copied(), min_cosine);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    pub fn file_ann_search(
        &self,
        query: &[f32],
        top_k: usize,
        min_cosine: f32,
    ) -> Vec<(NodeId, f32)> {
        if query.len() != self.dim || self.file_node_ids.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(NodeId, f32)> = (0..self.file_node_ids.len())
            .filter_map(|idx| {
                let id = self.file_node_ids.get(idx)?.clone();
                let score = self.score_file_at(query, idx)?;
                if score >= min_cosine {
                    Some((id, score))
                } else {
                    None
                }
            })
            .collect();
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
        if self.uses_int8() && !self.vectors_i8.is_empty() {
            let end = start + self.dim;
            if end > self.vectors_i8.len() {
                return false;
            }
            dequant_slice(
                &self.vectors_i8[start..end],
                self.symbol_row_scale(idx),
                out,
            );
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

    pub fn rebuild_lookup_maps(&mut self) {
        self.file_id_to_row = self
            .file_node_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
        self.symbol_id_to_row = self
            .node_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
    }
}

pub fn load_sidecar(path: &Path) -> neuromesh_core::Result<Option<EmbeddingSidecar>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(path)?;
    let sidecar = bincode::deserialize(&raw)
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    Ok(Some(sidecar))
}

/// Atomic replace: write temp file then rename (prevents torn reads on crash).
pub fn save_sidecar(path: &Path, sidecar: &EmbeddingSidecar) -> neuromesh_core::Result<()> {
    save_sidecar_atomic(path, sidecar)
}

pub fn save_sidecar_atomic(path: &Path, sidecar: &EmbeddingSidecar) -> neuromesh_core::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = bincode::serialize(sidecar)
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    let tmp: PathBuf = path.with_extension("bin.tmp");
    std::fs::write(&tmp, &bytes)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&tmp, path).map_err(neuromesh_core::NeuroMeshError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::quantize::quantize_unit_vector;

    #[test]
    fn sidecar_v6_hierarchical_round_trip() {
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
            node_ids: vec![NodeId::new("sym1")],
            vectors: Vec::new(),
            module_centroids: Vec::new(),
            content_hashes: vec!["h1".into()],
            vectors_i8: qi8.clone(),
            quant_scales: vec![row_scale],
            quant_scale: DEFAULT_QUANT_SCALE,
            file_node_ids: vec![NodeId::new("file1")],
            file_content_hashes: vec!["fh1".into()],
            file_vectors_i8: qi8,
            file_quant_scales: vec![row_scale],
            symbol_file_index: vec![0],
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("embeddings.bin");
        save_sidecar_atomic(&path, &sidecar).expect("save");
        let loaded = load_sidecar(&path).expect("load").expect("some");
        assert_eq!(loaded.version, SIDECAR_VERSION);
        assert!(loaded.is_hierarchical());

        let index = EmbeddingIndex::from_sidecar(loaded);
        assert!(index.is_hierarchical());
        assert_eq!(index.file_count(), 1);
        assert_eq!(index.symbol_count(), 1);
        let hits = index.file_ann_search(&v_norm, 1, 0.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            index.symbol_indices_for_files(&[NodeId::new("file1")]),
            vec![0]
        );
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
