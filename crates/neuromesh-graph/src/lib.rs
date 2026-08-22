pub mod activation;
pub mod edge;
pub mod graph;
pub mod node;
pub mod physarum;
pub mod synapse;

pub use activation::{SpreadingActivation, SpreadingActivationConfig};
pub use edge::{PheromoneConfig, PheromoneEngine};
pub use graph::{GraphStats, NeuralProjectGraph};
pub use node::NodeFactory;
pub use physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
pub use synapse::{NeuralSpike, StdpConfig, SynapticPlasticityEngine};
