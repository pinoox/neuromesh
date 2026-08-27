use crate::registry::{ReversibleContextRegistry, StoredFold};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldExpansion {
    pub fold_id: String,
    pub symbol_name: String,
    pub signature: String,
    pub original_body: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub restored_tokens: usize,
}

pub struct ExpansionEngine {
    registry: Arc<ReversibleContextRegistry>,
}

impl ExpansionEngine {
    pub fn new(registry: Arc<ReversibleContextRegistry>) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &Arc<ReversibleContextRegistry> {
        &self.registry
    }

    /// Expands a folded intron by fold_id without touching the disk.
    pub fn expand_fold(&self, fold_id: &str) -> Option<FoldExpansion> {
        let stored: StoredFold = self.registry.get_fold(fold_id)?;
        Some(FoldExpansion {
            fold_id: stored.fold.fold_id,
            symbol_name: stored.fold.symbol_name,
            signature: stored.fold.signature,
            original_body: stored.fold.original_body.clone(),
            file_path: stored.file_path.to_string_lossy().into_owned(),
            start_line: stored.fold.start_line,
            end_line: stored.fold.end_line,
            restored_tokens: stored.fold.saved_tokens,
        })
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
            sidecar: false,
            folded_symbols: Vec::new(),
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
