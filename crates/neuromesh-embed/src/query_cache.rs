//! Per-packet query embedding cache — one ONNX inference per prompt per request.

use crate::{Embedder, EmbedderError};
use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static PACKET_CACHE: Mutex<Option<PacketCacheState>> = Mutex::new(None);
static PACKET_CACHE_DEPTH: AtomicUsize = AtomicUsize::new(0);

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

/// Enter a packet scope (nested calls preserve the cached query vector).
pub fn packet_cache_begin() {
    let depth = PACKET_CACHE_DEPTH.fetch_add(1, Ordering::Relaxed);
    if depth == 0 {
        *PACKET_CACHE.lock() = None;
    }
}

/// Leave a packet scope; clear cache when the outermost scope ends.
pub fn packet_cache_end() {
    let depth = PACKET_CACHE_DEPTH.fetch_sub(1, Ordering::Relaxed);
    if depth == 1 {
        *PACKET_CACHE.lock() = None;
    }
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
