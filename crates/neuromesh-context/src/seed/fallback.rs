use crate::seed::sink::SeedSink;
use neuromesh_core::SeedResolutionConfig;
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::is_prompt_stopword;

/// Fast substring scan over symbol index when the active engine yields zero seeds.
pub fn lexical_fallback(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let lower = prompt.to_lowercase();
    let mut tried = 0usize;
    for hit in graph.search_symbols("", 64) {
        if tried >= config.max_resolved_seeds * 4 {
            break;
        }
        let Some(node) = graph.get_node(&hit.id) else {
            continue;
        };
        let name_l = node.name.to_lowercase();
        if name_l.len() < 4 || is_prompt_stopword(&name_l) {
            continue;
        }
        if !lower.contains(&name_l) && !name_l.contains(&lower.chars().take(12).collect::<String>())
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
