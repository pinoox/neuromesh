//! Concept-driven L1 seed resolution from code-derived concept index.

use crate::retrieval::alias::canonical_concepts;
use crate::retrieval::query_intent::QueryPlan;
use neuromesh_core::{NodeId, NodeType, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashSet;

const MAX_CONCEPT_SEEDS: usize = 6;
const MIN_CONCEPT_SCORE: f32 = 0.84;

/// Resolve graph symbol seeds from query plan concepts before lexical fallback.
pub fn resolve_concept_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    plan: &QueryPlan,
) -> Vec<(NodeId, f32, String)> {
    let known: HashSet<&str> = canonical_concepts().iter().copied().collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for concept in &plan.concepts {
        if !known.contains(concept.as_str()) {
            continue;
        }
        for node_id in graph.concept_nodes(concept) {
            if !seen.insert(node_id.clone()) {
                continue;
            }
            let Some(node) = graph.get_node(&node_id) else {
                continue;
            };
            if node.node_type == NodeType::File || node.name.contains('.') {
                continue;
            }
            let score = concept_seed_score(&node.name, concept, signature);
            if score < MIN_CONCEPT_SCORE {
                continue;
            }
            out.push((node_id, score, format!("concept:{concept}→{}", node.name)));
            if out.len() >= MAX_CONCEPT_SEEDS {
                out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                return out;
            }
        }
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn concept_seed_score(symbol: &str, concept: &str, signature: &TaskSignature) -> f32 {
    let sym_l = symbol.to_lowercase();
    let mut score: f32 = 0.72;
    if sym_l.contains(concept) {
        score += 0.12;
    }
    if signature
        .identifiers
        .iter()
        .any(|id| sym_l.contains(&id.to_lowercase()) || id.eq_ignore_ascii_case(symbol))
    {
        score += 0.18;
    }
    if signature
        .client_keywords
        .iter()
        .any(|kw| sym_l.contains(&kw.to_lowercase()) || kw.eq_ignore_ascii_case(symbol))
    {
        score += 0.15;
    }
    for hint in &signature.file_hints {
        if let Some(stem) = std::path::Path::new(hint)
            .file_stem()
            .and_then(|s| s.to_str())
        {
            if sym_l.contains(&stem.to_lowercase()) {
                score += 0.1;
            }
        }
    }
    score.min(0.95)
}
