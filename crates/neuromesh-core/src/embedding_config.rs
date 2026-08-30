use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingModelId {
    Gemma300mQ4,
    #[default]
    MiniLmMultilingualQ,
}

impl EmbeddingModelId {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "gemma300m_q4" | "gemma300m-q4" | "embeddinggemma300m_q4" | "gemma" => {
                Some(Self::Gemma300mQ4)
            }
            "minilm_multilingual_q" | "minilm-multilingual-q" | "minilm" => {
                Some(Self::MiniLmMultilingualQ)
            }
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gemma300mQ4 => "gemma300m_q4",
            Self::MiniLmMultilingualQ => "minilm_multilingual_q",
        }
    }

    pub fn default_matryoshka_dim(self) -> usize {
        match self {
            Self::Gemma300mQ4 => 256,
            Self::MiniLmMultilingualQ => 384,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub enabled: bool,
    pub model: EmbeddingModelId,
    pub matryoshka_dim: usize,
    pub ann_top_k: usize,
    /// Max embedding seeds inserted after ANN (pool remains `ann_top_k`).
    pub embed_seed_cap: usize,
    pub min_cosine: f32,
    pub index_on_build: bool,
    /// ONNX Runtime intra-op threads (`None` = all cores). Default 2 for lower ORT RAM.
    pub intra_threads: Option<usize>,
    /// MCP semantic prompt LRU cache (near-duplicate prompts).
    pub semantic_cache_enabled: bool,
    pub semantic_cache_entries: usize,
    pub semantic_cache_min_cosine: f32,
    /// Drop optional files with cosine >= threshold to a kept file (`None` = off).
    pub optional_dedup_min_cosine: Option<f32>,
    /// Index-time directory centroids in sidecar.
    pub module_cluster_enabled: bool,
    /// Refine rule-based General intent via embedding prototypes (opt-in).
    pub embed_intent_for_general: bool,
    /// Lower cosine floor for L3 recovery pass only (primary seeds use `min_cosine`).
    pub recovery_min_cosine: f32,
    /// Lexical/graph coarse pool before ANN (recall-safe two-stage).
    pub two_stage_enabled: bool,
    /// Max coarse candidates before fine ANN (clamped 200–500).
    pub coarse_pool_max: usize,
    /// File-first sidecar (v6): cold index embeds files; symbols lazy at query.
    pub hierarchical_index: bool,
    /// Top file hits before symbol subset ANN.
    pub file_ann_top_k: usize,
    /// Cosine floor for tier-0 file ANN.
    pub file_min_cosine: f32,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        let model = EmbeddingModelId::MiniLmMultilingualQ;
        Self {
            enabled: true,
            model,
            matryoshka_dim: model.default_matryoshka_dim(),
            ann_top_k: 16,
            embed_seed_cap: 4,
            min_cosine: 0.45,
            index_on_build: false,
            intra_threads: Some(2),
            semantic_cache_enabled: true,
            semantic_cache_entries: 16,
            semantic_cache_min_cosine: 0.96,
            optional_dedup_min_cosine: None,
            module_cluster_enabled: false,
            embed_intent_for_general: false,
            recovery_min_cosine: 0.38,
            two_stage_enabled: true,
            coarse_pool_max: 400,
            hierarchical_index: false,
            file_ann_top_k: 4,
            file_min_cosine: 0.35,
        }
    }
}

impl EmbeddingConfig {
    pub fn effective_enabled(&self) -> bool {
        self.enabled
    }

    /// Runtime embed tier active: sidecar loaded with hierarchical file-first index.
    pub fn embed_runtime_active(&self, sidecar_loaded: bool) -> bool {
        sidecar_loaded && self.hierarchical_index
    }

    pub fn normalized(mut self) -> Self {
        if self.matryoshka_dim == 0 {
            self.matryoshka_dim = self.model.default_matryoshka_dim();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_runtime_active_requires_sidecar_and_hierarchical() {
        let config = EmbeddingConfig {
            hierarchical_index: true,
            ..Default::default()
        };
        assert!(!config.embed_runtime_active(false));
        assert!(config.embed_runtime_active(true));
        let flat = EmbeddingConfig {
            hierarchical_index: false,
            ..Default::default()
        };
        assert!(!flat.embed_runtime_active(true));
    }
}
