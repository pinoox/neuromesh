use crate::activator::ContextActivator;
use crate::retrieval::budget::RetrievalBudget;
use crate::retrieval::sufficiency::{SufficiencyEstimate, SufficiencyEstimator};
use crate::retrieval::tier::RetrievalTier;
use neuromesh_core::{ContextView, OptimizationMode, RetrievalMetadata, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::normalize_unicode;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub struct RetrievalOrchestrator {
    budget: RetrievalBudget,
    estimator: SufficiencyEstimator,
}

impl RetrievalOrchestrator {
    pub fn new(budget: RetrievalBudget) -> Self {
        Self {
            budget,
            estimator: SufficiencyEstimator::default(),
        }
    }

    /// L1 → L2 → L3 tiered retrieval with conservative early exit.
    pub fn run(
        &self,
        activator: &ContextActivator,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
    ) -> ContextView {
        let mut levels_attempted: Vec<String> = Vec::new();
        let mut latency_ms: HashMap<String, u64> = HashMap::new();
        let mut best_view: Option<ContextView> = None;
        let mut best_est: Option<SufficiencyEstimate> = None;
        let mut final_tier = RetrievalTier::L1;

        for tier in RetrievalTier::all() {
            let started = Instant::now();
            let mut sig = signature.clone();
            sig.engine_override = Some(tier.seed_engine());
            sig.raw_prompt = normalize_unicode(&sig.raw_prompt);
            let tier_mode = optimization_for_tier(*tier, mode);
            let tier_budget = self.budget.for_tier(*tier);

            let view = activator.activate_with_hops(graph, &sig, tier_mode, tier.graph_hops());
            let elapsed = started.elapsed().as_millis() as u64;
            levels_attempted.push(tier.as_str().to_string());
            latency_ms.insert(tier.as_str().to_string(), elapsed);

            let est = self.estimator.estimate(&view, signature);
            final_tier = *tier;
            best_view = Some(view);
            best_est = Some(est.clone());

            let within_budget = elapsed <= tier_budget.latency_ms.saturating_mul(3)
                && best_view
                    .as_ref()
                    .map(|v| v.active_tokens <= tier_budget.selected_tokens)
                    .unwrap_or(true);

            if est.eligible_for_early_exit && within_budget {
                break;
            }
            if *tier == RetrievalTier::L3 {
                break;
            }
        }

        let mut view = best_view.expect("orchestrator always produces a view");
        let est = best_est.expect("orchestrator always estimates sufficiency");
        enforce_no_full_workspace(&mut view, graph);

        let next_action = suggest_next_action(&est, &view);
        let suggested_keywords = if matches!(est.claim.as_str(), "partial" | "insufficient") {
            Some(suggest_keywords(signature, &view))
        } else {
            None
        };

        view.retrieval = Some(RetrievalMetadata {
            retrieval_level: final_tier.as_str().to_string(),
            sufficiency_score: est.score,
            confidence: est.confidence,
            claim: est.claim.clone(),
            levels_attempted,
            latency_ms,
            full_workspace_fallback: false,
            critical_gaps: est.critical_gaps.clone(),
            non_critical_gaps: est.non_critical_gaps.clone(),
            eligible_for_early_exit: est.eligible_for_early_exit,
            next_action,
            suggested_keywords,
        });

        view
    }
}

fn optimization_for_tier(tier: RetrievalTier, fallback: OptimizationMode) -> OptimizationMode {
    match tier {
        RetrievalTier::L1 => OptimizationMode::MaxSavings,
        RetrievalTier::L2 => OptimizationMode::Balanced,
        RetrievalTier::L3 => OptimizationMode::MaxQuality,
    }
    .max(fallback)
}

trait ModeMax {
    fn max(self, other: OptimizationMode) -> OptimizationMode;
}

impl ModeMax for OptimizationMode {
    fn max(self, other: OptimizationMode) -> OptimizationMode {
        use OptimizationMode::*;
        match (self, other) {
            (MaxQuality, _) | (_, MaxQuality) => MaxQuality,
            (Balanced, MaxSavings) | (MaxSavings, Balanced) => Balanced,
            _ => self,
        }
    }
}

/// Hard ban: selected tokens must stay below workspace tokens.
fn enforce_no_full_workspace(view: &mut ContextView, graph: &NeuralProjectGraph) {
    let workspace = graph.total_tokens().max(1);
    if view.active_tokens >= workspace {
        view.active_nodes.retain(|n| {
            n.activation_score >= 0.8
                || n.expansion_reason
                    .as_deref()
                    .is_some_and(|r| r.contains("seed"))
        });
        view.active_tokens = view.active_nodes.iter().map(|n| n.node.token_cost).sum();
    }
    debug_assert!(
        view.active_tokens < workspace,
        "full-workspace fallback forbidden"
    );
}

fn suggest_next_action(est: &SufficiencyEstimate, view: &ContextView) -> Option<String> {
    if est.eligible_for_early_exit {
        return None;
    }
    if !est.critical_gaps.is_empty() {
        return Some("neuromesh_expand_gap".into());
    }
    if view
        .coverage
        .as_ref()
        .is_some_and(|c| c.claim == "no_seed_resolved")
    {
        return Some("neuromesh_search_symbols".into());
    }
    if est.claim == "partial" {
        return Some("neuromesh_search_symbols".into());
    }
    None
}

fn suggest_keywords(signature: &TaskSignature, view: &ContextView) -> Vec<String> {
    let mut out: Vec<String> = signature
        .identifiers
        .iter()
        .chain(signature.client_keywords.iter())
        .cloned()
        .collect();
    if let Some(coverage) = view.coverage.as_ref() {
        for gap in &coverage.packet_gaps {
            if let Some(name) = gap.path.rsplit('/').next() {
                let stem = name.trim_end_matches(".rs").trim_end_matches(".ts");
                if stem.len() >= 3 && !out.iter().any(|k| k == stem) {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out.truncate(8);
    out
}
