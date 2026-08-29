use crate::seed::sink::SeedSink;
use neuromesh_core::SeedResolutionConfig;
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::{is_prompt_stopword, normalize_prompt_tokens};

/// Graph-aware min token length: shorter symbols allowed when graph has few matches.
fn min_token_len(graph: &NeuralProjectGraph) -> usize {
    if graph.search_symbols("", 8).len() > 500 {
        4
    } else {
        3
    }
}

/// Fast substring scan over symbol index when the active engine yields zero seeds.
pub fn lexical_fallback(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let tokens = normalize_prompt_tokens(prompt);
    let lower = prompt.to_lowercase();
    let min_len = min_token_len(graph);
    let mut tried = 0usize;

    for token in &tokens {
        if token.len() < min_len || is_prompt_stopword(token) {
            continue;
        }
        let before = sink.resolved_count();
        sink.push(graph, prompt, token.clone(), 0.5, "fallback:token");
        if sink.resolved_count() > before && sink.resolved_count() >= config.max_resolved_seeds {
            return;
        }
    }

    for hit in graph.search_symbols("", 64) {
        if tried >= config.max_resolved_seeds * 4 {
            break;
        }
        let Some(node) = graph.get_node(&hit.id) else {
            continue;
        };
        let name_l = node.name.to_lowercase();
        if name_l.len() < min_len || is_prompt_stopword(&name_l) {
            continue;
        }
        if !lower.contains(&name_l)
            && !name_l.contains(&lower.chars().take(12).collect::<String>())
            && !tokens
                .iter()
                .any(|t| name_l.contains(t) || t.contains(&name_l))
        {
            continue;
        }
        tried += 1;
        let before = sink.resolved_count();
        sink.push(graph, prompt, node.name.clone(), 0.45, "fallback:lexical");
        if sink.resolved_count() > before && sink.resolved_count() >= config.max_resolved_seeds {
            break;
        }
    }
}
