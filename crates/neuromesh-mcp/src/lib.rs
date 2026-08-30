pub mod descriptors;
pub mod graph_proxy;
pub mod learning;
pub mod packet_cache;
pub mod protocol;
pub mod response;
#[cfg(feature = "embeddings")]
pub mod semantic_cache;
pub mod server;
pub mod stdio;
pub mod tools;
pub mod uri;

pub use learning::warmup_project_learning;
pub use server::{JsonRpcRequest, McpServer};
pub use stdio::read_message;
pub use tools::McpToolHandler;
