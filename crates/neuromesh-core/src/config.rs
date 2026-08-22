use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationMode {
    MaxQuality,
    Balanced,
    MaxSavings,
}

impl Default for OptimizationMode {
    fn default() -> Self {
        Self::Balanced
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
    OpenRouter,
    Cursor,
    Local,
    Mock,
}

impl Default for ProviderType {
    fn default() -> Self {
        Self::OpenAI
    }
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
            port: 8765,
            data_dir,
            mode: OptimizationMode::Balanced,
            provider: ProviderConfig::default(),
            local_ai: LocalAiConfig::default(),
            thresholds: Thresholds::default(),
        }
    }
}
