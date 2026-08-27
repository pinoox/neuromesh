use chrono::Utc;
use neuromesh_core::{
    is_bench_path, is_json_schema_path, is_legacy_path, is_locale_path, name_match_specificity,
    prompt_targets_bench, prompt_targets_json_schema, prompt_targets_legacy, prompt_targets_locale,
    prompt_targets_types, ContextNode, NodeType, TaskSignature,
};

#[derive(Debug, Clone)]
pub struct ScoringWeights {
    pub relevance_weight: f32,
    pub impact_weight: f32,
    pub recency_half_life_days: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            relevance_weight: 1.0,
            impact_weight: 1.0,
            recency_half_life_days: 7.0,
        }
    }
}

pub struct ActivationScorer {
    weights: ScoringWeights,
}

impl ActivationScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    /// Calculates multi-factor activation score:
    /// Score = Relevance * Confidence * TaskImpact * Recency * RelationshipStrength * HistoricalSuccess
    pub fn score_node(
        &self,
        node: &ContextNode,
        signature: &TaskSignature,
        relationship_strength: f32,
        historical_success: f32,
    ) -> f32 {
        // 1. Relevance Factor R(n, T)
        let relevance = self.compute_relevance(node, signature);

        // 2. Confidence C(T)
        let confidence = signature.confidence;

        // 3. Task Impact Factor I(n, T)
        let task_impact = self.compute_task_impact(node, signature);

        // 4. Recency Decay rho(n)
        let recency = self.compute_recency(node);

        // 5. Clamped relationship and historical factors
        let rel_strength = relationship_strength.clamp(0.05, 1.0);
        let hist_success = historical_success.clamp(0.20, 1.0);

        let final_score =
            relevance * confidence * task_impact * recency * rel_strength * hist_success;
        final_score.clamp(0.0, 1.0)
    }

    fn compute_relevance(&self, node: &ContextNode, signature: &TaskSignature) -> f32 {
        let node_name_lower = node.name.to_lowercase();
        let entity_lower = signature.entity.to_lowercase();
        let path_lower = node
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();

        let mut best = 0.30f32;
        for ident in &signature.identifiers {
            let ident_lower = ident.to_lowercase();
            if node_name_lower == ident_lower {
                return 1.0;
            }
            if ident_lower.len() >= 4 && node_name_lower.contains(&ident_lower) {
                let score = 0.92 * name_match_specificity(&ident_lower, &node_name_lower);
                best = best.max(score);
            }
        }
        if best > 0.30 {
            return best;
        }

        for hint in &signature.file_hints {
            let hint_lower = hint.replace('\\', "/").to_lowercase();
            if path_lower.ends_with(&hint_lower) || path_lower.contains(&hint_lower) {
                return 0.96;
            }
        }

        // Exact match with task entity
        if !entity_lower.is_empty() && entity_lower != "workspace" {
            if node_name_lower == entity_lower {
                return 1.0;
            }
            if entity_lower.len() >= 4 && node_name_lower.contains(&entity_lower) {
                return 0.92 * name_match_specificity(&entity_lower, &node_name_lower);
            }
        }

        // Match with related concepts (identifier-sized only)
        for concept in &signature.related_concepts {
            let concept_lower = concept.to_lowercase();
            if concept_lower.len() < 4 {
                continue;
            }
            if node_name_lower == concept_lower {
                return 0.85;
            }
            if node_name_lower.contains(&concept_lower) {
                return 0.85 * name_match_specificity(&concept_lower, &node_name_lower);
            }
        }

        // Technology / style match (e.g. SCSS file when style is SCSS)
        if let Some(style) = &signature.style {
            if style.to_lowercase() == "scss" && node.file_path.to_string_lossy().ends_with(".scss")
            {
                return 0.80;
            }
        }

        // Fallback base relevance
        0.30
    }

    fn compute_task_impact(&self, node: &ContextNode, signature: &TaskSignature) -> f32 {
        let base: f32 = match node.node_type {
            NodeType::Component | NodeType::Api => 0.95,
            NodeType::File => 0.90,
            NodeType::Function | NodeType::Class => 0.85,
            NodeType::StyleToken => 0.80,
            NodeType::Symbol => {
                if prompt_targets_types(signature.raw_prompt.as_str()) {
                    0.88
                } else {
                    0.75
                }
            }
            NodeType::Import => 0.70,
            NodeType::Config => 0.65,
            NodeType::Test => {
                if signature.intent == neuromesh_core::TaskIntent::Test {
                    1.0
                } else {
                    0.35
                }
            }
            NodeType::Doc => 0.40,
            _ => 0.50,
        };
        let prompt = signature.raw_prompt.as_str();
        if is_bench_path(&node.file_path)
            && !prompt_targets_bench(prompt)
            && signature.intent != neuromesh_core::TaskIntent::Optimize
        {
            return base.min(0.35);
        }
        if is_legacy_path(&node.file_path) && !prompt_targets_legacy(prompt) {
            return base.min(0.35);
        }
        if is_json_schema_path(&node.file_path) && !prompt_targets_json_schema(prompt) {
            return base.min(0.45);
        }
        if is_locale_path(&node.file_path) && !prompt_targets_locale(prompt) {
            return base.min(0.30);
        }
        base
    }

    fn compute_recency(&self, node: &ContextNode) -> f32 {
        let now = Utc::now();
        let age_seconds = (now - node.last_accessed).num_seconds().max(0) as f32;
        let half_life_seconds = self.weights.recency_half_life_days * 86400.0;

        // Exponential decay: e^(-lambda * t)
        (-0.693 * (age_seconds / half_life_seconds))
            .exp()
            .clamp(0.2, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::{NodeId, ProjectId, TaskIntent};
    use std::path::PathBuf;

    fn fn_node(name: &str, path: &str) -> ContextNode {
        ContextNode {
            id: NodeId::new(name),
            project_id: ProjectId::new("t"),
            file_path: PathBuf::from(path),
            node_type: NodeType::Function,
            name: name.into(),
            signature: None,
            line_range: Some(1..10),
            token_cost: 40,
            content: None,
            content_hash: String::new(),
            parent: None,
            base_relevance: 1.0,
            access_count: 0,
            last_accessed: Utc::now(),
        }
    }

    fn sig_with_ident(ident: &str) -> TaskSignature {
        let mut sig = TaskSignature::new("how does parse report a validation error path");
        sig.identifiers = vec![ident.into()];
        sig.intent = TaskIntent::Explain;
        sig
    }

    #[test]
    fn tighter_name_match_outranks_decorated_substring() {
        let scorer = ActivationScorer::new(ScoringWeights::default());
        let sig = sig_with_ident("parse");
        let safe = fn_node("safeParse", "packages/zod/src/v4/core/parse.ts");
        let nested = fn_node(
            "parseNestedObject",
            "packages/bench/compile-object-build.ts",
        );
        let safe_rel = scorer.compute_relevance(&safe, &sig);
        let nested_rel = scorer.compute_relevance(&nested, &sig);
        assert!(
            safe_rel > nested_rel,
            "safeParse relevance {safe_rel} should beat parseNestedObject {nested_rel}"
        );
    }

    #[test]
    fn bench_and_locale_paths_are_penalized_for_non_bench_tasks() {
        let scorer = ActivationScorer::new(ScoringWeights::default());
        let sig = sig_with_ident("safeParse");
        let prod = fn_node("safeParse", "packages/zod/src/v4/core/parse.ts");
        let bench = fn_node("safeParse", "packages/bench/safeparse.ts");
        let locale = fn_node("localeError", "packages/zod/src/v4/locales/fa.ts");
        let v3 = fn_node("safeParse", "packages/zod/src/v3/types.ts");
        let json = fn_node("parse", "packages/zod/src/v4/core/to-json-schema.ts");
        let prod_impact = scorer.compute_task_impact(&prod, &sig);
        let bench_impact = scorer.compute_task_impact(&bench, &sig);
        let locale_impact = scorer.compute_task_impact(&locale, &sig);
        let v3_impact = scorer.compute_task_impact(&v3, &sig);
        let json_impact = scorer.compute_task_impact(&json, &sig);
        assert!(
            bench_impact < prod_impact,
            "bench impact {bench_impact} should be below production {prod_impact}"
        );
        assert!(
            locale_impact < prod_impact,
            "locale impact {locale_impact} should be below production {prod_impact}"
        );
        assert!(
            v3_impact < prod_impact,
            "v3 impact {v3_impact} should be below production {prod_impact}"
        );
        assert!(
            json_impact < prod_impact,
            "json-schema impact {json_impact} should be below production {prod_impact}"
        );
        assert!(bench_impact <= 0.35);
    }
}
