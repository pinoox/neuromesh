use neuromesh_core::{ContextView, RankCandidateView};
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct RankingMetrics {
    pub mrr: f32,
    pub ndcg_at_k: f32,
    pub recall_at_k: f32,
    pub precision_at_k: f32,
    pub emitted_count: usize,
    pub learning_gain: f32,
    pub emission_gain: i32,
}

pub fn mrr(gold_files: &[String], ranked_paths: &[String]) -> f32 {
    for (rank, path) in ranked_paths.iter().enumerate() {
        if gold_files.iter().any(|g| paths_match(g, path)) {
            return 1.0 / (rank + 1) as f32;
        }
    }
    0.0
}

pub fn ndcg_at_k(gold_files: &[String], ranked_paths: &[String], k: usize) -> f32 {
    let k = k.min(ranked_paths.len());
    if k == 0 || gold_files.is_empty() {
        return 0.0;
    }
    let gold: HashSet<_> = gold_files.iter().cloned().collect();
    let mut dcg = 0.0f32;
    for (i, path) in ranked_paths.iter().take(k).enumerate() {
        let rel = if gold.iter().any(|g| paths_match(g, path)) {
            1.0
        } else {
            0.0
        };
        dcg += rel / (i as f32 + 2.0).log2();
    }
    let ideal_hits = gold_files.len().min(k);
    let mut idcg = 0.0f32;
    for i in 0..ideal_hits {
        idcg += 1.0 / (i as f32 + 2.0).log2();
    }
    if idcg <= 0.0 {
        0.0
    } else {
        dcg / idcg
    }
}

pub fn recall_at_k(gold_files: &[String], ranked_paths: &[String], k: usize) -> f32 {
    if gold_files.is_empty() {
        return 1.0;
    }
    let hits = ranked_paths
        .iter()
        .take(k)
        .filter(|p| gold_files.iter().any(|g| paths_match(g, p)))
        .count();
    let unique_gold = gold_files.len();
    (hits as f32 / unique_gold as f32).min(1.0)
}

pub fn precision_at_k(gold_files: &[String], ranked_paths: &[String], k: usize) -> f32 {
    let k = k.min(ranked_paths.len());
    if k == 0 {
        return 0.0;
    }
    let hits = ranked_paths
        .iter()
        .take(k)
        .filter(|p| gold_files.iter().any(|g| paths_match(g, p)))
        .count();
    hits as f32 / k as f32
}

fn paths_match(gold: &str, path: &str) -> bool {
    let g = gold.replace('\\', "/");
    let p = path.replace('\\', "/");
    p.ends_with(&g) || p.contains(&g)
}

pub fn ranked_paths_from_view(view: &ContextView) -> Vec<String> {
    let mut paths: Vec<String> = view
        .rank_candidates
        .iter()
        .map(|c| c.path.clone())
        .collect();
    if paths.is_empty() {
        paths = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
    }
    paths
}

pub fn emitted_paths_from_view(view: &ContextView) -> Vec<String> {
    let emitted: Vec<String> = view
        .rank_candidates
        .iter()
        .filter(|c| c.emitted)
        .map(|c| c.path.clone())
        .collect();
    if !emitted.is_empty() {
        return emitted;
    }
    view.active_nodes
        .iter()
        .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

pub fn compute_ranking_metrics(
    gold_files: &[String],
    before: &ContextView,
    after: &ContextView,
    k: usize,
) -> RankingMetrics {
    let before_ranked = ranked_paths_from_view(before);
    let after_ranked = ranked_paths_from_view(after);
    let before_emitted = emitted_paths_from_view(before);
    let after_emitted = emitted_paths_from_view(after);
    let before_hits = before_emitted
        .iter()
        .filter(|p| gold_files.iter().any(|g| paths_match(g, p)))
        .count();
    let after_hits = after_emitted
        .iter()
        .filter(|p| gold_files.iter().any(|g| paths_match(g, p)))
        .count();
    RankingMetrics {
        mrr: mrr(gold_files, &after_ranked),
        ndcg_at_k: ndcg_at_k(gold_files, &after_ranked, k),
        recall_at_k: recall_at_k(gold_files, &after_emitted, k),
        precision_at_k: precision_at_k(gold_files, &after_emitted, k),
        emitted_count: after_emitted.len(),
        learning_gain: mrr(gold_files, &after_ranked) - mrr(gold_files, &before_ranked),
        emission_gain: after_hits as i32 - before_hits as i32,
    }
}

#[derive(Debug, Clone)]
pub struct DoseResponsePoint {
    pub reinforcement: i32,
    pub learned_bonus: f32,
    pub candidate_score: f32,
    pub rank: usize,
    pub emitted: bool,
}

pub fn dose_response_rank(
    candidates: &[RankCandidateView],
    target_path_substr: &str,
) -> Option<(usize, f32, bool)> {
    for (i, c) in candidates.iter().enumerate() {
        if c.path.contains(target_path_substr) {
            return Some((i + 1, c.score, c.emitted));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndcg_perfect_when_gold_first() {
        let gold = vec!["src/a.ts".into()];
        let ranked = vec!["src/a.ts".into(), "src/b.ts".into()];
        assert!((ndcg_at_k(&gold, &ranked, 2) - 1.0).abs() < 0.01);
    }

    #[test]
    fn mrr_finds_first_hit() {
        let gold = vec!["b.ts".into()];
        let ranked = vec!["a.ts".into(), "pkg/b.ts".into()];
        assert!((mrr(&gold, &ranked) - 0.5).abs() < 0.01);
    }
}
