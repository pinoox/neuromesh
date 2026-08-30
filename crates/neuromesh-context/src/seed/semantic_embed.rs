//! L3 vector recovery via precomputed symbol embeddings.

use crate::retrieval::embedding_confidence::TIER_EMBEDDING_PRIMARY;
use crate::seed::ranker::{signal_weight, SignalKind};
use crate::seed::sink::SeedSink;
use neuromesh_core::{EmbeddingConfig, SeedResolutionConfig};
use neuromesh_embed::embed_query_cached;
use neuromesh_graph::NeuralProjectGraph;

pub fn push_embedding_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    embedding_config: &EmbeddingConfig,
    seed_config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) -> bool {
    if !embedding_config.enabled {
        return false;
    }
    let index = graph.embedding_index();
    if !index.is_loaded() {
        return false;
    }

    let query = match embed_query_cached(embedding_config, prompt) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("embedding query failed: {e}");
            return false;
        }
    };

    let hits = index.ann_search(
        &query,
        embedding_config.ann_top_k,
        embedding_config.min_cosine,
    );
    if hits.is_empty() {
        return false;
    }

    let weight = signal_weight(seed_config, SignalKind::SemanticEmbed, 0);
    for (idx, (node_id, score)) in hits.iter().enumerate() {
        let energy = weight * score;
        let decay = 1.0 / (1.0 + idx as f32 * 0.05);
        sink.insert(
            node_id.clone(),
            energy * decay,
            format!("semantic_embed:{score:.3}"),
            Some(TIER_EMBEDDING_PRIMARY),
            Some(*score),
        );
    }
    true
}
