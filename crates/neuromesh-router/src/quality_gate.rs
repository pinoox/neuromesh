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

        let (effective_mode, allow_optimization, reason) = if signature.requires_conservative_mode()
        {
            (
                OptimizationMode::MaxQuality,
                false,
                "Critical/security task: honor safety over the requested savings mode".to_string(),
            )
        } else {
            (
                requested_mode,
                true,
                format!(
                    "Honoring requested mode {:?} (osmotic recommended {:?})",
                    requested_mode, membrane.recommended_mode
                ),
            )
        };

        QualityGateDecision {
            allow_optimization,
            effective_mode,
            membrane_state: OsmoticMembraneState {
                recommended_mode: effective_mode,
                rationale: reason.clone(),
                ..membrane
            },
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::{OptimizationMode, TaskRisk, TaskSignature};

    #[test]
    fn honors_requested_max_quality() {
        let sig = TaskSignature::new("How does handle_tool_call extract intent?");
        let decision = QualityGate::evaluate(&sig, OptimizationMode::MaxQuality);
        assert_eq!(decision.effective_mode, OptimizationMode::MaxQuality);
    }

    #[test]
    fn critical_task_upgrades_savings() {
        let mut sig = TaskSignature::new("Rotate payment credentials");
        sig.risk = TaskRisk::Critical;
        let decision = QualityGate::evaluate(&sig, OptimizationMode::MaxSavings);
        assert_eq!(decision.effective_mode, OptimizationMode::MaxQuality);
    }
}
