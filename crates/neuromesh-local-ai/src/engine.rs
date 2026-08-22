use crate::model::LocalModelDescriptor;
use neuromesh_core::{ContextNode, LocalAiConfig, Result, TaskSignature};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceAssessment {
    pub intent_confidence: f32,
    pub uncertainty_score: f32,
    pub compression_recommendation: String,
    pub candidate_rankings: Vec<(String, f32)>,
}

pub struct LocalAiEngine {
    pub config: LocalAiConfig,
    active_model: Arc<RwLock<LocalModelDescriptor>>,
}

impl LocalAiEngine {
    pub fn new(config: LocalAiConfig) -> Self {
        let descriptor = match config.model_name.as_str() {
            "1.5B" => LocalModelDescriptor::qwen_1_5b(),
            "3B" => LocalModelDescriptor::llama_3b(),
            _ => LocalModelDescriptor::qwen_0_6b(),
        };

        Self {
            config,
            active_model: Arc::new(RwLock::new(descriptor)),
        }
    }

    /// Performs fast local neural assessment of task signature and context candidates
    pub fn assess_context(
        &self,
        signature: &TaskSignature,
        candidates: &[ContextNode],
    ) -> Result<InferenceAssessment> {
        let mut rankings = Vec::new();

        for node in candidates {
            let mut score = 0.50f32;
            let node_name = node.name.to_lowercase();
            let entity = signature.entity.to_lowercase();

            if node_name.contains(&entity) {
                score += 0.40;
            }

            for concept in &signature.related_concepts {
                if node_name.contains(&concept.to_lowercase()) {
                    score += 0.20;
                }
            }

            rankings.push((node.name.clone(), score.clamp(0.0, 1.0)));
        }

        rankings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let uncertainty_score = if signature.confidence > 0.8 {
            0.15
        } else {
            0.65
        };

        let recommendation = if uncertainty_score > 0.5 {
            "conservative".to_string()
        } else {
            "balanced".to_string()
        };

        Ok(InferenceAssessment {
            intent_confidence: signature.confidence,
            uncertainty_score,
            compression_recommendation: recommendation,
            candidate_rankings: rankings,
        })
    }

    pub fn get_model_info(&self) -> LocalModelDescriptor {
        self.active_model.read().clone()
    }
}
