mod embedder;
mod intent_prototypes;
mod query_cache;
mod search;
mod semantic_packet_cache;

pub use embedder::{
    format_document, format_document_for_model, format_document_gemma, format_document_minilm,
    format_query, format_query_for_model, format_query_gemma, format_query_minilm, Embedder,
    EmbedderError,
};
pub use intent_prototypes::{best_intent_match, warm_intent_prototypes, IntentPrototype};
pub use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
pub use query_cache::{
    cached_query_vector, embed_query_cached, packet_cache_begin, packet_cache_end,
};
pub use search::{ann_search, cosine_similarity, truncate_and_normalize};
pub use semantic_packet_cache::{SemanticCacheKey, SemanticCachePayload, SemanticPacketCache};
