use chrono::{DateTime, Utc};
use neuromesh_core::ProjectId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub task_signature_hash: String,
    pub context_hash: String,
    pub model: String,
    pub response: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub hit_count: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub struct SemanticCache {
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
}

impl SemanticCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(project_id: &ProjectId, task_hash: &str, context_hash: &str, model: &str) -> String {
        format!("{}:{}:{}:{}", project_id.0, task_hash, context_hash, model)
    }

    pub fn get(
        &self,
        project_id: &ProjectId,
        task_hash: &str,
        context_hash: &str,
        model: &str,
    ) -> Option<CachedResponse> {
        let key = Self::key(project_id, task_hash, context_hash, model);
        let mut map = self.cache.write();
        if let Some(entry) = map.get_mut(&key) {
            entry.hit_count += 1;
            return Some(entry.clone());
        }
        None
    }

    pub fn put(
        &self,
        project_id: &ProjectId,
        task_hash: &str,
        context_hash: &str,
        model: &str,
        response: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) {
        let key = Self::key(project_id, task_hash, context_hash, model);
        let entry = CachedResponse {
            task_signature_hash: task_hash.to_string(),
            context_hash: context_hash.to_string(),
            model: model.to_string(),
            response: response.to_string(),
            prompt_tokens,
            completion_tokens,
            hit_count: 0,
            created_at: Utc::now(),
        };

        self.cache.write().insert(key, entry);
    }

    pub fn invalidate_project(&self, project_id: &ProjectId) {
        let prefix = format!("{}:", project_id.0);
        let mut map = self.cache.write();
        map.retain(|k, _| !k.starts_with(&prefix));
    }

    pub fn clear(&self) {
        self.cache.write().clear();
    }
}
