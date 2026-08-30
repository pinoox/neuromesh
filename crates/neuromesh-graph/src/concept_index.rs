//! Code-derived concept dictionary built at graph ingest time.

use neuromesh_core::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type ConceptId = String;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConceptIndex {
    /// Concept → symbol node ids (bounded per concept).
    pub concept_to_nodes: BTreeMap<ConceptId, Vec<NodeId>>,
    /// Lowercase symbol name → concept ids.
    pub symbol_to_concepts: BTreeMap<String, Vec<ConceptId>>,
}

const MAX_NODES_PER_CONCEPT: usize = 24;

/// Heuristic naming patterns → concept id.
fn concepts_for_symbol(name: &str) -> Vec<ConceptId> {
    let lower = name.to_lowercase();
    let mut out = Vec::new();
    let mut push = |c: &str| {
        if !out.iter().any(|x| x == c) {
            out.push(c.to_string());
        }
    };
    if lower.contains("middleware") {
        push("middleware");
    }
    if lower.contains("router") || lower.ends_with("route") || lower.contains("routing") {
        push("routing");
    }
    if lower.starts_with("auth") || lower.contains("authenticate") {
        push("auth");
    }
    if lower.contains("session") || lower.contains("cookie") {
        push("session");
    }
    if lower.contains("repository") || lower.contains("repo") {
        push("repository");
    }
    if lower.starts_with("render") || lower.contains("template") {
        push("render");
    }
    if lower.contains("store") && !lower.contains("restore") {
        push("store");
    }
    if lower.contains("config") || lower.contains("settings") {
        push("config");
    }
    if lower.contains("static") || lower.contains("asset") {
        push("static");
    }
    if lower.contains("model") || lower.contains("schema") {
        push("database");
    }
    out
}

impl ConceptIndex {
    pub fn register_symbol(&mut self, node_id: NodeId, symbol_name: &str) {
        for concept in concepts_for_symbol(symbol_name) {
            let nodes = self.concept_to_nodes.entry(concept.clone()).or_default();
            if nodes.len() < MAX_NODES_PER_CONCEPT && !nodes.contains(&node_id) {
                nodes.push(node_id.clone());
            }
            let aliases = self
                .symbol_to_concepts
                .entry(symbol_name.to_lowercase())
                .or_default();
            if !aliases.contains(&concept) {
                aliases.push(concept);
            }
        }
    }

    pub fn lookup(&self, concept: &str) -> &[NodeId] {
        self.concept_to_nodes
            .get(&concept.to_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn concepts_for_symbol(&self, symbol: &str) -> &[ConceptId] {
        self.symbol_to_concepts
            .get(&symbol.to_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::NodeId;

    #[test]
    fn middleware_symbol_maps() {
        let mut idx = ConceptIndex::default();
        let id = NodeId::new("n1");
        idx.register_symbol(id.clone(), "AuthMiddleware");
        assert!(idx.lookup("middleware").contains(&id));
        assert!(idx.lookup("auth").contains(&id));
    }
}
