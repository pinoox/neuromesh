use crate::{EmbeddingConfig, OptimizationMode, SeedEngineId, SeedResolutionConfig};
use serde::{Deserialize, Serialize};

/// Unified retrieval preset (`fast` | `hybrid` | `deep`).
///
/// Maps to internal seed/embeddings/quality settings. Prefer this over scattered flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalEngine {
    /// Zero-embed: graph + query-side lexical expansion only.
    #[default]
    Fast,
    /// Incremental MiniLM sidecar (Phase A defaults) + graph traversal.
    Hybrid,
    /// Full quality: embed + dedup + module centroids + max_quality activation.
    Deep,
}

impl RetrievalEngine {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "fast" | "zero_embed" | "zero-embed" | "lexical" => Some(Self::Fast),
            "hybrid" | "semantic" | "embedded" | "embed" => Some(Self::Hybrid),
            "deep" | "max_quality" | "max-quality" | "quality" => Some(Self::Deep),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Hybrid => "hybrid",
            Self::Deep => "deep",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Fast, Self::Hybrid, Self::Deep]
    }

    pub fn help_line() -> &'static str {
        "fast | hybrid | deep"
    }

    /// Apply preset to the legacy config surfaces (seed, embeddings, optimization mode).
    pub fn apply_preset(
        self,
        mode: &mut OptimizationMode,
        seed: &mut SeedResolutionConfig,
        emb: &mut EmbeddingConfig,
    ) {
        match self {
            Self::Fast => {
                emb.enabled = false;
                emb.index_on_build = false;
                emb.two_stage_enabled = false;
                emb.optional_dedup_min_cosine = None;
                emb.module_cluster_enabled = false;
                seed.engine = SeedEngineId::KeywordsExpanded;
                seed.auto_extract_keywords = true;
                *mode = OptimizationMode::Balanced;
            }
            Self::Hybrid => {
                emb.enabled = true;
                emb.index_on_build = false;
                emb.two_stage_enabled = true;
                emb.coarse_pool_max = 400;
                emb.optional_dedup_min_cosine = None;
                emb.module_cluster_enabled = false;
                if emb.intra_threads.is_none() {
                    emb.intra_threads = Some(2);
                }
                seed.engine = SeedEngineId::SemanticLite;
                seed.auto_extract_keywords = false;
                *mode = OptimizationMode::Balanced;
            }
            Self::Deep => {
                emb.enabled = true;
                emb.index_on_build = false;
                emb.two_stage_enabled = true;
                emb.coarse_pool_max = 400;
                emb.optional_dedup_min_cosine = Some(0.93);
                emb.module_cluster_enabled = true;
                if emb.intra_threads.is_none() {
                    emb.intra_threads = Some(2);
                }
                seed.engine = SeedEngineId::SemanticLite;
                seed.auto_extract_keywords = false;
                *mode = OptimizationMode::MaxQuality;
            }
        }
        *emb = emb.clone().normalized();
    }

    /// L3 recovery seed engine when embeddings are unavailable or in fast mode.
    pub fn l3_seed_engine(
        self,
        configured: SeedEngineId,
        embeddings_enabled: bool,
    ) -> SeedEngineId {
        match self {
            Self::Fast => SeedEngineId::KeywordsExpanded,
            Self::Hybrid | Self::Deep if embeddings_enabled => SeedEngineId::SemanticLite,
            Self::Hybrid | Self::Deep => configured,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RetrievalConfig {
    pub engine: RetrievalEngine,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_preset_disables_embeddings() {
        let mut mode = OptimizationMode::MaxQuality;
        let mut seed = SeedResolutionConfig::default();
        let mut emb = EmbeddingConfig::default();
        RetrievalEngine::Fast.apply_preset(&mut mode, &mut seed, &mut emb);
        assert!(!emb.enabled);
        assert_eq!(seed.engine, SeedEngineId::KeywordsExpanded);
        assert!(seed.auto_extract_keywords);
        assert_eq!(mode, OptimizationMode::Balanced);
    }

    #[test]
    fn hybrid_preset_enables_phase_a_defaults() {
        let mut mode = OptimizationMode::MaxSavings;
        let mut seed = SeedResolutionConfig::default();
        let mut emb = EmbeddingConfig::default();
        RetrievalEngine::Hybrid.apply_preset(&mut mode, &mut seed, &mut emb);
        assert!(emb.enabled);
        assert!(emb.two_stage_enabled);
        assert_eq!(seed.engine, SeedEngineId::SemanticLite);
        assert_eq!(mode, OptimizationMode::Balanced);
    }

    #[test]
    fn deep_preset_max_quality_and_dedup() {
        let mut mode = OptimizationMode::Balanced;
        let mut seed = SeedResolutionConfig::default();
        let mut emb = EmbeddingConfig::default();
        RetrievalEngine::Deep.apply_preset(&mut mode, &mut seed, &mut emb);
        assert_eq!(mode, OptimizationMode::MaxQuality);
        assert_eq!(emb.optional_dedup_min_cosine, Some(0.93));
        assert!(emb.module_cluster_enabled);
    }

    #[test]
    fn parses_engine_aliases() {
        assert_eq!(RetrievalEngine::parse("fast"), Some(RetrievalEngine::Fast));
        assert_eq!(
            RetrievalEngine::parse("semantic"),
            Some(RetrievalEngine::Hybrid)
        );
        assert_eq!(
            RetrievalEngine::parse("max-quality"),
            Some(RetrievalEngine::Deep)
        );
    }
}
