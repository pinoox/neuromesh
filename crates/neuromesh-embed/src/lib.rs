mod bundled_model;
mod embedder;
mod intent_prototypes;
mod model_install;
mod query_cache;
mod search;
mod semantic_packet_cache;

pub use bundled_model::{
    bundled_minilm_available, bundled_model_search_paths, resolve_bundled_minilm_dir,
};
pub use model_install::{
    default_models_root, install_hint, install_hint_with_flag, install_model, is_model_installed,
    list_installed, parse_model_id, EmbedModelSpec, InstallOptions, ModelInstallError, CATALOG,
    MINILM_MULTILINGUAL_Q,
};

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
