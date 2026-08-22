use chrono::{DateTime, Utc};
use neuromesh_core::ProjectId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolResult {
    pub tool_name: String,
    pub input_hash: String,
    pub output_content: String,
    pub hit_count: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub struct ToolCache {
    cache: Arc<RwLock<HashMap<String, CachedToolResult>>>,
}

impl ToolCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(project_id: &ProjectId, tool_name: &str, input_hash: &str) -> String {
        format!("{}:{}:{}", project_id.0, tool_name, input_hash)
    }

    pub fn get(&self, project_id: &ProjectId, tool_name: &str, input_hash: &str) -> Option<String> {
        let key = Self::key(project_id, tool_name, input_hash);
        let mut map = self.cache.write();
        if let Some(entry) = map.get_mut(&key) {
            if entry.expires_at > Utc::now() {
                entry.hit_count += 1;
                return Some(entry.output_content.clone());
            } else {
                map.remove(&key);
            }
        }
        None
    }

    pub fn put(
        &self,
        project_id: &ProjectId,
        tool_name: &str,
        input_hash: &str,
        output_content: &str,
        ttl_seconds: i64,
    ) {
        let key = Self::key(project_id, tool_name, input_hash);
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_seconds);

        let entry = CachedToolResult {
            tool_name: tool_name.to_string(),
            input_hash: input_hash.to_string(),
            output_content: output_content.to_string(),
            hit_count: 0,
            created_at: now,
            expires_at,
        };

        self.cache.write().insert(key, entry);
    }

    pub fn invalidate_tool(&self, tool_name: &str) {
        let mut map = self.cache.write();
        map.retain(|_, v| v.tool_name != tool_name);
    }

    pub fn clear(&self) {
        self.cache.write().clear();
    }
}
