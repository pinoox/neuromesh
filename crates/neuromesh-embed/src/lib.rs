mod embedder;
mod search;

pub use embedder::{format_document, format_query, Embedder, EmbedderError};
pub use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
pub use search::{ann_search, cosine_similarity, truncate_and_normalize};
