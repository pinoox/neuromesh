//! L3 vector recovery via precomputed symbol embeddings.

use crate::retrieval::embedding_confidence::TIER_EMBEDDING_PRIMARY;
use crate::seed::ranker::{signal_weight, SignalKind};
use crate::seed::sink::SeedSink;
use neuromesh_core::{EmbeddingConfig, SeedResolutionConfig, TaskSignature};
use neuromesh_embed::embed_query_cached;
use neuromesh_graph::coarse_candidate_indices;
use neuromesh_graph::NeuralProjectGraph;

pub fn push_embedding_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
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

    let min_cosine = signature
        .embed_min_cosine_override
        .unwrap_or(embedding_config.min_cosine);

    let hits = if embedding_config.hierarchical_index && index.is_hierarchical() {
        hierarchical_ann_hits(
            graph,
            &index,
            signature,
            prompt,
            embedding_config,
            &query,
            min_cosine,
        )
    } else {
        flat_ann_hits(
            &index,
            graph,
            signature,
            prompt,
            embedding_config,
            &query,
            min_cosine,
        )
    };

    if hits.is_empty() {
        return false;
    }

    let seed_cap = embedding_config.embed_seed_cap.max(1);
    let weight = signal_weight(seed_config, SignalKind::SemanticEmbed, 0);
    for (idx, (node_id, score)) in hits.iter().take(seed_cap).enumerate() {
        let energy = weight * score;
        let decay = 1.0 / (1.0 + idx as f32 * 0.05);
        let reason = if embedding_config.hierarchical_index && index.is_hierarchical() {
            format!("hierarchical:symbol_subset:{score:.3}")
        } else {
            format!("semantic_embed:{score:.3}")
        };
        sink.insert(
            node_id.clone(),
            energy * decay,
            reason,
            Some(TIER_EMBEDDING_PRIMARY),
            Some(*score),
        );
    }
    true
}

fn flat_ann_hits(
    index: &neuromesh_graph::EmbeddingIndex,
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    embedding_config: &EmbeddingConfig,
    query: &[f32],
    min_cosine: f32,
) -> Vec<(neuromesh_core::NodeId, f32)> {
    let n = index.node_ids.len();
    let pool = coarse_candidate_indices(
        graph,
        index,
        signature,
        prompt,
        embedding_config.coarse_pool_max,
    );
    let use_subset = embedding_config.two_stage_enabled && pool.len() >= 64 && pool.len() >= n / 20;

    let mut hits = if use_subset {
        index.ann_search_subset(query, &pool, embedding_config.ann_top_k, min_cosine)
    } else {
        index.ann_search(query, embedding_config.ann_top_k, min_cosine)
    };

    if hits.is_empty() && use_subset {
        hits = index.ann_search(query, embedding_config.ann_top_k, min_cosine);
    }
    hits
}

fn hierarchical_ann_hits(
    graph: &NeuralProjectGraph,
    index: &neuromesh_graph::EmbeddingIndex,
    signature: &TaskSignature,
    prompt: &str,
    embedding_config: &EmbeddingConfig,
    query: &[f32],
    min_cosine: f32,
) -> Vec<(neuromesh_core::NodeId, f32)> {
    let file_hits = index.file_ann_search(
        query,
        embedding_config.file_ann_top_k.max(1),
        embedding_config.file_min_cosine,
    );

    if let Some(workspace) = graph.workspace_root() {
        let file_ids: Vec<_> = file_hits.iter().map(|(id, _)| id.clone()).collect();
        if !file_ids.is_empty() {
            if let Err(e) = neuromesh_graph::lazy_embed_symbols_for_files(
                graph,
                &workspace,
                embedding_config,
                &file_ids,
            ) {
                tracing::warn!("lazy symbol embed failed: {e}");
            }
        }
    }

    // Reload index after lazy embed may have appended symbol tier.
    let index = graph.embedding_index();

    let file_ids: Vec<_> = file_hits.iter().map(|(id, _)| id.clone()).collect();
    let mut pool = index.symbol_indices_for_files(&file_ids);
    let coarse = coarse_candidate_indices(
        graph,
        &index,
        signature,
        prompt,
        embedding_config.coarse_pool_max,
    );
    for idx in coarse {
        if !pool.contains(&idx) {
            pool.push(idx);
        }
    }

    let mut hits = if pool.is_empty() {
        Vec::new()
    } else {
        index.ann_search_subset(query, &pool, embedding_config.ann_top_k, min_cosine)
    };

    if hits.is_empty() {
        hits = flat_ann_hits(
            &index,
            graph,
            signature,
            prompt,
            embedding_config,
            query,
            min_cosine,
        );
    }
    hits
}
