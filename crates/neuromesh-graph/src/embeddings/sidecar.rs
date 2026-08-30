use neuromesh_core::NodeId;
use serde::{Deserialize, Serialize};

pub const SIDECAR_VERSION: u32 = 4;

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
    pub vectors: Vec<f32>,
    #[serde(default)]
    pub module_centroids: Vec<ModuleCentroid>,
    /// Per-symbol passage hash for incremental rebuild (v4+).
    #[serde(default)]
    pub content_hashes: Vec<String>,
}

impl EmbeddingSidecar {
    pub fn is_compatible(&self, model_id: &str, dim: usize, generation: u64, digest: &str) -> bool {
        self.version == SIDECAR_VERSION
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
}

impl EmbeddingIndex {
    pub fn from_sidecar(sidecar: EmbeddingSidecar) -> Self {
        Self {
            model_id: sidecar.model_id,
            dim: sidecar.dim,
            graph_generation: sidecar.graph_generation,
            graph_digest: sidecar.graph_digest,
            node_ids: sidecar.node_ids,
            vectors: sidecar.vectors,
            module_centroids: sidecar.module_centroids,
            content_hashes: sidecar.content_hashes,
        }
    }

    pub fn is_loaded(&self) -> bool {
        !self.node_ids.is_empty() && self.dim > 0
    }

    pub fn ann_search(&self, query: &[f32], top_k: usize, min_cosine: f32) -> Vec<(NodeId, f32)> {
        if query.len() != self.dim {
            return Vec::new();
        }
        let mut scored: Vec<(NodeId, f32)> = self
            .node_ids
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| {
                let start = idx * self.dim;
                let end = start + self.dim;
                if end > self.vectors.len() {
                    return None;
                }
                let slice = &self.vectors[start..end];
                let score = slice
                    .iter()
                    .zip(query.iter())
                    .map(|(a, b)| a * b)
                    .sum::<f32>();
                if score >= min_cosine {
                    Some((id.clone(), score))
                } else {
                    None
                }
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
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

    #[test]
    fn sidecar_round_trip() {
        let sidecar = EmbeddingSidecar {
            version: SIDECAR_VERSION,
            model_id: "gemma300m_q4".into(),
            dim: 2,
            graph_generation: 7,
            graph_digest: "abc".into(),
            node_ids: vec![NodeId::new("a"), NodeId::new("b")],
            vectors: vec![1.0, 0.0, 0.0, 1.0],
            module_centroids: Vec::new(),
            content_hashes: vec![],
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("embeddings.bin");
        save_sidecar(&path, &sidecar).expect("save");
        let loaded = load_sidecar(&path).expect("load").expect("some");
        assert_eq!(loaded.model_id, sidecar.model_id);
        assert_eq!(loaded.vectors, sidecar.vectors);
    }
}
