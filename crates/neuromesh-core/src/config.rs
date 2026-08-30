use crate::{
    EmbeddingConfig, GraphBackendId, GraphProxyConfig, NeuroMeshError, NmConfigOverlay,
    PacketHeaderConfig, Result, RetrievalConfig, RetrievalEngine, SeedResolutionConfig,
};
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
    /// Files with min symbol `base_relevance` below this leave optional/required sets.
    #[serde(default = "default_penalized_suppression")]
    pub penalized_suppression_threshold: f32,
    /// Scale learned influence when file does not match query focus terms.
    #[serde(default = "default_learning_unrelated_cap")]
    pub learning_relevance_cap_unrelated: f32,
    /// Half-life for temporal decay of learned influence (days).
    #[serde(default = "default_learning_decay_half_life")]
    pub learning_decay_half_life_days: f32,
    /// Hard cap on learned score component in unified ranking.
    #[serde(default = "default_max_learned_influence")]
    pub max_learned_influence: f32,
    /// Min learning bonus before a reinforced file can be injected into emission.
    #[serde(default = "default_learning_promotion_min_bonus")]
    pub learning_promotion_min_bonus: f32,
}

fn default_penalized_suppression() -> f32 {
    0.55
}
fn default_learning_unrelated_cap() -> f32 {
    0.35
}
fn default_learning_decay_half_life() -> f32 {
    30.0
}
fn default_max_learned_influence() -> f32 {
    48.0
}
fn default_learning_promotion_min_bonus() -> f32 {
    14.0
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
            penalized_suppression_threshold: default_penalized_suppression(),
            learning_relevance_cap_unrelated: default_learning_unrelated_cap(),
            learning_decay_half_life_days: default_learning_decay_half_life(),
            max_learned_influence: default_max_learned_influence(),
            learning_promotion_min_bonus: default_learning_promotion_min_bonus(),
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
    /// Explicit index file cap. `None` (default) auto-grows to fit production sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files: Option<usize>,
    /// Default `managed`: per-project data under `~/.neuromesh/projects/`.
    /// `local` writes `<workspace>/.neuromesh` for every repo.
    #[serde(default)]
    pub project_store: crate::paths::ProjectStore,
    /// Canonical workspace paths allowed to use `<workspace>/.neuromesh`
    /// while `project_store` stays `managed`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trust_local: Vec<String>,
    #[serde(default)]
    pub seed_resolution: SeedResolutionConfig,
    #[serde(default)]
    pub packet_header: PacketHeaderConfig,
    #[serde(default)]
    pub graph_backend: GraphProxyConfig,
    #[serde(default)]
    pub embeddings: EmbeddingConfig,
    #[serde(default)]
    pub retrieval: RetrievalConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: Self::DEFAULT_PORT,
            data_dir: crate::paths::neuromesh_home(),
            mode: OptimizationMode::Balanced,
            provider: ProviderConfig::default(),
            local_ai: LocalAiConfig::default(),
            thresholds: Thresholds::default(),
            max_files: None,
            project_store: crate::paths::ProjectStore::Managed,
            trust_local: Vec::new(),
            seed_resolution: SeedResolutionConfig::default(),
            packet_header: PacketHeaderConfig::default(),
            graph_backend: GraphProxyConfig::default(),
            embeddings: EmbeddingConfig::default(),
            retrieval: RetrievalConfig::default(),
        }
    }
}

impl Config {
    pub const DEFAULT_PORT: u16 = 8765;

    /// Home `config.json`, then the resolved project slot (managed or trusted local).
    /// Workspace `.neuromesh/config.json` is ignored unless the workspace is trusted.
    /// `NEUROMESH_PORT`, `NEUROMESH_MAX_FILES`, and `NEUROMESH_STORE` win over files.
    pub fn load() -> Self {
        Self::from_files().with_env_overrides().normalized()
    }

    pub fn from_files() -> Self {
        let home = crate::paths::neuromesh_home();
        let mut cfg = Self::default();
        let home_cfg = home.join("config.json");
        if home_cfg.exists() {
            if let Some(loaded) = Self::read_file(&home_cfg) {
                cfg = loaded;
            }
        }
        cfg.data_dir = home;
        if let Ok(raw) = std::env::var("NEUROMESH_STORE") {
            if let Ok(store) = crate::paths::ProjectStore::parse(&raw) {
                cfg.project_store = store;
            }
        }
        if let Ok(ws) = std::env::current_dir() {
            let _ = crate::paths::ensure_project_data_dir(&ws);
            let project_cfg = crate::paths::project_config_path(&ws);
            if project_cfg.exists() {
                if let Some(over) = Self::read_file(&project_cfg) {
                    cfg.overlay_project(over);
                }
            }
            if let Some(overlay) = Self::read_nm_config(&ws) {
                cfg.overlay_nm(overlay);
            }
        }
        cfg
    }

    /// Workspace-root `nm.config.json` — merges retrieval, packet_header, graph_backend.
    fn read_nm_config(workspace: &Path) -> Option<NmConfigOverlay> {
        let path = workspace.join("nm.config.json");
        if !path.exists() {
            return None;
        }
        let raw = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn overlay_nm(&mut self, overlay: NmConfigOverlay) {
        if let Some(ph) = overlay.packet_header {
            self.packet_header = ph;
        }
        if let Some(gb) = overlay.graph_backend {
            self.graph_backend = gb;
        }
        if let Some(re) = overlay.retrieval {
            self.retrieval = re;
        }
    }

    fn overlay_project(&mut self, other: Self) {
        self.host = other.host;
        self.port = other.port;
        self.max_files = other.max_files;
        self.mode = other.mode;
        self.provider = other.provider;
        self.local_ai = other.local_ai;
        self.thresholds = other.thresholds;
        self.seed_resolution = other.seed_resolution;
        self.packet_header = other.packet_header;
        self.graph_backend = other.graph_backend;
        self.embeddings = other.embeddings;
        self.retrieval = other.retrieval;
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
        if let Ok(raw) = std::env::var("NEUROMESH_MAX_FILES") {
            match parse_max_files(&raw) {
                Ok(None) => self.max_files = None,
                Ok(Some(n)) => self.max_files = Some(n),
                Err(_) => {}
            }
        }
        if let Ok(raw) = std::env::var("NEUROMESH_GRAPH_BACKEND") {
            if let Some(backend) = GraphBackendId::parse(&raw) {
                self.graph_backend.backend = backend;
            }
        }
        if let Ok(raw) = std::env::var("NEUROMESH_ENGINE") {
            if let Some(engine) = RetrievalEngine::parse(&raw) {
                self.retrieval.engine = engine;
            }
        }
        if let Ok(raw) = std::env::var("NEUROMESH_EMBED_MODEL") {
            if let Some(model) = crate::EmbeddingModelId::parse(&raw) {
                self.embeddings.model = model;
            }
        }
        if let Ok(raw) = std::env::var("NEUROMESH_EMBED_THREADS") {
            if let Ok(n) = raw.trim().parse::<usize>() {
                self.embeddings.intra_threads = if n == 0 { None } else { Some(n) };
            }
        }
        if let Ok(raw) = std::env::var("NEUROMESH_SEMANTIC_CACHE") {
            self.embeddings.semantic_cache_enabled = Self::parse_env_bool(&raw, true);
        }
        if let Ok(raw) = std::env::var("NEUROMESH_OPTIONAL_DEDUP") {
            let t = raw.trim().to_lowercase();
            self.embeddings.optional_dedup_min_cosine = if t == "0" || t == "off" || t == "false" {
                None
            } else if let Ok(f) = t.parse::<f32>() {
                Some(f)
            } else {
                self.embeddings.optional_dedup_min_cosine
            };
        }
        self.embeddings = self.embeddings.clone().normalized();
        self
    }

    /// Apply the unified retrieval engine preset to seed/embeddings/mode.
    pub fn apply_retrieval_preset(&mut self) {
        self.retrieval.engine.apply_preset(
            &mut self.mode,
            &mut self.seed_resolution,
            &mut self.embeddings,
        );
    }

    /// Load overlays/env, then apply retrieval preset.
    pub fn normalized(mut self) -> Self {
        self.apply_retrieval_preset();
        self
    }

    fn parse_env_bool(raw: &str, default: bool) -> bool {
        match raw.trim().to_lowercase().as_str() {
            "0" | "false" | "no" | "off" => false,
            "1" | "true" | "yes" | "on" => true,
            _ => default,
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_max_files(mut self, max_files: Option<usize>) -> Self {
        self.max_files = max_files;
        self
    }

    /// Write `port` / `host` / `max_files` into the resolved project data dir.
    /// Managed default: `~/.neuromesh/projects/<id>/config.json`.
    /// Trusted local: `<cwd>/.neuromesh/config.json`.
    pub fn save_local(&self) -> Result<PathBuf> {
        let ws = std::env::current_dir()?;
        let dir = crate::paths::ensure_project_data_dir(&ws)?;
        let path = dir.join("config.json");
        let mut merged = if path.exists() {
            Self::read_file(&path).unwrap_or_default()
        } else {
            Self::default()
        };
        merged.port = self.port;
        merged.host = self.host.clone();
        merged.max_files = self.max_files;
        merged.seed_resolution = self.seed_resolution.clone();
        merged.packet_header = self.packet_header.clone();
        merged.embeddings = self.embeddings.clone();
        merged.graph_backend = self.graph_backend.clone();
        merged.retrieval = self.retrieval.clone();
        fs::write(&path, serde_json::to_string_pretty(&merged)?)?;
        Ok(path)
    }

    pub fn home_config_path() -> PathBuf {
        crate::paths::neuromesh_home().join("config.json")
    }

    pub fn workspace_nm_config_path(workspace: &Path) -> PathBuf {
        workspace.join("nm.config.json")
    }

    pub fn set_global_graph_backend(backend: GraphBackendId) -> Result<PathBuf> {
        let path = Self::home_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut cfg = if path.exists() {
            Self::read_file(&path).unwrap_or_default()
        } else {
            Self::default()
        };
        cfg.graph_backend.backend = backend;
        fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
        Ok(path)
    }

    pub fn set_workspace_graph_backend(
        workspace: &Path,
        backend: GraphBackendId,
    ) -> Result<PathBuf> {
        let path = Self::workspace_nm_config_path(workspace);
        let mut overlay = Self::read_nm_config(workspace).unwrap_or_default();
        let mut gb = overlay.graph_backend.take().unwrap_or_default();
        gb.backend = backend;
        overlay.graph_backend = Some(gb);
        fs::write(&path, serde_json::to_string_pretty(&overlay)?)?;
        Ok(path)
    }

    pub fn set_global_retrieval_engine(engine: RetrievalEngine) -> Result<PathBuf> {
        let path = Self::home_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut cfg = if path.exists() {
            Self::read_file(&path).unwrap_or_default()
        } else {
            Self::default()
        };
        cfg.retrieval.engine = engine;
        cfg.apply_retrieval_preset();
        fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
        Ok(path)
    }

    pub fn set_workspace_retrieval_engine(
        workspace: &Path,
        engine: RetrievalEngine,
    ) -> Result<PathBuf> {
        let path = Self::workspace_nm_config_path(workspace);
        let mut overlay = Self::read_nm_config(workspace).unwrap_or_default();
        let mut retrieval = overlay.retrieval.take().unwrap_or_default();
        retrieval.engine = engine;
        overlay.retrieval = Some(retrieval);
        fs::write(&path, serde_json::to_string_pretty(&overlay)?)?;
        Ok(path)
    }

    pub fn global_retrieval_engine() -> Option<RetrievalEngine> {
        let path = Self::home_config_path();
        Self::read_file(&path).map(|c| c.retrieval.engine)
    }

    pub fn workspace_retrieval_engine(workspace: &Path) -> Option<RetrievalEngine> {
        Self::read_nm_config(workspace)
            .and_then(|o| o.retrieval)
            .map(|r| r.engine)
    }

    pub fn project_slot_config(workspace: &Path) -> Option<Self> {
        let path = crate::paths::project_config_path(workspace);
        if path.exists() {
            Self::read_file(&path)
        } else {
            None
        }
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

/// `auto` / `0` = grow to production sources. Otherwise a positive file count.
pub fn parse_max_files(raw: &str) -> Result<Option<usize>> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("auto") || raw == "0" {
        return Ok(None);
    }
    let n: usize = raw
        .parse()
        .map_err(|_| NeuroMeshError::Config(format!("invalid --max-files: {raw}")))?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(n))
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

    #[test]
    fn parse_max_files_auto_and_limit() {
        assert_eq!(parse_max_files("auto").unwrap(), None);
        assert_eq!(parse_max_files("0").unwrap(), None);
        assert_eq!(parse_max_files("20000").unwrap(), Some(20000));
        assert!(parse_max_files("nope").is_err());
    }
}
