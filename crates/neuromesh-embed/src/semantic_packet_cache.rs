//! Cross-request semantic LRU for near-duplicate MCP prompts.

use crate::search::cosine_similarity;
use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCacheKey {
    pub graph_generation: u64,
    pub graph_digest: String,
    pub model: EmbeddingModelId,
    pub dim: usize,
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCachePayload {
    pub response: Value,
    pub details: Value,
    pub detail_tag: String,
}

struct Entry {
    key: SemanticCacheKey,
    prompt_vec: Vec<f32>,
    payload: SemanticCachePayload,
    #[allow(dead_code)]
    created_at: Instant,
}

struct Inner {
    project_id: Option<String>,
    order: VecDeque<usize>,
    entries: HashMap<usize, Entry>,
    next_id: usize,
}

impl Inner {
    fn clear_if_project_changed(&mut self, project_id: &str) {
        if self.project_id.as_deref().is_some_and(|p| p != project_id) {
            self.order.clear();
            self.entries.clear();
        }
        self.project_id = Some(project_id.to_string());
    }
}

pub struct SemanticPacketCache {
    #[allow(dead_code)]
    max_entries: usize,
    inner: Mutex<Inner>,
    hits: Mutex<u64>,
}

impl Default for SemanticPacketCache {
    fn default() -> Self {
        Self::new(16)
    }
}

impl SemanticPacketCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            inner: Mutex::new(Inner {
                project_id: None,
                order: VecDeque::new(),
                entries: HashMap::new(),
                next_id: 0,
            }),
            hits: Mutex::new(0),
        }
    }

    pub fn hit_count(&self) -> u64 {
        *self.hits.lock()
    }

    pub fn entry_count(&self) -> usize {
        self.inner.lock().entries.len()
    }

    pub fn lookup(
        &self,
        key: &SemanticCacheKey,
        query_vec: &[f32],
        min_cosine: f32,
    ) -> Option<SemanticCachePayload> {
        let mut inner = self.inner.lock();
        inner.clear_if_project_changed(&key.project_id);
        let mut best: Option<(usize, f32)> = None;
        for &slot in &inner.order {
            let Some(entry) = inner.entries.get(&slot) else {
                continue;
            };
            if entry.key != *key {
                continue;
            }
            let score = cosine_similarity(&entry.prompt_vec, query_vec);
            if score >= min_cosine {
                if best.map(|(_, s)| score > s).unwrap_or(true) {
                    best = Some((slot, score));
                }
            }
        }
        let (slot, _) = best?;
        let entry = inner.entries.get(&slot)?.payload.clone();
        *self.hits.lock() += 1;
        Some(entry)
    }

    pub fn insert(
        &self,
        config: &EmbeddingConfig,
        key: &SemanticCacheKey,
        prompt_vec: Vec<f32>,
        payload: SemanticCachePayload,
    ) {
        if !config.semantic_cache_enabled {
            return;
        }
        let cap = config.semantic_cache_entries.max(1);
        let mut inner = self.inner.lock();
        inner.clear_if_project_changed(&key.project_id);
        let id = inner.next_id;
        inner.next_id += 1;
        inner.entries.insert(
            id,
            Entry {
                key: key.clone(),
                prompt_vec,
                payload,
                created_at: Instant::now(),
            },
        );
        inner.order.push_back(id);
        while inner.order.len() > cap {
            if let Some(old) = inner.order.pop_front() {
                inner.entries.remove(&old);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::EmbeddingModelId;

    fn key(gen: u64) -> SemanticCacheKey {
        SemanticCacheKey {
            graph_generation: gen,
            graph_digest: "abc".into(),
            model: EmbeddingModelId::MiniLmMultilingualQ,
            dim: 4,
            project_id: "p1".into(),
        }
    }

    fn payload() -> SemanticCachePayload {
        SemanticCachePayload {
            response: serde_json::json!({"packet_id": "old"}),
            details: serde_json::json!({"packet_id": "old"}),
            detail_tag: "minimal".into(),
        }
    }

    #[test]
    fn same_vector_hits() {
        let cache = SemanticPacketCache::new(4);
        let cfg = EmbeddingConfig {
            semantic_cache_entries: 4,
            semantic_cache_min_cosine: 0.96,
            ..EmbeddingConfig::default()
        };
        let vec = vec![1.0, 0.0, 0.0, 0.0];
        cache.insert(&cfg, &key(1), vec.clone(), payload());
        let hit = cache.lookup(&key(1), &vec, 0.96);
        assert!(hit.is_some());
        assert_eq!(cache.hit_count(), 1);
    }

    #[test]
    fn generation_change_misses() {
        let cache = SemanticPacketCache::new(4);
        let cfg = EmbeddingConfig::default();
        let vec = vec![0.0, 1.0, 0.0, 0.0];
        cache.insert(&cfg, &key(1), vec.clone(), payload());
        assert!(cache.lookup(&key(2), &vec, 0.96).is_none());
    }

    #[test]
    fn below_threshold_misses() {
        let cache = SemanticPacketCache::new(4);
        let cfg = EmbeddingConfig::default();
        cache.insert(&cfg, &key(1), vec![1.0, 0.0, 0.0, 0.0], payload());
        let orth = vec![0.0, 1.0, 0.0, 0.0];
        assert!(cache.lookup(&key(1), &orth, 0.96).is_none());
    }
}
