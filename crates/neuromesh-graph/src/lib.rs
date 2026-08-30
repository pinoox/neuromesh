pub mod activation;
pub mod concept_index;
pub mod edge;
pub mod embeddings;
pub mod graph;
mod intern;
pub mod manifest;
pub mod node;
pub mod physarum;
pub mod query;
pub mod synapse;

#[cfg(test)]
mod quality_tests;
#[cfg(test)]
mod repo_quality_tests;

pub use activation::{SpreadingActivation, SpreadingActivationConfig};
pub use concept_index::{ConceptId, ConceptIndex};
pub use edge::{PheromoneConfig, PheromoneEngine};
#[cfg(feature = "embeddings")]
pub use embeddings::{
    graph_digest, maybe_rebuild_embeddings, rebuild_embeddings, rebuild_embeddings_for_workspace,
    refresh_embeddings_after_index,
};
pub use embeddings::{load_sidecar, EmbeddingIndex, EmbeddingSidecar};
pub use graph::{
    node_learning_bonus, path_echoes_symbol, GraphStats, IndexState, NeuralProjectGraph,
    NodeLearningProfile, GRAPH_PARSER_EPOCH,
};
pub use node::NodeFactory;
pub use physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
pub use query::{
    ArchitecturePackage, ArchitectureSummary, ImpactResult, NeighborView, SearchHit,
    TraceDirection, TraceHop, TraceResult,
};
pub use synapse::{NeuralSpike, StdpConfig, SynapticPlasticityEngine};
