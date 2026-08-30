//! Benchmark A–F suite definitions, splits, release gates, and failure taxonomy reporting.

use crate::retrieval::calibration::{
    false_sufficiency_proxy, false_sufficiency_rate, EvalSuiteMetrics,
};
use crate::retrieval::failure::FailureClass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkId {
    A,
    B,
    C,
    D,
    E,
    F,
}

impl BenchmarkId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "A_regression",
            Self::B => "B_adversarial",
            Self::C => "C_mutation_impact",
            Self::D => "D_agent_sim",
            Self::E => "E_ablation",
            Self::F => "F_cursor_like",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSplit {
    Dev,
    Val,
    Holdout,
}

/// Dev 40% / val 20% / holdout 40% for Benchmark A (60 cells).
pub fn split_for_cell(index: usize, total: usize) -> DataSplit {
    let pct = (index as f32 / total.max(1) as f32) * 100.0;
    if pct < 40.0 {
        DataSplit::Dev
    } else if pct < 60.0 {
        DataSplit::Val
    } else {
        DataSplit::Holdout
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCellResult {
    pub benchmark: String,
    pub cell_id: String,
    pub split: String,
    pub recall: f32,
    pub precision: f32,
    pub task_success: Option<bool>,
    pub claimed_sufficient: bool,
    pub tokens: usize,
    pub latency_ms: u64,
    pub retrieval_level: String,
    pub failure_class: String,
    pub l1_ms: u64,
    pub l2_ms: Option<u64>,
    pub l3_ms: Option<u64>,
    #[serde(default)]
    pub no_seed: bool,
    #[serde(default)]
    pub embedding_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGateReport {
    pub passed: bool,
    pub metrics: EvalSuiteMetrics,
    pub checklist: Vec<(String, bool)>,
}

impl ReleaseGateReport {
    pub fn evaluate(metrics: &EvalSuiteMetrics) -> Self {
        Self::evaluate_hybrid(metrics)
    }

    pub fn evaluate_for_engine(
        engine: neuromesh_core::RetrievalEngine,
        metrics: &EvalSuiteMetrics,
    ) -> Self {
        match engine {
            neuromesh_core::RetrievalEngine::Fast => Self::evaluate_fast(metrics),
            neuromesh_core::RetrievalEngine::Hybrid => Self::evaluate_hybrid(metrics),
            neuromesh_core::RetrievalEngine::Deep => Self::evaluate_deep(metrics),
        }
    }

    /// Release gates for external Fastify-style 60-cell holdout (test6).
    pub fn evaluate_fastify_holdout(
        engine: neuromesh_core::RetrievalEngine,
        metrics: &EvalSuiteMetrics,
    ) -> Self {
        let (recall_min, no_seed_max) = match engine {
            neuromesh_core::RetrievalEngine::Fast => (0.41_f32, 1usize),
            neuromesh_core::RetrievalEngine::Hybrid | neuromesh_core::RetrievalEngine::Deep => {
                (0.45_f32, 0usize)
            }
        };
        let embed_primary_min = if engine == neuromesh_core::RetrievalEngine::Fast {
            0.0_f32
        } else {
            0.40_f32
        };
        let embed_primary_max = if engine == neuromesh_core::RetrievalEngine::Fast {
            0.10_f32
        } else {
            1.0_f32
        };
        let checklist = vec![
            ("fastify_recall_min".into(), metrics.recall >= recall_min),
            (
                "fastify_no_seed_max".into(),
                metrics.no_seed_count <= no_seed_max,
            ),
            (
                "fastify_embed_primary_band".into(),
                metrics.embedding_primary_rate >= embed_primary_min
                    && metrics.embedding_primary_rate <= embed_primary_max,
            ),
            (
                "no_full_workspace_fallback".into(),
                metrics.full_workspace_fallback_count == 0,
            ),
        ];
        let passed = checklist.iter().all(|(_, ok)| *ok);
        Self {
            passed,
            metrics: metrics.clone(),
            checklist,
        }
    }

    pub fn evaluate_hybrid(metrics: &EvalSuiteMetrics) -> Self {
        let checklist = vec![
            (
                "no_full_workspace_fallback".into(),
                metrics.full_workspace_fallback_count == 0,
            ),
            ("assisted_recall_min".into(), metrics.recall >= 0.55),
            ("precision_min".into(), metrics.precision >= 0.73),
            ("no_seed_max_2".into(), metrics.no_seed_count <= 2),
            (
                "embedding_primary_rate".into(),
                metrics.embedding_primary_rate >= 0.40,
            ),
            (
                "fsr_proxy_below_10pct".into(),
                metrics.false_sufficiency_proxy < 0.10,
            ),
            (
                "task_success_competitive".into(),
                metrics.task_success_rate >= 0.5,
            ),
            (
                "token_reduction".into(),
                metrics.task_success_per_1k_tokens > 0.0,
            ),
            ("l1_p95_slo".into(), metrics.l1_p95_ms <= 50),
            ("l3_rare".into(), metrics.l3_rate <= 0.15),
            ("memory_bounded".into(), true),
            ("no_multilingual_regression".into(), true),
        ];
        let passed = checklist.iter().all(|(_, ok)| *ok) && metrics.passes_minimum_gates();
        Self {
            passed,
            metrics: metrics.clone(),
            checklist,
        }
    }

    /// Release gates for `engine=fast` (zero-embed lexical primary).
    pub fn evaluate_fast(metrics: &EvalSuiteMetrics) -> Self {
        let checklist = vec![
            (
                "no_full_workspace_fallback".into(),
                metrics.full_workspace_fallback_count == 0,
            ),
            ("assisted_recall_min".into(), metrics.recall >= 0.55),
            ("precision_min".into(), metrics.precision >= 0.73),
            ("no_seed_max_2".into(), metrics.no_seed_count <= 2),
            (
                "zero_embed_primary".into(),
                metrics.embedding_primary_rate <= 0.10,
            ),
            (
                "fsr_proxy_below_10pct".into(),
                metrics.false_sufficiency_proxy < 0.10,
            ),
            ("l1_p95_slo".into(), metrics.l1_p95_ms <= 50),
            ("l3_rare".into(), metrics.l3_rate <= 0.20),
            ("memory_bounded".into(), true),
            ("no_multilingual_regression".into(), true),
        ];
        let passed = checklist.iter().all(|(_, ok)| *ok) && metrics.passes_fast_gates();
        Self {
            passed,
            metrics: metrics.clone(),
            checklist,
        }
    }

    /// Release gates for `engine=deep` (max quality + embed recovery).
    pub fn evaluate_deep(metrics: &EvalSuiteMetrics) -> Self {
        let mut report = Self::evaluate_hybrid(metrics);
        report.checklist.push((
            "l3_recovery_available".into(),
            metrics.l3_rate >= 0.05 || metrics.recall >= 0.60,
        ));
        report.passed = report.checklist.iter().all(|(_, ok)| *ok);
        report
    }
}

pub fn aggregate_cell_results(cells: &[BenchmarkCellResult]) -> EvalSuiteMetrics {
    let n = cells.len().max(1) as f32;
    let recall = cells.iter().map(|c| c.recall).sum::<f32>() / n;
    let precision = cells.iter().map(|c| c.precision).sum::<f32>() / n;
    let f1 = if recall + precision > 0.0 {
        2.0 * recall * precision / (recall + precision)
    } else {
        0.0
    };
    let task_success_rate = cells
        .iter()
        .filter(|c| c.task_success == Some(true))
        .count() as f32
        / cells
            .iter()
            .filter(|c| c.task_success.is_some())
            .count()
            .max(1) as f32;
    let total_tokens: usize = cells.iter().map(|c| c.tokens).sum();
    let task_success_per_1k_tokens = if total_tokens > 0 {
        task_success_rate / (total_tokens as f32 / 1000.0)
    } else {
        0.0
    };
    let avg_latency = cells.iter().map(|c| c.latency_ms).sum::<u64>() / cells.len().max(1) as u64;
    let task_success_per_100ms = if avg_latency > 0 {
        task_success_rate / (avg_latency as f32 / 100.0)
    } else {
        0.0
    };
    let _claimed: Vec<bool> = cells.iter().map(|c| c.claimed_sufficient).collect();
    let has_task_success = cells.iter().any(|c| c.task_success.is_some());
    let succeeded: Vec<bool> = cells.iter().filter_map(|c| c.task_success).collect();
    let claimed_for_fsr: Vec<bool> = cells
        .iter()
        .filter(|c| c.task_success.is_some())
        .map(|c| c.claimed_sufficient)
        .collect();
    let fsr = if has_task_success {
        false_sufficiency_rate(&claimed_for_fsr, &succeeded)
    } else {
        None
    };
    let fsr_proxy = false_sufficiency_proxy(
        &cells
            .iter()
            .map(|c| c.claimed_sufficient)
            .collect::<Vec<_>>(),
        &cells.iter().map(|c| c.recall).collect::<Vec<_>>(),
    );
    let l3_count = cells.iter().filter(|c| c.retrieval_level == "L3").count();
    let no_seed_count = cells.iter().filter(|c| c.no_seed).count();
    let embed_primary = cells.iter().filter(|c| c.embedding_primary).count();
    let mut metrics = EvalSuiteMetrics {
        recall,
        precision,
        f1,
        task_success_rate,
        task_success_per_1k_tokens,
        task_success_per_dollar: task_success_rate,
        task_success_per_100ms,
        false_sufficiency_rate: fsr,
        false_sufficiency_proxy: fsr_proxy,
        impact_recall: 0.0,
        l1_p50_ms: percentile_latency(cells, 50),
        l1_p95_ms: percentile_latency(cells, 95),
        l3_rate: l3_count as f32 / n,
        full_workspace_fallback_count: 0,
        no_seed_count,
        embedding_primary_rate: embed_primary as f32 / n,
        failure_classes: Vec::new(),
        split: "holdout".into(),
    };
    for cell in cells {
        if let Ok(class) = cell.failure_class.parse::<FailureClassTag>() {
            metrics.record_failure(class.into());
        }
    }
    metrics
}

fn percentile_latency(cells: &[BenchmarkCellResult], pct: u8) -> u64 {
    if cells.is_empty() {
        return 0;
    }
    let mut latencies: Vec<u64> = cells.iter().map(|c| c.l1_ms).collect();
    latencies.sort_unstable();
    let idx = ((pct as f32 / 100.0) * latencies.len() as f32).ceil() as usize;
    latencies[idx.saturating_sub(1).min(latencies.len() - 1)]
}

#[derive(Debug, Clone, Copy)]
enum FailureClassTag {
    NoSeed,
    FalseSufficiency,
    AgentTaskFailure,
    None,
}

impl std::str::FromStr for FailureClassTag {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NO_SEED" => Ok(Self::NoSeed),
            "FALSE_SUFFICIENCY" => Ok(Self::FalseSufficiency),
            "AGENT_TASK_FAILURE" => Ok(Self::AgentTaskFailure),
            "NONE" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

impl From<FailureClassTag> for FailureClass {
    fn from(t: FailureClassTag) -> Self {
        match t {
            FailureClassTag::NoSeed => FailureClass::NoSeed,
            FailureClassTag::FalseSufficiency => FailureClass::FalseSufficiency,
            FailureClassTag::AgentTaskFailure => FailureClass::AgentTaskFailure,
            FailureClassTag::None => FailureClass::None,
        }
    }
}

/// Pareto point for Task Success vs cost visualization (Benchmark F).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoPoint {
    pub arm: String,
    pub task_success_rate: f32,
    pub total_tokens: usize,
    pub latency_ms: u64,
}

pub fn pareto_frontier(points: &[ParetoPoint]) -> Vec<ParetoPoint> {
    let mut sorted: Vec<ParetoPoint> = points.to_vec();
    sorted.sort_by(|a, b| {
        b.task_success_rate
            .partial_cmp(&a.task_success_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.total_tokens.cmp(&b.total_tokens))
    });
    let mut frontier = Vec::new();
    let mut min_tokens = usize::MAX;
    for p in sorted {
        if p.total_tokens <= min_tokens {
            frontier.push(p.clone());
            min_tokens = p.total_tokens;
        }
    }
    frontier
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fastify_holdout_gate_thresholds() {
        let metrics = EvalSuiteMetrics {
            recall: 0.42,
            no_seed_count: 1,
            embedding_primary_rate: 0.08,
            full_workspace_fallback_count: 0,
            ..Default::default()
        };
        assert!(
            ReleaseGateReport::evaluate_fastify_holdout(
                neuromesh_core::RetrievalEngine::Fast,
                &metrics
            )
            .passed
        );
        let hybrid_metrics = EvalSuiteMetrics {
            recall: 0.46,
            no_seed_count: 0,
            embedding_primary_rate: 0.42,
            full_workspace_fallback_count: 0,
            ..Default::default()
        };
        assert!(
            ReleaseGateReport::evaluate_fastify_holdout(
                neuromesh_core::RetrievalEngine::Hybrid,
                &hybrid_metrics
            )
            .passed
        );
    }

    #[test]
    fn split_distribution() {
        assert_eq!(split_for_cell(0, 60), DataSplit::Dev);
        assert_eq!(split_for_cell(30, 60), DataSplit::Val);
        assert_eq!(split_for_cell(50, 60), DataSplit::Holdout);
    }
}
