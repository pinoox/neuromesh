//! Single-pass incremental L1→L2→L3 escalation without full re-activation.

use crate::activator::ContextActivator;
use crate::retrieval::budget::RetrievalBudget;
use crate::retrieval::patterns::pattern_expand;
use crate::retrieval::query_intent::QueryPlan;
use crate::retrieval::sufficiency::{SufficiencyEstimate, SufficiencyEstimator};
use crate::retrieval::tier::RetrievalTier;
use neuromesh_core::{ContextView, OptimizationMode, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::normalize_unicode;
use std::collections::HashMap;
use std::time::Instant;

pub struct EscalationResult {
    pub view: ContextView,
    pub final_tier: RetrievalTier,
    pub levels_attempted: Vec<String>,
    pub latency_ms: HashMap<String, u64>,
    pub estimate: SufficiencyEstimate,
}

pub fn run_incremental(
    activator: &ContextActivator,
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    mode: OptimizationMode,
    budget: &RetrievalBudget,
    estimator: &SufficiencyEstimator,
) -> EscalationResult {
    let plan = QueryPlan::from_signature(signature);
    let mut levels_attempted: Vec<String> = Vec::new();
    let mut latency_ms: HashMap<String, u64> = HashMap::new();

    // L1: fast match, 1 hop, concept + alias seeds
    let l1_start = Instant::now();
    let mut sig = signature.clone();
    sig.raw_prompt = normalize_unicode(&sig.raw_prompt);
    sig.engine_override = Some(RetrievalTier::L1.seed_engine());
    let mut view = activator.activate_incremental(
        graph,
        &sig,
        OptimizationMode::MaxSavings.max(mode),
        IncrementalPhase::L1,
        &plan,
        None,
    );
    latency_ms.insert(
        RetrievalTier::L1.as_str().into(),
        l1_start.elapsed().as_millis() as u64,
    );
    levels_attempted.push(RetrievalTier::L1.as_str().into());

    let mut est = estimator.estimate(&view, signature);
    let mut final_tier = RetrievalTier::L1;

    let l1_budget = budget.for_tier(RetrievalTier::L1);
    if est.eligible_for_early_exit
        && view.active_tokens <= l1_budget.selected_tokens
        && est.critical_gaps.is_empty()
    {
        return EscalationResult {
            view,
            final_tier,
            levels_attempted,
            latency_ms,
            estimate: est,
        };
    }

    // L2: pattern expand + 2 hops — only when critical gaps remain
    if !est.critical_gaps.is_empty() {
        let l2_start = Instant::now();
        let seed_ids = activator.seed_node_ids(&view);
        let pattern_files = pattern_expand(graph, &seed_ids, plan.intent);
        sig.engine_override = Some(RetrievalTier::L2.seed_engine());
        view = activator.activate_incremental(
            graph,
            &sig,
            OptimizationMode::Balanced.max(mode),
            IncrementalPhase::L2 {
                extra_files: pattern_files,
                hops: RetrievalTier::L2.graph_hops(),
            },
            &plan,
            Some(view),
        );
        latency_ms.insert(
            RetrievalTier::L2.as_str().into(),
            l2_start.elapsed().as_millis() as u64,
        );
        levels_attempted.push(RetrievalTier::L2.as_str().into());
        final_tier = RetrievalTier::L2;
        est = estimator.estimate(&view, signature);

        let l2_budget = budget.for_tier(RetrievalTier::L2);
        if est.eligible_for_early_exit
            && view.active_tokens <= l2_budget.selected_tokens
            && est.critical_gaps.is_empty()
        {
            return EscalationResult {
                view,
                final_tier,
                levels_attempted,
                latency_ms,
                estimate: est,
            };
        }
    }

    // L3: bounded semantic recovery (max 2 seeds) — only when still critical
    if !est.critical_gaps.is_empty() {
        let l3_start = Instant::now();
        sig.engine_override = Some(RetrievalTier::L3.seed_engine());
        view = activator.activate_incremental(
            graph,
            &sig,
            OptimizationMode::MaxQuality.max(mode),
            IncrementalPhase::L3 {
                max_recovery_seeds: 2,
                hops: RetrievalTier::L2.graph_hops(),
            },
            &plan,
            Some(view),
        );
        latency_ms.insert(
            RetrievalTier::L3.as_str().into(),
            l3_start.elapsed().as_millis() as u64,
        );
        levels_attempted.push(RetrievalTier::L3.as_str().into());
        final_tier = RetrievalTier::L3;
        est = estimator.estimate(&view, signature);
    }

    EscalationResult {
        view,
        final_tier,
        levels_attempted,
        latency_ms,
        estimate: est,
    }
}

#[derive(Debug, Clone)]
pub enum IncrementalPhase {
    L1,
    L2 {
        extra_files: std::collections::HashSet<neuromesh_core::NodeId>,
        hops: u8,
    },
    L3 {
        max_recovery_seeds: u8,
        hops: u8,
    },
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
