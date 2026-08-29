//! Tiered retrieval orchestration, sufficiency estimation, and impact analysis.

pub mod alias;
pub mod budget;
pub mod calibration;
pub mod failure;
pub mod gap;
pub mod impact;
pub mod orchestrator;
pub mod sufficiency;
pub mod task_profile;
pub mod tier;

pub use alias::{expand_aliases, AliasEntry};
pub use budget::RetrievalBudget;
pub use calibration::{CalibrationReport, EvalSuiteMetrics};
pub use failure::FailureClass;
pub use gap::{classify_gaps, ClassifiedGap, GapSeverity};
pub use impact::{retrieve_impact_context, ImpactRetrievalResult};
pub use orchestrator::RetrievalOrchestrator;
pub use sufficiency::{SufficiencyEstimate, SufficiencyEstimator};
pub use task_profile::{detect_task_profile, TaskProfileKind};
pub use tier::RetrievalTier;
