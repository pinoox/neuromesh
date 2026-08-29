use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyContextPacket {
    pub task: String,
    pub provider: String,
    pub coverage: String,
    pub files: Vec<ProxyContextFile>,
    pub packet_tokens: usize,
    pub symbols_found: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyContextFile {
    pub path: String,
    pub code: String,
    pub tokens: usize,
    pub why: String,
    pub qualified_name: Option<String>,
}
