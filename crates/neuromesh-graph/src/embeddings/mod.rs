mod sidecar;
mod sketch;

#[cfg(feature = "embeddings")]
mod build;
mod dot;
mod quantize;

#[cfg(feature = "embeddings")]
mod coarse;
#[cfg(feature = "embeddings")]
mod file_passage;
#[cfg(feature = "embeddings")]
mod file_rerank;
#[cfg(feature = "embeddings")]
mod lazy_symbols;
#[cfg(feature = "embeddings")]
mod sidecar_lock;

pub use sidecar::{
    load_sidecar, save_sidecar, save_sidecar_atomic, EmbeddingIndex, EmbeddingSidecar,
    ModuleCentroid, MIN_SIDECAR_VERSION, SIDECAR_VERSION,
};
pub use sketch::{node_type_label, symbol_sketch};

#[cfg(feature = "embeddings")]
pub use build::{
    ensure_file_tier_sidecar, graph_digest, maybe_rebuild_embeddings, passage_hash,
    rebuild_embeddings, rebuild_embeddings_for_workspace, refresh_embeddings_after_index,
    symbol_passage_for_node,
};

#[cfg(feature = "embeddings")]
pub use coarse::coarse_candidate_indices;

#[cfg(feature = "embeddings")]
pub use file_rerank::{concept_stem_patterns, rerank_file_hits, stem_union_file_hits};

#[cfg(feature = "embeddings")]
pub use lazy_symbols::{lazy_embed_symbols_for_files, sidecar_tier_stats};

pub use dot::{dot_f32_f32, dot_f32_i8};
pub use quantize::{pearson_correlation, spearman_correlation, DEFAULT_QUANT_SCALE};
