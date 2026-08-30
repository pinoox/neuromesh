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
    /// ONNX Runtime intra-op threads (`None` = all cores). Default 4 for laptop hybrid CPUs.
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
            index_on_build: true,
            intra_threads: Some(4),
            semantic_cache_enabled: true,
            semantic_cache_entries: 16,
            semantic_cache_min_cosine: 0.96,
            optional_dedup_min_cosine: Some(0.93),
            module_cluster_enabled: true,
            embed_intent_for_general: false,
        }
    }
}

impl EmbeddingConfig {
    pub fn effective_enabled(&self) -> bool {
        self.enabled
    }

    pub fn normalized(mut self) -> Self {
        if self.matryoshka_dim == 0 {
            self.matryoshka_dim = self.model.default_matryoshka_dim();
        }
        self
    }
}
