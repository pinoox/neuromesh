mod sidecar;
mod sketch;

#[cfg(feature = "embeddings")]
mod build;
mod dot;
mod quantize;

#[cfg(feature = "embeddings")]
mod coarse;

pub use sidecar::{
    load_sidecar, save_sidecar, EmbeddingIndex, EmbeddingSidecar, ModuleCentroid,
    MIN_SIDECAR_VERSION, SIDECAR_VERSION,
};
pub use sketch::{node_type_label, symbol_sketch};

#[cfg(feature = "embeddings")]
pub use build::{
    graph_digest, maybe_rebuild_embeddings, rebuild_embeddings, rebuild_embeddings_for_workspace,
    refresh_embeddings_after_index,
};

#[cfg(feature = "embeddings")]
pub use coarse::coarse_candidate_indices;

pub use dot::{dot_f32_f32, dot_f32_i8};
pub use quantize::{pearson_correlation, spearman_correlation, DEFAULT_QUANT_SCALE};
