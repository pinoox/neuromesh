use crate::seed::sink::SeedSink;
use neuromesh_core::{SeedEngineId, SeedResolutionConfig, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;

#[derive(Debug, Clone)]
pub struct SeedEngineResult {
    pub resolved_count: usize,
    pub scaffold_used: bool,
}

pub trait SeedResolutionEngine: Send + Sync {
    fn id(&self) -> SeedEngineId;

    fn resolve(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        prompt: &str,
        config: &SeedResolutionConfig,
        sink: &mut SeedSink<'_, '_, '_>,
        is_style: bool,
    ) -> SeedEngineResult;
}
