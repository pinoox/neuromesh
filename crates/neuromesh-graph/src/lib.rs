pub mod activation;
pub mod edge;
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
pub use edge::{PheromoneConfig, PheromoneEngine};
pub use graph::{
    path_echoes_symbol, GraphStats, IndexState, NeuralProjectGraph, NodeLearningProfile,
};
pub use node::NodeFactory;
pub use physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
pub use query::{
    ArchitecturePackage, ArchitectureSummary, ImpactResult, NeighborView, SearchHit,
    TraceDirection, TraceHop, TraceResult,
};
pub use synapse::{NeuralSpike, StdpConfig, SynapticPlasticityEngine};
