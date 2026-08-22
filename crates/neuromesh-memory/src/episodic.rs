use chrono::{DateTime, Utc};
use neuromesh_core::{NodeId, ProjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicRecord {
    pub id: String,
    pub project_id: ProjectId,
    pub task_signature_hash: String,
    pub intent: String,
    pub summary: String,
    pub activated_node_ids: Vec<NodeId>,
    pub successful_path: Vec<String>,
    pub success: bool,
    pub tokens_saved: usize,
    pub created_at: DateTime<Utc>,
}

impl EpisodicRecord {
    pub fn new(
        project_id: ProjectId,
        task_hash: String,
        intent: String,
        summary: String,
        activated_nodes: Vec<NodeId>,
        successful_path: Vec<String>,
        success: bool,
        tokens_saved: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            project_id,
            task_signature_hash: task_hash,
            intent,
            summary,
            activated_node_ids: activated_nodes,
            successful_path,
            success,
            tokens_saved,
            created_at: Utc::now(),
        }
    }
}
