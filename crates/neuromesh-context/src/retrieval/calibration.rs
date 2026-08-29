use crate::retrieval::failure::FailureClass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationReport {
    pub brier_score: f32,
    pub false_sufficiency_rate: f32,
    pub false_insufficiency_rate: f32,
    pub likely_sufficient_threshold: f32,
    pub partial_threshold: f32,
    pub sample_count: usize,
}

/// Evaluation-suite metrics for benchmark release gates.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalSuiteMetrics {
    pub recall: f32,
    pub precision: f32,
    pub f1: f32,
    pub task_success_rate: f32,
    pub task_success_per_1k_tokens: f32,
    pub task_success_per_dollar: f32,
    pub task_success_per_100ms: f32,
    pub false_sufficiency_rate: f32,
    pub impact_recall: f32,
    pub l1_p50_ms: u64,
    pub l1_p95_ms: u64,
    pub l3_rate: f32,
    pub full_workspace_fallback_count: usize,
    #[serde(default)]
    pub failure_classes: Vec<(String, usize)>,
    #[serde(default)]
    pub split: String,
}

impl EvalSuiteMetrics {
    pub fn passes_minimum_gates(&self) -> bool {
        self.full_workspace_fallback_count == 0
            && self.false_sufficiency_rate < 0.10
            && self.recall >= 0.55
            && self.precision >= 0.75
    }

    pub fn record_failure(&mut self, class: FailureClass) {
        if class == FailureClass::None {
            return;
        }
        let key = class.as_str().to_string();
        if let Some((_, count)) = self.failure_classes.iter_mut().find(|(k, _)| k == &key) {
            *count += 1;
        } else {
            self.failure_classes.push((key, 1));
        }
    }
}

/// Brier score for sufficiency calibration (lower is better).
pub fn brier_score(predicted: &[f32], outcomes: &[bool]) -> f32 {
    if predicted.len() != outcomes.len() || predicted.is_empty() {
        return 1.0;
    }
    let sum: f32 = predicted
        .iter()
        .zip(outcomes.iter())
        .map(|(p, &success)| {
            let o = if success { 1.0 } else { 0.0 };
            (p - o).powi(2)
        })
        .sum();
    sum / predicted.len() as f32
}

/// FSR = false_sufficient_cases / all_cases_claimed_sufficient
pub fn false_sufficiency_rate(claimed_sufficient: &[bool], task_succeeded: &[bool]) -> f32 {
    let mut claimed = 0usize;
    let mut false_sufficient = 0usize;
    for (&sufficient, &success) in claimed_sufficient.iter().zip(task_succeeded.iter()) {
        if sufficient {
            claimed += 1;
            if !success {
                false_sufficient += 1;
            }
        }
    }
    if claimed == 0 {
        0.0
    } else {
        false_sufficient as f32 / claimed as f32
    }
}

/// False Insufficiency Rate: claimed insufficient but task would have succeeded.
pub fn false_insufficiency_rate(claimed_insufficient: &[bool], task_succeeded: &[bool]) -> f32 {
    let mut claimed = 0usize;
    let mut false_insuff = 0usize;
    for (&insufficient, &success) in claimed_insufficient.iter().zip(task_succeeded.iter()) {
        if insufficient {
            claimed += 1;
            if success {
                false_insuff += 1;
            }
        }
    }
    if claimed == 0 {
        0.0
    } else {
        false_insuff as f32 / claimed as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsr_computation() {
        let claimed = vec![true, true, true, false];
        let success = vec![true, false, false, true];
        assert!((false_sufficiency_rate(&claimed, &success) - 2.0 / 3.0).abs() < 0.01);
    }
}
