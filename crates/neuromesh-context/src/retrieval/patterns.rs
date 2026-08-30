//! L2 graph pattern templates for intent-driven expansion.

use crate::retrieval::query_intent::QueryIntent;
use neuromesh_core::{NodeId, NodeType};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashSet;

pub const MAX_PATTERN_FILES: usize = 6;
pub const MAX_PATTERN_HOPS: usize = 1;

/// Expand seed neighborhood using intent-specific bounded graph walk.
pub fn pattern_expand(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    intent: QueryIntent,
) -> HashSet<NodeId> {
    if seeds.is_empty() {
        return HashSet::new();
    }
    let role_filter = intent_role_filter(intent);
    let expanded = graph.neighborhood(seeds, MAX_PATTERN_HOPS);
    let mut file_ids: HashSet<NodeId> = HashSet::new();
    for node_id in expanded {
        let Some(node) = graph.get_node(&node_id) else {
            continue;
        };
        if node.node_type != NodeType::File {
            continue;
        }
        if matches_role(&node.name, &node.file_path.to_string_lossy(), role_filter) {
            file_ids.insert(node_id);
        }
        if file_ids.len() >= MAX_PATTERN_FILES {
            break;
        }
    }
    file_ids
}

fn intent_role_filter(intent: QueryIntent) -> &'static [&'static str] {
    match intent {
        QueryIntent::TraceRouting => &["route", "router", "handler", "app", "endpoint"],
        QueryIntent::TraceMiddleware => &["middleware", "use", "pipeline", "next"],
        QueryIntent::TraceSession => &["session", "cookie", "store", "config"],
        QueryIntent::TraceAuth => &["auth", "login", "session", "middleware"],
        QueryIntent::TraceRender => &["render", "template", "view", "engine"],
        QueryIntent::TraceStatic => &["static", "public", "asset"],
        QueryIntent::TraceQuery => &["model", "repository", "query", "adapter", "store"],
        _ => &[],
    }
}

fn matches_role(name: &str, path: &str, keywords: &[&str]) -> bool {
    if keywords.is_empty() {
        return true;
    }
    let name_l = name.to_lowercase();
    let path_l = path.to_lowercase();
    keywords
        .iter()
        .any(|kw| name_l.contains(kw) || path_l.contains(kw))
}
