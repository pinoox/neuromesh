use crate::activator::seed_uncovered_clusters_inner;
use crate::activator_seed::{
    prune_weak_greenfield_seeds, push_anchor_queries, push_client_expansion, push_client_keywords,
    push_path_hint_seeds, token_fallback_seeds,
};
use crate::scaffold_routing::inject_scaffold_seeds;
use crate::seed::sink::SeedSink;
use neuromesh_core::{SeedEngineId, SeedResolutionConfig, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;

pub fn dispatch(
    engine: SeedEngineId,
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
    is_style: bool,
) -> bool {
    match engine {
        SeedEngineId::Off => run_off(graph, signature, prompt, sink),
        SeedEngineId::Keywords => run_keywords(graph, signature, prompt, config, sink),
        SeedEngineId::KeywordsExpanded => {
            run_keywords_expanded(graph, signature, prompt, config, sink)
        }
        SeedEngineId::SemanticLite => {
            run_semantic_lite(graph, signature, prompt, config, sink, is_style)
        }
        SeedEngineId::Hybrid => run_hybrid(graph, signature, prompt, config, sink, is_style),
    }
}

fn run_off(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    sink: &mut SeedSink<'_, '_, '_>,
) -> bool {
    push_anchor_queries(graph, signature, prompt, sink);
    seed_uncovered_clusters_inner(graph, signature, &mut sink.buffers_mut());
    false
}

fn run_keywords(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) -> bool {
    push_anchor_queries(graph, signature, prompt, sink);
    seed_uncovered_clusters_inner(graph, signature, &mut sink.buffers_mut());
    push_client_keywords(graph, signature, prompt, config, sink);
    false
}

fn run_keywords_expanded(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) -> bool {
    push_anchor_queries(graph, signature, prompt, sink);
    seed_uncovered_clusters_inner(graph, signature, &mut sink.buffers_mut());
    push_client_keywords(graph, signature, prompt, config, sink);
    push_client_expansion(graph, signature, prompt, config, sink);
    push_path_hint_seeds(graph, signature, prompt, config, sink);
    false
}

fn run_semantic_lite(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    _config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
    is_style: bool,
) -> bool {
    push_anchor_queries(graph, signature, prompt, sink);
    seed_uncovered_clusters_inner(graph, signature, &mut sink.buffers_mut());
    if sink.resolved_count() == 0 && !is_style {
        token_fallback_seeds(graph, signature, prompt, sink);
    }
    prune_weak_greenfield_seeds(graph, signature, sink);
    if sink.resolved_count() == 0 {
        inject_scaffold_seeds(graph, prompt, signature, sink)
    } else {
        false
    }
}

fn run_hybrid(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
    is_style: bool,
) -> bool {
    push_anchor_queries(graph, signature, prompt, sink);
    seed_uncovered_clusters_inner(graph, signature, &mut sink.buffers_mut());
    push_client_keywords(graph, signature, prompt, config, sink);
    push_client_expansion(graph, signature, prompt, config, sink);
    push_path_hint_seeds(graph, signature, prompt, config, sink);
    if sink.resolved_count() == 0 && !is_style {
        token_fallback_seeds(graph, signature, prompt, sink);
    }
    prune_weak_greenfield_seeds(graph, signature, sink);
    if sink.resolved_count() == 0 {
        inject_scaffold_seeds(graph, prompt, signature, sink)
    } else {
        false
    }
}
