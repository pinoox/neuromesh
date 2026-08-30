use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingModelId {
    #[default]
    Gemma300mQ4,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub enabled: bool,
    pub model: EmbeddingModelId,
    pub matryoshka_dim: usize,
    pub ann_top_k: usize,
    pub min_cosine: f32,
    pub index_on_build: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: EmbeddingModelId::Gemma300mQ4,
            matryoshka_dim: 256,
            ann_top_k: 16,
            min_cosine: 0.45,
            index_on_build: true,
        }
    }
}

impl EmbeddingConfig {
    pub fn effective_enabled(&self) -> bool {
        self.enabled
    }
}
