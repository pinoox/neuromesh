use crate::fold::normalize_fold_query;
use crate::skeleton::FoldedIntron;
use neuromesh_core::{ContextNode, InactiveContextDescriptor, NodeId, ProjectId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StoredFold {
    pub fold: FoldedIntron,
    pub file_path: PathBuf,
}

#[derive(Clone, Default)]
pub struct ReversibleContextRegistry {
    inactive_nodes: Arc<RwLock<HashMap<NodeId, InactiveContextDescriptor>>>,
    node_store: Arc<RwLock<HashMap<NodeId, ContextNode>>>,
    folds: Arc<RwLock<HashMap<String, StoredFold>>>,
    session_project: Arc<RwLock<Option<ProjectId>>>,
}

impl ReversibleContextRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_inactive(
        &self,
        node: &ContextNode,
        relevance: f32,
        confidence: f32,
        activation_score: f32,
        parent_node: Option<NodeId>,
    ) {
        let descriptor = InactiveContextDescriptor {
            id: node.id.clone(),
            file_path: node.file_path.clone(),
            line_range: node.line_range.clone(),
            content_hash: node.content_hash.clone(),
            version: 1,
            token_cost: node.token_cost,
            relevance,
            confidence,
            activation_score,
            parent_node,
        };

        self.inactive_nodes
            .write()
            .insert(node.id.clone(), descriptor);
        self.node_store
            .write()
            .insert(node.id.clone(), node.clone());
    }

    pub fn register_fold(&self, file_path: PathBuf, fold: FoldedIntron) {
        self.folds
            .write()
            .insert(fold.fold_id.clone(), StoredFold { fold, file_path });
    }

    pub fn get_fold(&self, fold_id: &str) -> Option<StoredFold> {
        let query = normalize_fold_query(fold_id);
        if query.is_empty() {
            return None;
        }
        let folds = self.folds.read();
        if let Some(hit) = folds.get(&query) {
            return Some(hit.clone());
        }
        let prefix = format!("{query}_");
        let mut prefixed: Vec<StoredFold> = folds
            .values()
            .filter(|stored| {
                stored.fold.fold_id == query || stored.fold.fold_id.starts_with(&prefix)
            })
            .cloned()
            .collect();
        if prefixed.len() == 1 {
            return prefixed.pop();
        }
        if prefixed.len() > 1 {
            prefixed.sort_by(|a, b| {
                b.fold
                    .task_score
                    .partial_cmp(&a.fold.task_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return prefixed.into_iter().next();
        }
        let mut by_symbol: Vec<StoredFold> = folds
            .values()
            .filter(|stored| stored.fold.symbol_name.eq_ignore_ascii_case(&query))
            .cloned()
            .collect();
        if by_symbol.len() == 1 {
            return by_symbol.pop();
        }
        if by_symbol.len() > 1 {
            by_symbol.sort_by(|a, b| {
                b.fold
                    .task_score
                    .partial_cmp(&a.fold.task_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return by_symbol.into_iter().next();
        }
        None
    }

    pub fn get_inactive_descriptors(&self) -> Vec<InactiveContextDescriptor> {
        self.inactive_nodes.read().values().cloned().collect()
    }

    pub fn retrieve_and_activate(&self, id: &NodeId) -> Option<ContextNode> {
        self.inactive_nodes.write().remove(id);
        self.node_store.read().get(id).cloned()
    }

    pub fn begin_activate(&self, project_id: &ProjectId) {
        let mut session = self.session_project.write();
        if session.as_ref() != Some(project_id) {
            self.folds.write().clear();
            *session = Some(project_id.clone());
        }
        self.inactive_nodes.write().clear();
        self.node_store.write().clear();
    }

    pub fn fold_count(&self) -> usize {
        self.folds.read().len()
    }

    pub fn clear(&self) {
        self.inactive_nodes.write().clear();
        self.node_store.write().clear();
        self.folds.write().clear();
        *self.session_project.write() = None;
    }
}
