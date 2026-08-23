use crate::{NeuroMeshError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationMode {
    MaxQuality,
    #[default]
    Balanced,
    MaxSavings,
}

impl std::fmt::Display for OptimizationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxQuality => write!(f, "Maximum Quality"),
            Self::Balanced => write!(f, "Balanced"),
            Self::MaxSavings => write!(f, "Maximum Savings"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    #[default]
    OpenAI,
    Anthropic,
    Google,
    OpenRouter,
    Cursor,
    Local,
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: String,
    pub timeout_seconds: u64,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider_type: ProviderType::OpenAI,
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            base_url: Some("https://api.openai.com/v1".into()),
            default_model: "gpt-4o".into(),
            timeout_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAiConfig {
    pub enabled: bool,
    pub model_path: Option<PathBuf>,
    pub model_name: String,
    pub context_size: usize,
    pub threads: usize,
    pub use_gpu: bool,
}

impl Default for LocalAiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model_path: None,
            model_name: "qwen-0.6b-q4".into(),
            context_size: 2048,
            threads: 4,
            use_gpu: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    pub min_activation_score_balanced: f32,
    pub min_activation_score_savings: f32,
    pub max_tokens_budget: usize,
    pub spreading_hops: usize,
    pub pheromone_evaporation_rate: f32,
    pub expansion_confidence_floor: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            min_activation_score_balanced: 0.35,
            min_activation_score_savings: 0.60,
            max_tokens_budget: 1000000,
            spreading_hops: 3,
            pheromone_evaporation_rate: 0.02,
            expansion_confidence_floor: 0.40,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub mode: OptimizationMode,
    pub provider: ProviderConfig,
    pub local_ai: LocalAiConfig,
    pub thresholds: Thresholds,
}

impl Default for Config {
    fn default() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let data_dir = home_dir.join(".neuromesh");

        Self {
            host: "127.0.0.1".into(),
            port: Self::DEFAULT_PORT,
            data_dir,
            mode: OptimizationMode::Balanced,
            provider: ProviderConfig::default(),
            local_ai: LocalAiConfig::default(),
            thresholds: Thresholds::default(),
        }
    }
}

impl Config {
    pub const DEFAULT_PORT: u16 = 8765;

    /// Project `.neuromesh/config.json`, then `~/.neuromesh/config.json`, then defaults.
    /// `NEUROMESH_PORT` wins over files.
    pub fn load() -> Self {
        Self::from_files().with_env_overrides()
    }

    pub fn from_files() -> Self {
        let local = std::env::current_dir()
            .ok()
            .map(|d| d.join(".neuromesh").join("config.json"));
        if let Some(path) = local.as_ref().filter(|p| p.exists()) {
            if let Some(cfg) = Self::read_file(path) {
                return cfg;
            }
        }
        if let Some(path) = dirs::home_dir().map(|h| h.join(".neuromesh").join("config.json")) {
            if path.exists() {
                if let Some(cfg) = Self::read_file(&path) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    fn read_file(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(raw) = std::env::var("NEUROMESH_PORT") {
            if let Ok(port) = parse_port(&raw) {
                self.port = port;
            }
        }
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Write `port` / `host` into `<cwd>/.neuromesh/config.json`.
    /// Merges into the existing project file; never copies `~/.neuromesh` secrets.
    pub fn save_local(&self) -> Result<PathBuf> {
        let dir = std::env::current_dir()?.join(".neuromesh");
        fs::create_dir_all(&dir)?;
        let path = dir.join("config.json");
        let mut merged = if path.exists() {
            Self::read_file(&path).unwrap_or_default()
        } else {
            Self::default()
        };
        merged.port = self.port;
        merged.host = self.host.clone();
        fs::write(&path, serde_json::to_string_pretty(&merged)?)?;
        Ok(path)
    }
}

pub fn parse_port(raw: &str) -> Result<u16> {
    let port: u16 = raw
        .trim()
        .parse()
        .map_err(|_| NeuroMeshError::Config(format!("invalid port: {raw}")))?;
    if port == 0 {
        return Err(NeuroMeshError::Config("port must be 1–65535".into()));
    }
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_monitor_port_is_8765() {
        assert_eq!(Config::default().port, 8765);
    }

    #[test]
    fn parse_port_rejects_zero_and_junk() {
        assert!(parse_port("0").is_err());
        assert!(parse_port("abc").is_err());
        assert_eq!(parse_port("9000").unwrap(), 9000);
    }
}
