use crate::embeddings::EmbeddingIndex;
use crate::query::tokenize;
use crate::NeuralProjectGraph;
use neuromesh_core::{EdgeType, NodeId, NodeType, TaskSignature};
use std::collections::{HashMap, HashSet};

const MIN_POOL: usize = 200;
const MAX_POOL: usize = 500;

fn push_index(seen: &mut HashSet<usize>, ordered: &mut Vec<usize>, idx: usize) {
    if seen.insert(idx) {
        ordered.push(idx);
    }
}

fn push_id(
    seen: &mut HashSet<usize>,
    ordered: &mut Vec<usize>,
    id_to_idx: &HashMap<&NodeId, usize>,
    id: &NodeId,
) {
    if let Some(&idx) = id_to_idx.get(id) {
        push_index(seen, ordered, idx);
    }
}

/// Union coarse lexical/graph candidates mapped to embedding index positions.
pub fn coarse_candidate_indices(
    graph: &NeuralProjectGraph,
    index: &EmbeddingIndex,
    signature: &TaskSignature,
    prompt: &str,
    max_pool: usize,
) -> Vec<usize> {
    let max_pool = max_pool.clamp(MIN_POOL, MAX_POOL);
    if index.node_ids.is_empty() {
        return Vec::new();
    }

    let id_to_idx: HashMap<&NodeId, usize> = index
        .node_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();

    let mut seen: HashSet<usize> = HashSet::new();
    let mut ordered: Vec<usize> = Vec::new();

    for ident in &signature.identifiers {
        for hit in graph.search_symbols(ident, 12) {
            push_id(&mut seen, &mut ordered, &id_to_idx, &hit.id);
        }
    }

    for token in tokenize(prompt) {
        if token.len() < 3 || is_prompt_stop(&token) {
            continue;
        }
        for hit in graph.search_symbols(&token, 8) {
            push_id(&mut seen, &mut ordered, &id_to_idx, &hit.id);
        }
    }

    for hint in &signature.file_hints {
        if let Some(file_id) = graph.resolve_file_hint(hint) {
            push_id(&mut seen, &mut ordered, &id_to_idx, &file_id);
            if let Some(file_node) = graph.get_node(&file_id) {
                let path = file_node.file_path.clone();
                for (i, node_id) in index.node_ids.iter().enumerate() {
                    if seen.contains(&i) {
                        continue;
                    }
                    if graph
                        .get_node(node_id)
                        .is_some_and(|n| n.file_path == path && n.node_type != NodeType::File)
                    {
                        push_index(&mut seen, &mut ordered, i);
                    }
                }
            }
        }
    }

    let seed_ids: Vec<NodeId> = ordered
        .iter()
        .filter_map(|&idx| index.node_ids.get(idx).cloned())
        .collect();
    for seed_id in seed_ids {
        for (neighbor_id, edge) in graph.get_connected_neighbors(&seed_id) {
            if matches!(
                edge.edge_type,
                EdgeType::Imports
                    | EdgeType::Calls
                    | EdgeType::Contains
                    | EdgeType::DependsOn
                    | EdgeType::References
            ) {
                push_id(&mut seen, &mut ordered, &id_to_idx, &neighbor_id);
            }
        }
    }

    ordered.truncate(max_pool);
    ordered
}

fn is_prompt_stop(token: &str) -> bool {
    matches!(
        token.to_lowercase().as_str(),
        "the"
            | "and"
            | "for"
            | "how"
            | "does"
            | "what"
            | "where"
            | "when"
            | "with"
            | "from"
            | "that"
            | "this"
            | "into"
            | "about"
            | "using"
            | "work"
            | "works"
    )
}
