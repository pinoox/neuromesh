#[cfg(test)]
mod seed_engine_tests {
    use crate::activator::resolve_seed_query;
    use crate::seed::ranker::{signal_weight, SignalKind};
    use crate::seed::sink::SeedBuffers;
    use crate::seed::{resolve_engine_id, run_seed_resolution};
    use neuromesh_core::{
        EmbeddingConfig, ProjectId, SeedEngineId, SeedResolutionConfig, TaskSignature,
    };
    use neuromesh_graph::NeuralProjectGraph;
    use neuromesh_task::TaskSignatureExtractor;
    use std::collections::HashMap;

    fn empty_signature_with_keywords() -> TaskSignature {
        let mut sig = TaskSignatureExtractor::extract("auth login token");
        sig.client_keywords = vec!["LoginView".into()];
        sig.client_expansion = vec!["authenticate".into()];
        sig
    }

    #[test]
    fn off_engine_ignores_client_signals() {
        let config = SeedResolutionConfig {
            engine: SeedEngineId::Off,
            ..Default::default()
        };
        let mut sig = empty_signature_with_keywords();
        sig.engine_override = Some(SeedEngineId::Off);
        assert_eq!(resolve_engine_id(&sig, &config), SeedEngineId::Off);
    }

    #[test]
    fn keyword_outranks_expansion_weight() {
        let config = SeedResolutionConfig::default();
        assert!(
            signal_weight(&config, SignalKind::Keyword, 0)
                > signal_weight(&config, SignalKind::Expansion, 0)
        );
    }

    #[test]
    fn semantic_lite_engine_is_default() {
        let config = SeedResolutionConfig::default();
        let sig = TaskSignature::new("demo");
        assert_eq!(resolve_engine_id(&sig, &config), SeedEngineId::SemanticLite);
    }

    #[test]
    fn lexical_engines_require_auto_extract_when_enabled() {
        let mut config = SeedResolutionConfig {
            engine: SeedEngineId::KeywordsExpanded,
            auto_extract_keywords: true,
            ..Default::default()
        };
        assert!(config.effective_auto_extract());
        config.engine = SeedEngineId::SemanticLite;
        assert!(!config.effective_auto_extract());
    }

    #[test]
    fn lexical_fallback_runs_on_empty_graph_prompt() {
        let graph = NeuralProjectGraph::new(ProjectId::new("empty"));
        let sig = TaskSignatureExtractor::extract("FindUserAccount service");
        let config = SeedResolutionConfig {
            engine: SeedEngineId::Keywords,
            ..Default::default()
        };
        let mut resolutions = Vec::new();
        let mut energies = HashMap::new();
        let mut reasons = HashMap::new();
        let mut buffers = SeedBuffers {
            resolutions: &mut resolutions,
            energies: &mut energies,
            reasons: &mut reasons,
        };
        let _ = run_seed_resolution(
            &graph,
            &sig,
            &sig.raw_prompt,
            &config,
            &EmbeddingConfig::default(),
            &mut buffers,
            resolve_seed_query,
            false,
        );
        assert!(buffers.energies.len() <= config.max_resolved_seeds);
    }

    #[test]
    fn engine_override_wins_over_config() {
        let config = SeedResolutionConfig::default();
        let mut sig = TaskSignature::new("demo");
        sig.engine_override = Some(SeedEngineId::Hybrid);
        assert_eq!(resolve_engine_id(&sig, &config), SeedEngineId::Hybrid);
    }

    #[test]
    fn embedding_config_defaults_minilm_enabled() {
        let cfg = EmbeddingConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.model.as_str(), "minilm_multilingual_q");
        assert_eq!(cfg.matryoshka_dim, 384);
        assert_eq!(cfg.intra_threads, Some(4));
        assert_eq!(cfg.embed_seed_cap, 4);
    }

    #[test]
    fn seed_weights_include_semantic_embed() {
        let cfg = SeedResolutionConfig::default();
        assert!(cfg.weights.semantic_embed_match > 0.0);
    }
}
