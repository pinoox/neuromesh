use serde::{Deserialize, Serialize};

/// Benchmark failure taxonomy — enables targeted improvement per failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailureClass {
    NoSeed,
    WrongSeed,
    MissingDependency,
    MissingTaskRole,
    GraphDisconnect,
    MultilingualMiss,
    AliasMiss,
    FalseSufficiency,
    BudgetExhausted,
    L3Exhausted,
    AgentTaskFailure,
    None,
}

impl FailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoSeed => "NO_SEED",
            Self::WrongSeed => "WRONG_SEED",
            Self::MissingDependency => "MISSING_DEPENDENCY",
            Self::MissingTaskRole => "MISSING_TASK_ROLE",
            Self::GraphDisconnect => "GRAPH_DISCONNECT",
            Self::MultilingualMiss => "MULTILINGUAL_MISS",
            Self::AliasMiss => "ALIAS_MISS",
            Self::FalseSufficiency => "FALSE_SUFFICIENCY",
            Self::BudgetExhausted => "BUDGET_EXHAUSTED",
            Self::L3Exhausted => "L3_EXHAUSTED",
            Self::AgentTaskFailure => "AGENT_TASK_FAILURE",
            Self::None => "NONE",
        }
    }
}

/// Classify a retrieval outcome for benchmark JSON reporting.
pub fn classify_retrieval_failure(
    claim: &str,
    seeds_hit: usize,
    critical_gaps: usize,
    budget_truncated: bool,
    level: &str,
    task_success: Option<bool>,
) -> FailureClass {
    if task_success == Some(false) {
        if claim == "likely_sufficient" {
            return FailureClass::FalseSufficiency;
        }
        return FailureClass::AgentTaskFailure;
    }
    if seeds_hit == 0 {
        return FailureClass::NoSeed;
    }
    if budget_truncated {
        return FailureClass::BudgetExhausted;
    }
    if level == "L3" && claim != "likely_sufficient" && critical_gaps > 0 {
        return FailureClass::L3Exhausted;
    }
    if critical_gaps > 0 {
        return FailureClass::MissingDependency;
    }
    if claim == "likely_sufficient" && task_success == Some(false) {
        return FailureClass::FalseSufficiency;
    }
    FailureClass::None
}
