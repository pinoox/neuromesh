mod sidecar;
mod sketch;

#[cfg(feature = "embeddings")]
mod build;

pub use sidecar::{
    load_sidecar, save_sidecar, EmbeddingIndex, EmbeddingSidecar, ModuleCentroid, SIDECAR_VERSION,
};
pub use sketch::{node_type_label, symbol_sketch};

#[cfg(feature = "embeddings")]
pub use build::{graph_digest, maybe_rebuild_embeddings, rebuild_embeddings};
