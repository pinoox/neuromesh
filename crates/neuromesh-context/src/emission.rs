use crate::selector::RankCandidate;
use crate::unified_score::{
    cmp_score_path, compute_unified_file_score, file_matches_focus_terms, file_path_str,
};
use neuromesh_core::{ContextScoreBreakdown, EmissionDropStage, NodeId, NodeType, Thresholds};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};

/// Tracks per-file emission decisions through selector → filter → materialize.
#[derive(Debug, Default)]
pub struct EmissionPipeline {
    drops: HashMap<NodeId, EmissionDropStage>,
    emitted: HashSet<NodeId>,
    breakdowns: HashMap<NodeId, ContextScoreBreakdown>,
}

impl EmissionPipeline {
    pub fn record_drop(&mut self, id: &NodeId, stage: EmissionDropStage) {
        if !self.emitted.contains(id) {
            self.drops.insert(id.clone(), stage);
        }
    }

    pub fn record_emitted(&mut self, id: &NodeId, breakdown: ContextScoreBreakdown) {
        self.emitted.insert(id.clone());
        self.drops.remove(id);
        self.breakdowns.insert(id.clone(), breakdown);
    }

    pub fn drop_stage(&self, id: &NodeId) -> EmissionDropStage {
        if self.emitted.contains(id) {
            EmissionDropStage::None
        } else {
            self.drops
                .get(id)
                .copied()
                .unwrap_or(EmissionDropStage::NotSelected)
        }
    }

    pub fn is_emitted(&self, id: &NodeId) -> bool {
        self.emitted.contains(id)
    }

    #[allow(clippy::ptr_arg)]
    pub fn suppress_penalized_optional(
        graph: &NeuralProjectGraph,
        optional: &mut Vec<NodeId>,
        required: &HashSet<NodeId>,
        pipeline: &mut EmissionPipeline,
        threshold: f32,
    ) {
        optional.retain(|id| {
            if required.contains(id) {
                return true;
            }
            let penalized = graph
                .file_min_base_relevance(id)
                .is_some_and(|r| r < threshold);
            if penalized {
                pipeline.record_drop(id, EmissionDropStage::PenalizedSuppress);
                false
            } else {
                true
            }
        });
    }

    pub fn rerank_optional_with_learning(
        graph: &NeuralProjectGraph,
        optional: &mut [NodeId],
        scores: &mut HashMap<NodeId, f32>,
        learning_index: &HashMap<NodeId, f32>,
        focus_terms: &HashSet<String>,
        thresholds: &Thresholds,
    ) {
        for id in optional.iter() {
            let base = scores.get(id).copied().unwrap_or(8.0);
            let breakdown = compute_unified_file_score(
                graph,
                id,
                base,
                learning_index,
                focus_terms,
                thresholds,
                0.0,
            );
            scores.insert(id.clone(), breakdown.final_score);
        }
        optional.sort_by(|a, b| {
            let sa = scores.get(a).copied().unwrap_or(0.0);
            let sb = scores.get(b).copied().unwrap_or(0.0);
            cmp_score_path(sa, &file_path_str(graph, a), sb, &file_path_str(graph, b))
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finalize_rank_candidates(
        &self,
        graph: &NeuralProjectGraph,
        scores: &HashMap<NodeId, f32>,
        learning_index: &HashMap<NodeId, f32>,
        selected_set: &HashSet<NodeId>,
        focus_terms: &HashSet<String>,
        thresholds: &Thresholds,
        prior: &[RankCandidate],
    ) -> Vec<RankCandidate> {
        let mut paths: HashMap<String, RankCandidate> =
            prior.iter().map(|c| (c.path.clone(), c.clone())).collect();

        for (id, score) in scores {
            let Some(node) = graph.get_node(id) else {
                continue;
            };
            if node.node_type != NodeType::File {
                continue;
            }
            let path = node.file_path.to_string_lossy().replace('\\', "/");
            let learned = learning_index.get(id).copied().unwrap_or(0.0);
            let breakdown = self.breakdowns.get(id).cloned().unwrap_or_else(|| {
                compute_unified_file_score(
                    graph,
                    id,
                    *score,
                    learning_index,
                    focus_terms,
                    thresholds,
                    0.0,
                )
            });
            let drop = self.drop_stage(id);
            let penalized = graph.file_min_base_relevance(id).is_some_and(|r| r < 0.75);
            let reason = if penalized {
                format!(
                    "penalized:{:.2}",
                    graph
                        .file_min_base_relevance(id)
                        .unwrap_or(node.base_relevance)
                )
            } else if learned >= 12.0 {
                format!("learned:{learned:.1}")
            } else {
                format!("utility:{:.2}", breakdown.final_score)
            };
            paths.insert(
                path.clone(),
                RankCandidate {
                    path,
                    score: breakdown.final_score,
                    learning_bonus: learned,
                    reason,
                    selected: selected_set.contains(id),
                    emitted: self.is_emitted(id),
                    drop_stage: if drop == EmissionDropStage::None && self.is_emitted(id) {
                        None
                    } else if drop != EmissionDropStage::None {
                        Some(drop)
                    } else {
                        Some(EmissionDropStage::NotSelected)
                    },
                    breakdown: Some(breakdown),
                },
            );
        }

        let mut out: Vec<RankCandidate> = paths.into_values().collect();
        out.sort_by(|a, b| cmp_score_path(a.score, &a.path, b.score, &b.path));
        out.truncate(24);
        out
    }

    /// Inject heavily reinforced files into the optional emission queue (positive learning loop).
    #[allow(clippy::too_many_arguments)]
    pub fn ensure_learned_emission(
        graph: &NeuralProjectGraph,
        optional: &mut Vec<NodeId>,
        scores: &mut HashMap<NodeId, f32>,
        required: &HashSet<NodeId>,
        learning_index: &HashMap<NodeId, f32>,
        focus_terms: &HashSet<String>,
        thresholds: &Thresholds,
        optional_cap: usize,
    ) {
        let min_bonus = thresholds.learning_promotion_min_bonus;

        let mut promoted: Vec<(NodeId, f32)> = graph
            .high_learning_files(min_bonus, 24)
            .into_iter()
            .filter(|(id, _)| {
                !required.contains(id)
                    && !graph
                        .file_min_base_relevance(id)
                        .is_some_and(|r| r < thresholds.penalized_suppression_threshold)
                    && file_matches_focus_terms(graph, id, focus_terms)
            })
            .map(|(id, _)| {
                let utility = scores.get(&id).copied().unwrap_or(12.0);
                let breakdown = compute_unified_file_score(
                    graph,
                    &id,
                    utility,
                    learning_index,
                    focus_terms,
                    thresholds,
                    0.0,
                );
                (id, breakdown.final_score)
            })
            .collect();
        promoted.sort_by(|a, b| {
            cmp_score_path(
                a.1,
                &file_path_str(graph, &a.0),
                b.1,
                &file_path_str(graph, &b.0),
            )
        });

        for (id, score) in promoted {
            scores.insert(id.clone(), score);
            if let Some(pos) = optional.iter().position(|x| x == &id) {
                optional.remove(pos);
            } else if optional.len() >= optional_cap {
                let weakest = optional
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let sa = scores.get(*a).copied().unwrap_or(0.0);
                        let sb = scores.get(*b).copied().unwrap_or(0.0);
                        cmp_score_path(sa, &file_path_str(graph, a), sb, &file_path_str(graph, b))
                    })
                    .map(|(idx, _)| idx);
                if let Some(idx) = weakest {
                    let weak_score = scores.get(&optional[idx]).copied().unwrap_or(0.0);
                    if score <= weak_score + 1.0 {
                        continue;
                    }
                    optional.remove(idx);
                }
            }
            optional.insert(0, id);
        }
        optional.truncate(optional_cap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_graph::NeuralProjectGraph;

    #[test]
    fn ensure_learned_emission_prepends_focused_file() {
        let graph = NeuralProjectGraph::new(ProjectId::new("promo"));
        let promo_path = std::path::PathBuf::from("src/components/PromoCodeInput.vue");
        let file = neuromesh_index::IndexedFile::new(
            ProjectId::new("promo"),
            promo_path.clone(),
            promo_path.clone(),
            "export default { name: 'PromoCodeInput' }",
            "hash".to_string(),
            40,
            chrono::Utc::now(),
        );
        graph.ingest_file(
            &file,
            &neuromesh_parser::CodeIntelligenceEngine::analyze(
                &promo_path,
                "export default { name: 'PromoCodeInput' }",
                neuromesh_index::SourceLanguage::Vue,
            ),
            Some("export default { name: 'PromoCodeInput' }"),
        );
        if let Some(node) = graph.resolve_feedback_node("PromoCodeInput") {
            for _ in 0..8 {
                graph.reinforce_node_access(&node.id, true);
            }
        }
        let thresholds = Thresholds::default();
        let learning_index = graph.file_learning_boost_index();
        let focus: HashSet<String> = ["promocodeinput".into(), "checkout".into()]
            .into_iter()
            .collect();
        let mut optional = Vec::new();
        let mut scores = HashMap::new();
        let required = HashSet::new();
        EmissionPipeline::ensure_learned_emission(
            &graph,
            &mut optional,
            &mut scores,
            &required,
            &learning_index,
            &focus,
            &thresholds,
            5,
        );
        assert!(
            optional.iter().any(|id| {
                graph
                    .get_node(id)
                    .is_some_and(|n| n.file_path.to_string_lossy().contains("PromoCodeInput"))
            }),
            "reinforced PromoCodeInput must enter optional emission queue"
        );
    }
}
