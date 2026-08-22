use crate::osmotic_gate::{OsmoticMembraneState, OsmoticQualityGate};
use neuromesh_core::{OptimizationMode, TaskSignature};

#[derive(Debug, Clone)]
pub struct QualityGateDecision {
    pub allow_optimization: bool,
    pub effective_mode: OptimizationMode,
    pub membrane_state: OsmoticMembraneState,
    pub reason: String,
}

pub struct QualityGate;

impl QualityGate {
    pub fn evaluate(
        signature: &TaskSignature,
        requested_mode: OptimizationMode,
    ) -> QualityGateDecision {
        let membrane = OsmoticQualityGate::regulate_membrane(signature, requested_mode);

        // Low Confidence (< 0.50) -> Bypass Optimization
        if signature.confidence < 0.50 {
            return QualityGateDecision {
                allow_optimization: false,
                effective_mode: OptimizationMode::MaxQuality,
                membrane_state: membrane,
                reason: format!(
                    "Low task understanding confidence ({:.2} < 0.50) triggered optimization bypass",
                    signature.confidence
                ),
            };
        }

        QualityGateDecision {
            allow_optimization: true,
            effective_mode: membrane.recommended_mode,
            reason: membrane.rationale.clone(),
            membrane_state: membrane,
        }
    }
}
