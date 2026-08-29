use neuromesh_core::{SeedEngineId, SeedResolutionConfig, SeedResolutionTelemetry, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use std::time::Instant;

pub mod engine;
pub mod engines;
pub mod fallback;
pub mod manifest;
pub mod micro_header;
pub mod ranker;
pub mod sink;

#[cfg(test)]
mod tests;

pub use manifest::NearestAncestorManifestResolver;
pub use micro_header::MicroHeaderGenerator;
pub use sink::{ResolveSeedFn, SeedBuffers, SeedSink};

#[derive(Debug, Clone)]
pub struct SeedRunResult {
    pub scaffold_used: bool,
    pub telemetry: SeedResolutionTelemetry,
    pub monorepo_packages: Vec<String>,
    pub packet_header: Option<String>,
}

pub fn resolve_engine_id(signature: &TaskSignature, config: &SeedResolutionConfig) -> SeedEngineId {
    signature.engine_override.unwrap_or(config.engine)
}

pub fn run_seed_resolution(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    buffers: &mut SeedBuffers<'_, '_, '_>,
    resolve: ResolveSeedFn,
    is_style: bool,
) -> SeedRunResult {
    let started = Instant::now();
    let engine = resolve_engine_id(signature, config);

    let scaffold_used = {
        let mut sink = SeedSink::new(
            buffers.resolutions,
            buffers.energies,
            buffers.reasons,
            resolve,
        );
        let used = engines::dispatch(
            engine, graph, signature, prompt, config, &mut sink, is_style,
        );

        if sink.resolved_count() == 0 {
            eprintln!(
                "[neuromesh] seed engine {:?} yielded zero seeds; running lexical fallback",
                engine
            );
            fallback::lexical_fallback(graph, prompt, config, &mut sink);
        }
        used
    };

    ranker::cap_and_rank(buffers, config);

    let seed_paths: Vec<String> = buffers
        .energies
        .keys()
        .filter_map(|id| graph.get_node(id))
        .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
        .collect();

    let mut manifest = NearestAncestorManifestResolver::new(graph);
    let monorepo_packages = manifest.packages_for_paths(&seed_paths);

    let telemetry = SeedResolutionTelemetry {
        engine: engine.as_str().to_string(),
        seeds_count: buffers.energies.len(),
        monorepo_packages: monorepo_packages.clone(),
        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
    };

    SeedRunResult {
        scaffold_used,
        telemetry,
        monorepo_packages,
        packet_header: None,
    }
}
