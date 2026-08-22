use crate::registry::ReversibleContextRegistry;
use chrono::{DateTime, Utc};
use neuromesh_core::{ActivatedNodeView, ContextStatus, NodeId, ProjectId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionAuditRecord {
    pub expansion_id: usize,
    pub project_id: ProjectId,
    pub timestamp: DateTime<Utc>,
    pub reason: String,
    pub activated_node_id: NodeId,
    pub added_tokens: usize,
}

pub struct ExpansionEngine {
    registry: Arc<ReversibleContextRegistry>,
}

impl ExpansionEngine {
    pub fn new(registry: Arc<ReversibleContextRegistry>) -> Self {
        Self { registry }
    }

    /// Expands context by reactivating an inactive node
    pub fn expand_node(
        &self,
        node_id: &NodeId,
        reason: &str,
    ) -> Option<(ActivatedNodeView, ExpansionAuditRecord)> {
        let node = self.registry.retrieve_and_activate(node_id)?;

        let view = ActivatedNodeView {
            node: node.clone(),
            activation_score: 1.0,
            status: ContextStatus::Expanded,
            expansion_reason: Some(reason.to_string()),
        };

        let audit = ExpansionAuditRecord {
            expansion_id: 1,
            project_id: node.project_id.clone(),
            timestamp: Utc::now(),
            reason: reason.to_string(),
            activated_node_id: node.id,
            added_tokens: node.token_cost,
        };

        Some((view, audit))
    }
}
