use crate::selector::RankCandidate;
use crate::unified_score::{cmp_score_path, compute_unified_file_score, file_path_str};
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
}
