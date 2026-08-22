use chrono::Utc;
use neuromesh_core::{ContextNode, NodeType, TaskSignature};

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

        // Exact match with task entity
        if node_name_lower == entity_lower || node_name_lower.contains(&entity_lower) {
            return 1.0;
        }

        // Match with related concepts
        for concept in &signature.related_concepts {
            let concept_lower = concept.to_lowercase();
            if node_name_lower.contains(&concept_lower) || concept_lower.contains(&node_name_lower)
            {
                return 0.85;
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
        match node.node_type {
            NodeType::Component => 0.95,
            NodeType::File => 0.90,
            NodeType::Function | NodeType::Class => 0.85,
            NodeType::StyleToken => 0.80,
            NodeType::Symbol => 0.75,
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
        }
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
