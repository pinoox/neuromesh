//! Per-packet query embedding cache — one ONNX inference per prompt per request.

use crate::{Embedder, EmbedderError};
use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
use parking_lot::Mutex;

static PACKET_CACHE: Mutex<Option<PacketCacheState>> = Mutex::new(None);

#[derive(Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    prompt: String,
    model: EmbeddingModelId,
    dim: usize,
}

struct PacketCacheState {
    key: CacheKey,
    vector: Vec<f32>,
}

/// Start a new packet scope (call once per `get_context_packet` / tiered activation).
pub fn packet_cache_begin() {
    *PACKET_CACHE.lock() = None;
}

/// Clear packet scope after activation completes.
pub fn packet_cache_end() {
    *PACKET_CACHE.lock() = None;
}

/// Embed `prompt` once per packet scope; subsequent calls return the cached vector.
pub fn embed_query_cached(
    config: &EmbeddingConfig,
    prompt: &str,
) -> Result<Vec<f32>, EmbedderError> {
    if !config.enabled {
        return Err(EmbedderError::Embed("embeddings disabled".into()));
    }
    let key = CacheKey {
        prompt: prompt.to_string(),
        model: config.model,
        dim: config.matryoshka_dim,
    };
    {
        let guard = PACKET_CACHE.lock();
        if let Some(state) = guard.as_ref() {
            if state.key == key {
                return Ok(state.vector.clone());
            }
        }
    }
    let arc = Embedder::lazy_global(config.clone())?;
    let vector = {
        let mut embedder = arc.lock();
        embedder.embed_query(prompt)?
    };
    *PACKET_CACHE.lock() = Some(PacketCacheState {
        key,
        vector: vector.clone(),
    });
    Ok(vector)
}

pub fn cached_query_vector(config: &EmbeddingConfig, prompt: &str) -> Option<Vec<f32>> {
    let key = CacheKey {
        prompt: prompt.to_string(),
        model: config.model,
        dim: config.matryoshka_dim,
    };
    PACKET_CACHE
        .lock()
        .as_ref()
        .filter(|s| s.key == key)
        .map(|s| s.vector.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_clears_state() {
        packet_cache_begin();
        assert!(cached_query_vector(&EmbeddingConfig::default(), "x").is_none());
        packet_cache_end();
    }
}
