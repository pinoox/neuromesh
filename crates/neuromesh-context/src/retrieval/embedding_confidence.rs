//! Embedding-based confidence checks for tier escalation and coverage honesty.

use neuromesh_core::{EmbeddingConfig, NodeId};
use neuromesh_graph::NeuralProjectGraph;

#[cfg(feature = "embeddings")]
use neuromesh_embed::{cached_query_vector, cosine_similarity, embed_query_cached};

pub const TIER_EMBEDDING_PRIMARY: &str = "embedding_primary";
pub const TIER_L1_EXACT: &str = "L1_exact";
pub const TIER_L2_PATTERN: &str = "L2_pattern";
pub const TIER_L3_SEMANTIC: &str = "L3_semantic_recovery";

fn query_vector(embedding_config: &EmbeddingConfig, prompt: &str) -> Option<Vec<f32>> {
    #[cfg(feature = "embeddings")]
    {
        if let Some(v) = cached_query_vector(embedding_config, prompt) {
            return Some(v);
        }
        embed_query_cached(embedding_config, prompt).ok()
    }
    #[cfg(not(feature = "embeddings"))]
    {
        let _ = (embedding_config, prompt);
        None
    }
}

/// Max cosine between `prompt` and resolved seed node vectors (0 when unavailable).
#[cfg(feature = "embeddings")]
pub fn max_seed_embedding_score(
    graph: &NeuralProjectGraph,
    prompt: &str,
    embedding_config: &EmbeddingConfig,
    seed_ids: &[NodeId],
) -> Option<f32> {
    if !embeddings_active_for_confidence(embedding_config, graph) || seed_ids.is_empty() {
        return None;
    }
    let index = graph.embedding_index();
    if !index.is_loaded() {
        return None;
    }
    let query = query_vector(embedding_config, prompt)?;
    let mut best = 0.0f32;
    for id in seed_ids {
        let idx = index.node_ids.iter().position(|n| n == id)?;
        let start = idx * index.dim;
        let end = start + index.dim;
        if end > index.vectors.len() {
            continue;
        }
        let score = cosine_similarity(&query, &index.vectors[start..end]);
        best = best.max(score);
    }
    Some(best)
}

#[cfg(not(feature = "embeddings"))]
pub fn max_seed_embedding_score(
    _graph: &NeuralProjectGraph,
    _prompt: &str,
    _embedding_config: &EmbeddingConfig,
    _seed_ids: &[NodeId],
) -> Option<f32> {
    None
}

/// Embeddings usable for cosine confidence (hybrid enabled or fast L3 sidecar loaded).
pub fn embeddings_active_for_confidence(
    embedding_config: &EmbeddingConfig,
    graph: &NeuralProjectGraph,
) -> bool {
    embedding_config.enabled || graph.embedding_index().is_loaded()
}

/// True when embeddings are on but resolved seeds align poorly with the prompt.
pub fn low_embedding_confidence(
    graph: &NeuralProjectGraph,
    prompt: &str,
    embedding_config: &EmbeddingConfig,
    seed_ids: &[NodeId],
) -> bool {
    if !embeddings_active_for_confidence(embedding_config, graph) || seed_ids.is_empty() {
        return false;
    }
    max_seed_embedding_score(graph, prompt, embedding_config, seed_ids)
        .is_some_and(|s| s < embedding_config.min_cosine)
}

/// Parse `semantic_embed:0.523` style seed reason.
pub fn parse_embedding_score(reason: &str) -> Option<f32> {
    reason
        .strip_prefix("semantic_embed:")
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.parse().ok())
}

pub fn is_embedding_reason(reason: &str) -> bool {
    reason.starts_with("semantic_embed:")
}

pub fn is_lexical_reason(reason: &str) -> bool {
    reason.starts_with("keyword:")
        || reason.starts_with("expansion:")
        || reason.starts_with("identifier:")
        || reason.starts_with("path_hint:")
        || reason.starts_with("anchor:")
        || reason.starts_with("concept:")
        || reason.contains(":client_keyword")
        || reason.contains(":identifier")
}

/// When embedding engine finds only weak matches, prefer honest `no_confident_match`.
pub fn confidence_coverage_override(
    seeds: &[neuromesh_core::SeedResolution],
    reasons: &std::collections::HashMap<neuromesh_core::NodeId, String>,
    embedding_config: &EmbeddingConfig,
    engine: neuromesh_core::SeedEngineId,
) -> Option<&'static str> {
    if !embedding_config.enabled || engine != neuromesh_core::SeedEngineId::SemanticLite {
        return None;
    }
    let resolved: Vec<_> = seeds.iter().filter(|s| s.resolved_id.is_some()).collect();
    if resolved.is_empty() {
        return None;
    }
    let has_lexical = resolved.iter().any(|s| {
        s.resolved_id
            .as_ref()
            .and_then(|id| reasons.get(id))
            .is_some_and(|r| is_lexical_reason(r) && s.confidence >= 0.5)
    });
    if has_lexical {
        return None;
    }
    let max_embed = resolved
        .iter()
        .filter_map(|s| s.embedding_score)
        .fold(0.0f32, f32::max);
    if max_embed > 0.0 && max_embed < embedding_config.min_cosine {
        return Some("no_confident_match");
    }
    if max_embed == 0.0 && resolved.iter().all(|s| is_embedding_reason(&s.query)) {
        return Some("no_confident_match");
    }
    None
}

pub fn dominant_resolution_tier(seeds: &[neuromesh_core::SeedResolution]) -> Option<String> {
    seeds
        .iter()
        .filter_map(|s| s.resolution_tier.clone())
        .next()
}

pub fn max_embedding_score_from_seeds(seeds: &[neuromesh_core::SeedResolution]) -> Option<f32> {
    seeds
        .iter()
        .filter_map(|s| s.embedding_score)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;

    #[test]
    fn embeddings_active_for_confidence_when_sidecar_loaded() {
        let graph = NeuralProjectGraph::new(ProjectId::new("conf-test"));
        let config = EmbeddingConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!embeddings_active_for_confidence(&config, &graph));
        let index = neuromesh_graph::EmbeddingIndex {
            dim: 384,
            file_node_ids: vec![NodeId::new("f1")],
            ..Default::default()
        };
        graph.install_embedding_index(index);
        assert!(embeddings_active_for_confidence(&config, &graph));
    }
}
