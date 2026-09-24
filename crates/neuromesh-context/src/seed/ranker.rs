use crate::seed::sink::SeedBuffers;
use neuromesh_core::SeedResolutionConfig;
use std::collections::HashSet;

/// Trim resolved seeds to config max; drop entries below min score threshold.
pub fn cap_and_rank(buffers: &mut SeedBuffers<'_, '_, '_>, config: &SeedResolutionConfig) {
    if buffers.energies.len() <= config.max_resolved_seeds {
        return;
    }
    // Ties on energy are common (every artifact seed of one kind scores the
    // same) and `energies` is a hash map, so the tie-break has to come from
    // somewhere stable: the order the seeds were resolved in. Prompt
    // identifiers come before artifact seeds, and artifact kinds come in
    // vocabulary order, so "first resolved" is also the more deliberate seed.
    let first_seen = |id: &neuromesh_core::NodeId| {
        buffers
            .resolutions
            .iter()
            .position(|s| s.resolved_id.as_ref() == Some(id))
            .unwrap_or(usize::MAX)
    };
    let mut ranked: Vec<(neuromesh_core::NodeId, f32, usize)> = buffers
        .energies
        .iter()
        .map(|(id, e)| (id.clone(), *e, first_seen(id)))
        .collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.0.as_str().cmp(b.0.as_str()))
    });
    let keep: HashSet<_> = ranked
        .iter()
        .take(config.max_resolved_seeds)
        .filter(|(_, score, _)| *score >= config.min_seed_score_threshold)
        .map(|(id, _, _)| id.clone())
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
        SignalKind::SemanticEmbed => config.weights.semantic_embed_match,
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
    SemanticEmbed,
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

#[cfg(test)]
mod cap_tests {
    use super::*;
    use neuromesh_core::{NodeId, SeedResolution};
    use std::collections::HashMap;

    fn resolution(query: &str, id: &str, energy: f32) -> SeedResolution {
        SeedResolution {
            query: query.into(),
            resolved_id: Some(NodeId::new(id)),
            confidence: energy,
            resolution_tier: None,
            embedding_score: None,
        }
    }

    /// Eight seeds at the same energy, a cap of five: the survivors must be
    /// the five resolved first, on every run. Before the tie-break the choice
    /// followed the energy map's iteration order, which is reseeded per map,
    /// so a real ML question (nanoGPT, eight artifact seeds at 0.86) shipped a
    /// different seed set — and a different packet — between two processes.
    #[test]
    fn cap_keeps_the_first_resolved_seeds_on_a_tie() {
        let config = SeedResolutionConfig {
            max_resolved_seeds: 5,
            ..SeedResolutionConfig::default()
        };
        let ids: Vec<String> = (0..8).map(|i| format!("sym:src/m.py:seed_{i}")).collect();
        let expected: Vec<&String> = ids.iter().take(5).collect();
        for _round in 0..25 {
            let mut resolutions: Vec<SeedResolution> = ids
                .iter()
                .enumerate()
                .map(|(i, id)| resolution(&format!("q{i}"), id, 0.86))
                .collect();
            let mut energies: HashMap<NodeId, f32> =
                ids.iter().map(|id| (NodeId::new(id), 0.86)).collect();
            let mut reasons: HashMap<NodeId, String> = ids
                .iter()
                .map(|id| (NodeId::new(id), "artifact".into()))
                .collect();
            let mut buffers = SeedBuffers {
                resolutions: &mut resolutions,
                energies: &mut energies,
                reasons: &mut reasons,
            };
            cap_and_rank(&mut buffers, &config);
            let mut kept: Vec<String> = energies.keys().map(|id| id.to_string()).collect();
            kept.sort();
            let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            want.sort();
            assert_eq!(kept, want, "cap kept a different seed set");
            assert_eq!(resolutions.len(), 5);
        }
    }
}
