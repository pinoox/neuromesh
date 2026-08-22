use chrono::{DateTime, Utc};
use neuromesh_core::ProjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFact {
    pub id: String,
    pub project_id: ProjectId,
    pub category: String, // 'architecture', 'convention', 'design_token', 'framework', 'constraint'
    pub key: String,
    pub content: String,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProjectFact {
    pub fn new(
        project_id: ProjectId,
        category: impl Into<String>,
        key: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_id,
            category: category.into(),
            key: key.into(),
            content: content.into(),
            confidence: 1.0,
            created_at: now,
            updated_at: now,
        }
    }
}
