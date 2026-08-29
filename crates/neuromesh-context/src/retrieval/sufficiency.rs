use crate::retrieval::gap::{classify_gaps, GapSeverity};
use crate::retrieval::task_profile::{detect_task_profile, task_role_coverage};
use neuromesh_core::{ContextView, TaskSignature};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SufficiencyEstimate {
    pub score: f32,
    pub confidence: f32,
    pub claim: String,
    pub eligible_for_early_exit: bool,
    pub critical_gaps: Vec<String>,
    pub non_critical_gaps: Vec<String>,
}

/// Calibrated weights — defaults are hypotheses; tune on dev set (see calibration.rs).
#[derive(Debug, Clone)]
pub struct SufficiencyWeights {
    pub relevance: f32,
    pub dependency: f32,
    pub task_role: f32,
    pub coherence: f32,
    pub seed_conf: f32,
    pub gap_penalty: f32,
    pub unresolved_penalty: f32,
    pub redundancy_penalty: f32,
    pub likely_sufficient_threshold: f32,
    pub partial_threshold: f32,
}

impl Default for SufficiencyWeights {
    fn default() -> Self {
        Self {
            relevance: 0.25,
            dependency: 0.20,
            task_role: 0.20,
            coherence: 0.10,
            seed_conf: 0.15,
            gap_penalty: 0.15,
            unresolved_penalty: 0.10,
            redundancy_penalty: 0.05,
            likely_sufficient_threshold: 0.70,
            partial_threshold: 0.40,
        }
    }
}

#[derive(Default)]
pub struct SufficiencyEstimator {
    weights: SufficiencyWeights,
}

impl SufficiencyEstimator {
    pub fn new(weights: SufficiencyWeights) -> Self {
        Self { weights }
    }

    pub fn estimate(&self, view: &ContextView, signature: &TaskSignature) -> SufficiencyEstimate {
        let profile = detect_task_profile(signature);
        let coverage = view.coverage.as_ref();
        let seeds_hit = coverage.map(|c| c.seeds_hit.len()).unwrap_or(0);
        let seeds_missed = coverage.map(|c| c.seeds_missed.len()).unwrap_or(0);
        let seeds_attempted = seeds_hit + seeds_missed;

        let relevance = if seeds_attempted == 0 {
            0.0
        } else {
            seeds_hit as f32 / seeds_attempted as f32
        };

        let dependency = view.seed_call_coverage.clamp(0.0, 1.0);
        let task_role = task_role_coverage(view, profile);
        let coherence = graph_coherence(view);
        let seed_conf = average_seed_confidence(view);

        let gaps = coverage
            .map(|c| classify_gaps(&c.packet_gaps, profile))
            .unwrap_or_default();
        let critical_gaps: Vec<String> = gaps
            .iter()
            .filter(|g| g.severity == GapSeverity::Critical)
            .map(|g| g.label.clone())
            .collect();
        let non_critical_gaps: Vec<String> = gaps
            .iter()
            .filter(|g| g.severity == GapSeverity::NonCritical)
            .map(|g| g.label.clone())
            .collect();

        let gap_count = critical_gaps.len() as f32 + non_critical_gaps.len() as f32 * 0.25;
        let unresolved = view.unresolved.len() as f32;
        let redundancy = redundancy_ratio(view);

        let w = &self.weights;
        let raw = w.relevance * relevance
            + w.dependency * dependency
            + w.task_role * task_role
            + w.coherence * coherence
            + w.seed_conf * seed_conf
            - w.gap_penalty * gap_count
            - w.unresolved_penalty * unresolved.min(5.0) * 0.1
            - w.redundancy_penalty * redundancy;

        let score = raw.clamp(0.0, 1.0);
        let confidence = (seed_conf * 0.4 + relevance * 0.3 + task_role * 0.3).clamp(0.0, 1.0);

        // Conservative policy: prefer partial over false sufficient.
        let claim = if seeds_hit == 0 {
            "insufficient".to_string()
        } else if !critical_gaps.is_empty() || score < w.partial_threshold {
            "partial".to_string()
        } else if score >= w.likely_sufficient_threshold
            && critical_gaps.is_empty()
            && task_role >= 0.5
            && confidence >= 0.65
        {
            "likely_sufficient".to_string()
        } else if score >= w.partial_threshold {
            "partial".to_string()
        } else {
            "insufficient".to_string()
        };

        let eligible_for_early_exit = claim == "likely_sufficient"
            && critical_gaps.is_empty()
            && task_role >= 0.5
            && confidence >= 0.65
            && !view.over_budget;

        SufficiencyEstimate {
            score,
            confidence,
            claim,
            eligible_for_early_exit,
            critical_gaps,
            non_critical_gaps,
        }
    }
}

fn average_seed_confidence(view: &ContextView) -> f32 {
    let resolved: Vec<f32> = view
        .seeds
        .iter()
        .filter(|s| s.resolved_id.is_some())
        .map(|s| s.confidence)
        .collect();
    if resolved.is_empty() {
        return 0.0;
    }
    resolved.iter().sum::<f32>() / resolved.len() as f32
}

/// Selected files form a connected-ish subgraph when they share path prefixes or seed edges.
pub fn graph_coherence(view: &ContextView) -> f32 {
    let paths: Vec<String> = view
        .active_nodes
        .iter()
        .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    if paths.len() <= 1 {
        return 1.0;
    }
    let mut connected = 0usize;
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            if share_prefix(&paths[i], &paths[j]) {
                connected += 1;
            }
        }
    }
    let pairs = paths.len() * (paths.len() - 1) / 2;
    if pairs == 0 {
        1.0
    } else {
        (connected as f32 / pairs as f32).clamp(0.0, 1.0)
    }
}

fn share_prefix(a: &str, b: &str) -> bool {
    let a_parts: Vec<&str> = a.split('/').collect();
    let b_parts: Vec<&str> = b.split('/').collect();
    if a_parts.is_empty() || b_parts.is_empty() {
        return false;
    }
    a_parts[0] == b_parts[0]
        || a_parts
            .iter()
            .zip(b_parts.iter())
            .take(2)
            .all(|(x, y)| x == y)
}

fn redundancy_ratio(view: &ContextView) -> f32 {
    let paths: HashSet<String> = view
        .active_nodes
        .iter()
        .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    if paths.is_empty() {
        return 0.0;
    }
    let sidecar = view
        .coverage
        .as_ref()
        .map(|c| c.sidecar_files.len())
        .unwrap_or(0);
    (sidecar as f32 / paths.len() as f32).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::{ContextView, ProjectId, TaskSignature};

    #[test]
    fn no_seeds_is_insufficient() {
        let view = ContextView {
            project_id: ProjectId::new("p"),
            active_nodes: Vec::new(),
            inactive_descriptors: Vec::new(),
            total_raw_tokens: 0,
            active_tokens: 0,
            reduction_percentage: 0.0,
            confidence_score: 0.0,
            bypass_applied: false,
            seeds: Vec::new(),
            unresolved: Vec::new(),
            coverage: None,
            next_actions: Vec::new(),
            budget_used: 0,
            budget_cap: 0,
            budget_mode: String::new(),
            budget_seed_tokens: 0,
            budget_fill_used: 0,
            budget_fill_cap: 0,
            over_budget: false,
            fold_ids: Vec::new(),
            seed_call_coverage: 0.0,
            workspace_tokens: 1000,
            physarum_used: false,
            physarum_ms: 0,
            selection_method: String::new(),
            task_scenario: String::new(),
            rank_candidates: Vec::new(),
            structural_evidence: Vec::new(),
            seed_resolution_telemetry: None,
            packet_header: None,
            retrieval: None,
        };
        let sig = TaskSignature::new("find something");
        let est = SufficiencyEstimator::default().estimate(&view, &sig);
        assert_eq!(est.claim, "insufficient");
        assert!(!est.eligible_for_early_exit);
    }
}
