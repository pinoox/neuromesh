//! MCP semantic prompt LRU — near-duplicate prompts skip full activation.

use crate::packet_cache::PacketDetails;
use crate::response::ResponseDetail;
use neuromesh_embed::cosine_similarity;
use neuromesh_embed::SemanticCacheKey;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

pub struct SemanticCacheHit {
    pub response: Value,
    pub details: PacketDetails,
}

struct Entry {
    key: SemanticCacheKey,
    prompt_vec: Vec<f32>,
    response: Value,
    details: PacketDetails,
    #[allow(dead_code)]
    detail: ResponseDetail,
    #[allow(dead_code)]
    created_at: Instant,
}

struct Inner {
    project_id: Option<String>,
    order: VecDeque<usize>,
    entries: HashMap<usize, Entry>,
    next_id: usize,
}

pub struct McpSemanticCache {
    max_entries: usize,
    inner: Mutex<Inner>,
    hits: Mutex<u64>,
}

impl Default for McpSemanticCache {
    fn default() -> Self {
        Self::new(16)
    }
}

impl McpSemanticCache {
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
    ) -> Option<SemanticCacheHit> {
        let mut inner = self.inner.lock();
        if inner
            .project_id
            .as_deref()
            .is_some_and(|p| p != key.project_id)
        {
            inner.order.clear();
            inner.entries.clear();
        }
        inner.project_id = Some(key.project_id.clone());

        let mut best: Option<(usize, f32)> = None;
        for &slot in &inner.order {
            let Some(entry) = inner.entries.get(&slot) else {
                continue;
            };
            if entry.key != *key {
                continue;
            }
            let score = cosine_similarity(&entry.prompt_vec, query_vec);
            if score >= min_cosine && best.map(|(_, s)| score > s).unwrap_or(true) {
                best = Some((slot, score));
            }
        }
        let (slot, _) = best?;
        let entry = inner.entries.get(&slot)?;
        *self.hits.lock() += 1;
        Some(SemanticCacheHit {
            response: entry.response.clone(),
            details: entry.details.clone(),
        })
    }

    pub fn insert(
        &self,
        max_entries: usize,
        key: SemanticCacheKey,
        prompt_vec: Vec<f32>,
        response: Value,
        details: PacketDetails,
        detail: ResponseDetail,
    ) {
        let cap = max_entries.max(1);
        let mut inner = self.inner.lock();
        if inner
            .project_id
            .as_deref()
            .is_some_and(|p| p != key.project_id)
        {
            inner.order.clear();
            inner.entries.clear();
        }
        inner.project_id = Some(key.project_id.clone());
        let id = inner.next_id;
        inner.next_id += 1;
        inner.entries.insert(
            id,
            Entry {
                key,
                prompt_vec,
                response,
                details,
                detail,
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
