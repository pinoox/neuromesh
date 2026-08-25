pub mod descriptors;
pub mod protocol;
pub mod server;
pub mod stdio;
pub mod tools;
pub mod uri;

pub use server::{JsonRpcRequest, McpServer};
pub use stdio::read_message;
pub use tools::McpToolHandler;
