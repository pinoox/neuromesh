use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Where NeuroMesh sources structural graph data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GraphBackendId {
    /// Built-in index + `NeuralProjectGraph` (default).
    #[default]
    Native,
    /// Prefer an external MCP graph when installed; otherwise native.
    Auto,
    /// Always use codebase-memory-mcp (CBM) via MCP stdio.
    ProxyCbm,
    /// Always use Graphify via MCP stdio (when adapter is available).
    ProxyGraphify,
}

impl GraphBackendId {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "native" | "local" | "builtin" | "built_in" => Some(Self::Native),
            "auto" | "detect" => Some(Self::Auto),
            "proxy_cbm" | "proxy-cbm" | "cbm" | "codebase-memory" | "codebase_memory" => {
                Some(Self::ProxyCbm)
            }
            "proxy_graphify" | "proxy-graphify" | "graphify" => Some(Self::ProxyGraphify),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Auto => "auto",
            Self::ProxyCbm => "proxy_cbm",
            Self::ProxyGraphify => "proxy_graphify",
        }
    }

    pub fn help_line() -> &'static str {
        "native | auto | proxy_cbm | proxy_graphify"
    }

    pub fn uses_proxy(self) -> bool {
        !matches!(self, Self::Native)
    }
}

/// External graph MCP server launch spec (from config or auto-detect).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphProxyLaunchSpec {
    pub provider: GraphProxyProvider,
    pub server_name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphProxyProvider {
    Cbm,
    Graphify,
}

impl GraphProxyProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbm => "cbm",
            Self::Graphify => "graphify",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphProxyConfig {
    pub backend: GraphBackendId,
    /// When proxy fails at runtime, fall back to native graph (recommended).
    pub fallback_native: bool,
    /// Override MCP server block name (auto-detect when unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    /// Manual launch override — skips auto-detect when command is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

impl Default for GraphProxyConfig {
    fn default() -> Self {
        Self {
            backend: GraphBackendId::Native,
            fallback_native: true,
            server_name: None,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
        }
    }
}
