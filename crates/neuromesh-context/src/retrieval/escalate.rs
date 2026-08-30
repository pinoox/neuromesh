//! Single-pass incremental L1→L2→L3 escalation without full re-activation.

use crate::activator::ContextActivator;
use crate::retrieval::budget::RetrievalBudget;
use crate::retrieval::embedding_confidence::{is_embedding_reason, low_embedding_confidence};
use crate::retrieval::patterns::pattern_expand;
#[cfg(not(feature = "embeddings"))]
use crate::retrieval::query_intent::QueryPlan;
use crate::retrieval::sufficiency::{SufficiencyEstimate, SufficiencyEstimator};
use crate::retrieval::tier::RetrievalTier;
use neuromesh_core::{
    Config, ContextView, EmbeddingConfig, OptimizationMode, SeedResolutionConfig, TaskSignature,
};
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
    let embedding_config = Config::load().embeddings.clone();
    let app_config = Config::load();
    let retrieval_engine = signature
        .retrieval_engine_override
        .unwrap_or(app_config.retrieval.engine);
    let configured_engine = if signature.retrieval_engine_override.is_some() {
        let mut seed = SeedResolutionConfig::default();
        let mut emb = EmbeddingConfig::default();
        let mut mode = OptimizationMode::Balanced;
        retrieval_engine.apply_preset(&mut mode, &mut seed, &mut emb);
        seed.engine
    } else {
        app_config.seed_resolution.engine
    };
    let embeddings_enabled = if signature.retrieval_engine_override.is_some() {
        let mut seed = SeedResolutionConfig::default();
        let mut emb = EmbeddingConfig::default();
        let mut mode = OptimizationMode::Balanced;
        retrieval_engine.apply_preset(&mut mode, &mut seed, &mut emb);
        emb.enabled
    } else {
        embedding_config.effective_enabled()
    };
    #[cfg(feature = "embeddings")]
    let plan = crate::retrieval::query_intent_embed::from_signature_with_embeddings(
        signature,
        &embedding_config,
    );
    #[cfg(not(feature = "embeddings"))]
    let plan = QueryPlan::from_signature(signature);
    let mut levels_attempted: Vec<String> = Vec::new();
    let mut latency_ms: HashMap<String, u64> = HashMap::new();

    // L1: configured seed engine (embedding-primary by default), 1 hop
    let l1_start = Instant::now();
    let mut sig = signature.clone();
    sig.raw_prompt = normalize_unicode(&sig.raw_prompt);
    sig.engine_override = Some(RetrievalTier::L1.seed_engine(
        configured_engine,
        retrieval_engine,
        embeddings_enabled,
    ));
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
    if can_early_exit(
        &est,
        &view,
        l1_budget,
        activator,
        graph,
        signature,
        &embedding_config,
    ) {
        return EscalationResult {
            view,
            final_tier,
            levels_attempted,
            latency_ms,
            estimate: est,
        };
    }

    // L2: pattern expand + 2 hops — critical gaps or low embedding confidence
    if should_escalate_to_l2(&est, &view, activator, graph, signature, &embedding_config) {
        let l2_start = Instant::now();
        let seed_ids = activator.seed_node_ids(&view);
        let pattern_files = pattern_expand(graph, &seed_ids, plan.intent);
        sig.engine_override = Some(RetrievalTier::L2.seed_engine(
            configured_engine,
            retrieval_engine,
            embeddings_enabled,
        ));
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
        if can_early_exit(
            &est,
            &view,
            l2_budget,
            activator,
            graph,
            signature,
            &embedding_config,
        ) {
            return EscalationResult {
                view,
                final_tier,
                levels_attempted,
                latency_ms,
                estimate: est,
            };
        }
    }

    // L3: bounded semantic recovery (max 2 seeds)
    if should_escalate_to_l3(&est, &view, activator, graph, signature, &embedding_config) {
        let l3_start = Instant::now();
        sig.engine_override = Some(RetrievalTier::L3.seed_engine(
            configured_engine,
            retrieval_engine,
            embeddings_enabled,
        ));
        sig.embed_min_cosine_override = Some(embedding_config.recovery_min_cosine);
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

fn can_early_exit(
    est: &SufficiencyEstimate,
    view: &ContextView,
    tier_budget: &crate::retrieval::budget::TierBudget,
    activator: &ContextActivator,
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    embedding_config: &neuromesh_core::EmbeddingConfig,
) -> bool {
    est.eligible_for_early_exit
        && view.active_tokens <= tier_budget.selected_tokens
        && est.critical_gaps.is_empty()
        && !needs_embedding_escalation(
            view,
            activator,
            graph,
            &signature.raw_prompt,
            embedding_config,
        )
}

fn should_escalate_to_l2(
    est: &SufficiencyEstimate,
    view: &ContextView,
    activator: &ContextActivator,
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    embedding_config: &neuromesh_core::EmbeddingConfig,
) -> bool {
    !est.critical_gaps.is_empty()
        || needs_embedding_escalation(
            view,
            activator,
            graph,
            &signature.raw_prompt,
            embedding_config,
        )
}

fn should_escalate_to_l3(
    est: &SufficiencyEstimate,
    view: &ContextView,
    activator: &ContextActivator,
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    embedding_config: &neuromesh_core::EmbeddingConfig,
) -> bool {
    !est.critical_gaps.is_empty()
        || needs_embedding_escalation(
            view,
            activator,
            graph,
            &signature.raw_prompt,
            embedding_config,
        )
}

/// Escalate when embeddings are on but resolved seeds align poorly with the prompt,
/// unless a strong lexical seed already matched.
fn needs_embedding_escalation(
    view: &ContextView,
    activator: &ContextActivator,
    graph: &NeuralProjectGraph,
    prompt: &str,
    embedding_config: &neuromesh_core::EmbeddingConfig,
) -> bool {
    if !embedding_config.enabled {
        return false;
    }
    let resolved: Vec<_> = view
        .seeds
        .iter()
        .filter(|s| s.resolved_id.is_some())
        .collect();
    if resolved.is_empty() {
        return false;
    }
    let has_strong_lexical = resolved.iter().any(|s| {
        !is_embedding_reason(&s.query)
            && !s.query.starts_with("semantic_embed:")
            && s.confidence >= 0.6
    });
    if has_strong_lexical {
        return false;
    }
    let seed_ids: Vec<_> = activator.seed_node_ids(view).into_iter().collect();
    low_embedding_confidence(graph, prompt, embedding_config, &seed_ids)
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
