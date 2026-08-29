//! Seed pipeline helpers shared by the strategy engines.

use crate::activator::prune_weak_greenfield_seeds_inner;
use crate::seed::ranker::{signal_weight, SignalKind};
use crate::seed::sink::SeedSink;
use neuromesh_core::{SeedResolutionConfig, TaskIntent, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::{is_prompt_stopword, normalize_prompt_tokens};

pub(crate) fn push_anchor_queries(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    for ident in &signature.identifiers {
        if ident.eq_ignore_ascii_case(signature.technology.as_str()) {
            continue;
        }
        sink.push(graph, prompt, ident.clone(), 1.0, "identifier");
    }
    if !signature.entity.is_empty()
        && signature.entity != "Workspace"
        && !signature
            .identifiers
            .iter()
            .any(|id| id == &signature.entity)
        && !matches!(signature.intent, TaskIntent::Create)
    {
        sink.push(graph, prompt, signature.entity.clone(), 1.0, "entity");
    }
    for hint in &signature.file_hints {
        sink.push(graph, prompt, hint.clone(), 0.95, "file");
    }
    for concept in &signature.related_concepts {
        if concept.len() < 4 {
            continue;
        }
        let lower = concept.to_lowercase();
        if lower == "layout" || lower == "breakpoints" || lower == "state" {
            continue;
        }
        if concept.eq_ignore_ascii_case(signature.technology.as_str()) {
            continue;
        }
        sink.push(graph, prompt, concept.clone(), 0.82, "concept");
    }
    if sink.resolved_count() == 0 && sink.resolutions().is_empty() {
        for token in signature.raw_prompt.split_whitespace().take(8) {
            let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if clean.len() < 5 || is_prompt_stopword(clean) {
                continue;
            }
            sink.push(graph, prompt, clean.to_string(), 0.55, "token");
        }
    }
}

pub(crate) fn push_client_keywords(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    if signature.client_keywords.is_empty() {
        return;
    }
    let mut resolved = 0usize;
    for (pos, kw) in signature
        .client_keywords
        .iter()
        .take(config.max_keywords)
        .enumerate()
    {
        if resolved >= config.max_resolved_seeds {
            break;
        }
        if sink.resolutions().iter().any(|s| s.query == *kw) {
            continue;
        }
        let energy = signal_weight(config, SignalKind::Keyword, pos);
        let before = sink.resolved_count();
        sink.push(graph, prompt, kw.clone(), energy, "client_keyword");
        if sink.resolved_count() > before {
            resolved += 1;
        }
    }
}

pub(crate) fn push_client_expansion(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    for (pos, term) in signature
        .client_expansion
        .iter()
        .take(config.max_expansion)
        .enumerate()
    {
        if sink.resolutions().iter().any(|s| s.query == *term) {
            continue;
        }
        let energy = signal_weight(config, SignalKind::Expansion, pos);
        sink.push(graph, prompt, term.clone(), energy, "client_expansion");
    }
}

pub(crate) fn push_path_hint_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    for (pos, hint) in signature.client_path_hints.iter().enumerate() {
        if graph.resolve_file_hint(hint).is_some() {
            let energy = signal_weight(config, SignalKind::PathHint, pos);
            sink.push(graph, prompt, hint.clone(), energy, "path_hint");
        }
    }
    for (pos, et) in signature.client_entity_types.iter().enumerate() {
        let energy = signal_weight(config, SignalKind::EntityType, pos);
        sink.push(graph, prompt, et.clone(), energy, "entity_type");
    }
}

pub(crate) fn token_fallback_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let tokens: Vec<String> =
        if signature.client_keywords.is_empty() && signature.client_expansion.is_empty() {
            signature
                .raw_prompt
                .split_whitespace()
                .take(8)
                .map(|t| {
                    t.trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
                        .to_string()
                })
                .collect()
        } else {
            normalize_prompt_tokens(signature.raw_prompt.as_str())
                .into_iter()
                .take(8)
                .collect()
        };
    for token in tokens {
        if token.len() < 5 || is_prompt_stopword(&token) {
            continue;
        }
        sink.push(graph, prompt, token, 0.55, "token");
    }
}

pub(crate) fn prune_weak_greenfield_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    // Brownfield-safe: only prune fuzzy NL seeds on greenfield Create tasks.
    if !matches!(signature.intent, TaskIntent::Create) {
        return;
    }
    if !signature.client_keywords.is_empty() || !signature.client_expansion.is_empty() {
        return;
    }
    prune_weak_greenfield_seeds_inner(graph, signature, &mut sink.buffers_mut());
}
