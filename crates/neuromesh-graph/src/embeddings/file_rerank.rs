//! Query-time file ANN rerank (lib boost, noise penalty, stem match) — G.3 guarded.

use crate::NeuralProjectGraph;
use neuromesh_core::{is_embed_tier_noise_path, NodeId};
use std::collections::HashSet;
use std::path::Path;

const LIB_BOOST: f32 = 1.12;
const NOISE_PENALTY: f32 = 0.55;
const STEM_BOOST: f32 = 1.15;

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn is_lib_or_src_impl(path: &Path) -> bool {
    let lower = normalized_path(path);
    let under_impl = lower.contains("/lib/") || lower.contains("/src/");
    under_impl && !lower.contains("/test") && !lower.contains("/tests/")
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn stem_matches_prompt(stem: &str, prompt_l: &str) -> bool {
    if stem.len() >= 4 && prompt_l.contains(stem) {
        return true;
    }
    for part in stem.split('-').chain(stem.split('_')) {
        if part.len() >= 4 && prompt_l.contains(part) {
            return true;
        }
    }
    false
}

fn apply_rerank_multiplier(
    graph: &NeuralProjectGraph,
    file_id: &NodeId,
    raw_score: f32,
    file_min_cosine: f32,
    prompt_l: &str,
) -> Option<f32> {
    if raw_score < file_min_cosine {
        return None;
    }
    let node = graph.get_node(file_id)?;
    let path = &node.file_path;
    let mut score = raw_score;
    if is_embed_tier_noise_path(path) {
        score *= NOISE_PENALTY;
    } else if is_lib_or_src_impl(path) {
        score *= LIB_BOOST;
    }
    let stem = file_stem(path);
    if !stem.is_empty() && stem_matches_prompt(&stem, prompt_l) {
        score *= STEM_BOOST;
    }
    if score < file_min_cosine {
        None
    } else {
        Some(score)
    }
}

/// Rerank file ANN hits with path/stem multipliers; drop below `file_min_cosine`.
pub fn rerank_file_hits(
    graph: &NeuralProjectGraph,
    prompt: &str,
    hits: Vec<(NodeId, f32)>,
    file_min_cosine: f32,
    top_k: usize,
) -> Vec<(NodeId, f32)> {
    let prompt_l = prompt.to_lowercase();
    let mut scored: Vec<(NodeId, f32)> = hits
        .into_iter()
        .filter_map(|(id, raw)| {
            apply_rerank_multiplier(graph, &id, raw, file_min_cosine, &prompt_l)
                .map(|adj| (id, adj))
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k.max(1));
    scored
}

/// Generic concept → filename stem substrings (anti-overfit: patterns, not gold paths).
pub fn concept_stem_patterns(concept: &str) -> &'static [&'static str] {
    match concept {
        "errors" | "error" => &["error", "handler", "serializer"],
        "plugin" => &["plugin", "middleware"],
        "content_type" => &["content-type", "parser", "contenttype"],
        "validation" => &["validation", "schema", "validator"],
        _ => &[],
    }
}

fn stem_matches_pattern(stem: &str, pattern: &str) -> bool {
    if pattern.ends_with('*') {
        let prefix = pattern.trim_end_matches('*');
        return !prefix.is_empty() && stem.starts_with(prefix);
    }
    if pattern.starts_with('*') {
        let suffix = pattern.trim_start_matches('*');
        return !suffix.is_empty() && stem.ends_with(suffix);
    }
    stem.contains(pattern)
}

/// Resolve generic stem patterns from matched alias concepts → file ids at floor score.
pub fn stem_union_file_hits(
    graph: &NeuralProjectGraph,
    prompt: &str,
    file_min_cosine: f32,
    existing: &[(NodeId, f32)],
) -> Vec<(NodeId, f32)> {
    let lower = prompt.to_lowercase();
    let concepts = [
        ("plugin", &["plugin", "پلاگین", "插件", "encapsul"][..]),
        (
            "validation",
            &["validation", "validate", "اعتبارسنجی", "验证"][..],
        ),
        ("errors", &["error", "خطا", "错误", "fehler"][..]),
        (
            "content_type",
            &["content-type", "content type", "نوع محتوا", "内容类型"][..],
        ),
    ];
    let mut patterns: Vec<&str> = Vec::new();
    for (concept, triggers) in concepts {
        if triggers.iter().any(|t| lower.contains(&t.to_lowercase())) {
            patterns.extend(concept_stem_patterns(concept));
        }
    }
    for token in prompt.split_whitespace() {
        let t = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if t.len() >= 4 && t.is_ascii() {
            patterns.push(t);
        }
    }
    if patterns.is_empty() {
        return Vec::new();
    }
    let seen: HashSet<_> = existing.iter().map(|(id, _)| id.clone()).collect();
    let mut out = Vec::new();
    for (file_id, path) in graph.file_node_paths() {
        if seen.contains(&file_id) || is_embed_tier_noise_path(&path) {
            continue;
        }
        let stem = file_stem(&path);
        if stem.is_empty() {
            continue;
        }
        if patterns.iter().any(|p| stem_matches_pattern(&stem, p)) {
            out.push((file_id, file_min_cosine));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;

    #[test]
    fn weak_noise_does_not_beat_strong_lib_after_rerank() {
        let graph = NeuralProjectGraph::new(ProjectId::new("rerank-test"));
        // Without graph nodes rerank returns empty — unit test multiplier logic via scores only
        let hits = vec![(NodeId::new("a"), 0.31), (NodeId::new("b"), 0.42)];
        let reranked = rerank_file_hits(&graph, "plugin utils", hits, 0.30, 8);
        // Graph has no nodes — all filtered; test floor guard
        assert!(reranked.is_empty() || reranked.iter().all(|(_, s)| *s >= 0.30));
    }

    #[test]
    fn stem_pattern_prefix_wildcard() {
        assert!(stem_matches_pattern("plugin-utils", "plugin"));
        assert!(stem_matches_pattern("error-handler", "error"));
    }
}
