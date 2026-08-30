//! Tiered retrieval orchestration, sufficiency estimation, and impact analysis.

pub mod alias;
pub mod budget;
pub mod calibration;
pub mod concept_seeds;
pub mod escalate;
pub mod failure;
pub mod gap;
pub mod impact;
pub mod orchestrator;
pub mod patterns;
pub mod query_intent;
pub mod sufficiency;
pub mod task_profile;
pub mod tier;

pub use alias::{expand_aliases, infer_assisted_seed_signals, AliasEntry};
pub use budget::RetrievalBudget;
pub use calibration::{CalibrationReport, EvalSuiteMetrics};
pub use concept_seeds::resolve_concept_seeds;
pub use escalate::{run_incremental, EscalationResult, IncrementalPhase};
pub use failure::FailureClass;
pub use gap::{classify_gaps, ClassifiedGap, GapSeverity};
pub use impact::{retrieve_impact_context, ImpactRetrievalResult};
pub use orchestrator::RetrievalOrchestrator;
pub use patterns::{pattern_expand, MAX_PATTERN_FILES, MAX_PATTERN_HOPS};
pub use query_intent::{classify_intent, QueryIntent, QueryPlan};
pub use sufficiency::{SufficiencyEstimate, SufficiencyEstimator};
pub use task_profile::{detect_task_profile, TaskProfileKind};
pub use tier::RetrievalTier;
