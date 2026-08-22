use neuromesh_core::{OptimizationMode, TaskIntent, TaskRisk, TaskSignature};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembranePermeability {
    /// Hyper-impermeable: strictly targeted sliced exons only (Max Savings: 90-95% reduction)
    HyperImpermeable,
    /// Semi-permeable: active exons + 1-hop structural connectors (Balanced: 75-85% reduction)
    SemiPermeable,
    /// Fully permeable: deep context + safety invariants (Max Quality / Conservative)
    FullyPermeable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmoticMembraneState {
    pub internal_osmotic_pressure: f32,
    pub external_osmotic_pressure: f32,
    pub net_membrane_gradient: f32,
    pub permeability: MembranePermeability,
    pub recommended_mode: OptimizationMode,
    pub rationale: String,
}

/// Cellular Membrane Osmotic Quality Gate
pub struct OsmoticQualityGate;

impl OsmoticQualityGate {
    /// Evaluates osmotic pressure and determines membrane permeability
    pub fn regulate_membrane(signature: &TaskSignature, requested_mode: OptimizationMode) -> OsmoticMembraneState {
        // 1. Calculate Internal Osmotic Pressure (Complexity & Risk)
        let risk_factor = match signature.risk {
            TaskRisk::Critical => 1.0,
            TaskRisk::High => 0.75,
            TaskRisk::Medium => 0.45,
            TaskRisk::Low => 0.15,
        };

        let domain_lower = signature.domain.to_lowercase();
        let domain_factor = if domain_lower.contains("security") || domain_lower.contains("auth") || domain_lower.contains("database") {
            0.85
        } else if domain_lower.contains("backend") || domain_lower.contains("devops") {
            0.65
        } else if domain_lower.contains("frontend") || domain_lower.contains("ui") {
            0.35
        } else {
            0.45
        };

        let intent_factor = match signature.intent {
            TaskIntent::Refactor | TaskIntent::Fix => 0.70,
            TaskIntent::Create | TaskIntent::Modify => 0.50,
            TaskIntent::Optimize => 0.60,
            TaskIntent::Test | TaskIntent::Explain | TaskIntent::Query => 0.30,
        };

        let internal_pressure = (risk_factor * 0.45 + domain_factor * 0.30 + intent_factor * 0.25)
            * (1.1 - signature.confidence * 0.2);

        // 2. External Osmotic Pressure from Mode
        let external_pressure = match requested_mode {
            OptimizationMode::MaxQuality => 0.80,
            OptimizationMode::Balanced => 0.50,
            OptimizationMode::MaxSavings => 0.20,
        };

        let net_gradient = internal_pressure - external_pressure;

        // 3. Determine Membrane Permeability
        let (permeability, recommended_mode, rationale) = if signature.requires_conservative_mode() || internal_pressure > 0.75 {
            (
                MembranePermeability::FullyPermeable,
                OptimizationMode::MaxQuality,
                "High osmotic pressure (critical risk / complex domain) opened membrane for full structural assurance".into(),
            )
        } else if internal_pressure < 0.35 && requested_mode == OptimizationMode::MaxSavings {
            (
                MembranePermeability::HyperImpermeable,
                OptimizationMode::MaxSavings,
                "Low osmotic pressure (low risk UI/style task) enabled hyper-impermeable slicing for maximum token reduction".into(),
            )
        } else {
            (
                MembranePermeability::SemiPermeable,
                OptimizationMode::Balanced,
                "Isotonic osmotic equilibrium maintained balanced context permeability".into(),
            )
        };

        OsmoticMembraneState {
            internal_osmotic_pressure: internal_pressure,
            external_osmotic_pressure: external_pressure,
            net_membrane_gradient: net_gradient,
            permeability,
            recommended_mode,
            rationale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osmotic_membrane_regulation() {
        let mut sig = TaskSignature::new("Change button color in Header.vue");
        sig.risk = TaskRisk::Low;
        sig.domain = "frontend".into();
        sig.intent = TaskIntent::Modify;
        sig.confidence = 0.95;

        let state = OsmoticQualityGate::regulate_membrane(&sig, OptimizationMode::MaxSavings);
        assert_eq!(state.permeability, MembranePermeability::HyperImpermeable);
        assert_eq!(state.recommended_mode, OptimizationMode::MaxSavings);

        let mut crit_sig = TaskSignature::new("Migrate payment authentication tokens");
        crit_sig.risk = TaskRisk::Critical;
        crit_sig.domain = "security".into();

        let crit_state = OsmoticQualityGate::regulate_membrane(&crit_sig, OptimizationMode::MaxSavings);
        assert_eq!(crit_state.permeability, MembranePermeability::FullyPermeable);
        assert_eq!(crit_state.recommended_mode, OptimizationMode::MaxQuality);
    }
}
