use crate::seed::sink::SeedBuffers;
use neuromesh_core::SeedResolutionConfig;
use std::collections::HashSet;

/// Trim resolved seeds to config max; drop entries below min score threshold.
pub fn cap_and_rank(buffers: &mut SeedBuffers<'_, '_, '_>, config: &SeedResolutionConfig) {
    if buffers.energies.len() <= config.max_resolved_seeds {
        return;
    }
    let mut ranked: Vec<(neuromesh_core::NodeId, f32)> = buffers
        .energies
        .iter()
        .map(|(id, e)| (id.clone(), *e))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let keep: HashSet<_> = ranked
        .iter()
        .take(config.max_resolved_seeds)
        .filter(|(_, score)| *score >= config.min_seed_score_threshold)
        .map(|(id, _)| id.clone())
        .collect();
    buffers.energies.retain(|id, _| keep.contains(id));
    buffers.reasons.retain(|id, _| keep.contains(id));
    buffers
        .resolutions
        .retain(|s| s.resolved_id.as_ref().is_none_or(|id| keep.contains(id)));
}

/// Weighted score for a candidate match (used when ranking keyword vs expansion).
pub fn signal_weight(config: &SeedResolutionConfig, signal: SignalKind, position: usize) -> f32 {
    let base = match signal {
        SignalKind::Identifier => config.weights.exact_identifier_match,
        SignalKind::Keyword => config.weights.primary_keyword_match,
        SignalKind::Expansion => config.weights.expansion_match,
        SignalKind::PathHint => config.weights.path_hint_bonus,
        SignalKind::EntityType => config.weights.entity_type_bonus,
    };
    let decay = 1.0 / (1.0 + position as f32 * 0.08);
    base * decay
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    Identifier,
    Keyword,
    Expansion,
    PathHint,
    EntityType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_outranks_expansion() {
        let config = SeedResolutionConfig::default();
        assert!(
            signal_weight(&config, SignalKind::Keyword, 0)
                > signal_weight(&config, SignalKind::Expansion, 0)
        );
    }
}
