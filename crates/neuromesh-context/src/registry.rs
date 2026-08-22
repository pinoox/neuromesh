use neuromesh_core::{ContextNode, InactiveContextDescriptor, NodeId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ReversibleContextRegistry {
    inactive_nodes: Arc<RwLock<HashMap<NodeId, InactiveContextDescriptor>>>,
    node_store: Arc<RwLock<HashMap<NodeId, ContextNode>>>,
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

        self.inactive_nodes.write().insert(node.id.clone(), descriptor);
        self.node_store.write().insert(node.id.clone(), node.clone());
    }

    pub fn get_inactive_descriptors(&self) -> Vec<InactiveContextDescriptor> {
        self.inactive_nodes.read().values().cloned().collect()
    }

    pub fn retrieve_and_activate(&self, id: &NodeId) -> Option<ContextNode> {
        self.inactive_nodes.write().remove(id);
        self.node_store.read().get(id).cloned()
    }

    pub fn clear(&self) {
        self.inactive_nodes.write().clear();
        self.node_store.write().clear();
    }
}
