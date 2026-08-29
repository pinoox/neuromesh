use crate::GraphProxyConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SeedEngineId {
    Off,
    Keywords,
    #[default]
    KeywordsExpanded,
    SemanticLite,
    Hybrid,
}

impl SeedEngineId {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "keywords" => Some(Self::Keywords),
            "keywords_expanded" | "keywords-expanded" => Some(Self::KeywordsExpanded),
            "semantic_lite" | "semantic-lite" => Some(Self::SemanticLite),
            "hybrid" => Some(Self::Hybrid),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Keywords => "keywords",
            Self::KeywordsExpanded => "keywords_expanded",
            Self::SemanticLite => "semantic_lite",
            Self::Hybrid => "hybrid",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Off,
            Self::Keywords,
            Self::KeywordsExpanded,
            Self::SemanticLite,
            Self::Hybrid,
        ]
    }

    pub fn help_line() -> &'static str {
        "off | keywords | keywords_expanded | semantic_lite | hybrid"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedSignalWeights {
    pub exact_identifier_match: f32,
    pub primary_keyword_match: f32,
    pub expansion_match: f32,
    pub path_hint_bonus: f32,
    pub entity_type_bonus: f32,
}

impl Default for SeedSignalWeights {
    fn default() -> Self {
        Self {
            exact_identifier_match: 1.0,
            primary_keyword_match: 0.8,
            expansion_match: 0.5,
            path_hint_bonus: 0.3,
            entity_type_bonus: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeedResolutionConfig {
    pub engine: SeedEngineId,
    pub max_keywords: usize,
    pub max_expansion: usize,
    pub max_resolved_seeds: usize,
    pub min_seed_score_threshold: f32,
    pub weights: SeedSignalWeights,
}

impl Default for SeedResolutionConfig {
    fn default() -> Self {
        Self {
            engine: SeedEngineId::KeywordsExpanded,
            max_keywords: 8,
            max_expansion: 8,
            max_resolved_seeds: 5,
            min_seed_score_threshold: 0.3,
            weights: SeedSignalWeights::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PacketHeaderConfig {
    pub enabled: bool,
    pub max_call_chain_depth: usize,
    pub include_stack: bool,
    pub include_seeds: bool,
    pub include_flow: bool,
}

impl Default for PacketHeaderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_call_chain_depth: 4,
            include_stack: true,
            include_seeds: true,
            include_flow: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeedResolutionTelemetry {
    pub engine: String,
    pub seeds_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub monorepo_packages: Vec<String>,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NmConfigOverlay {
    #[serde(default)]
    pub seed_resolution: Option<SeedResolutionConfig>,
    #[serde(default)]
    pub packet_header: Option<PacketHeaderConfig>,
    #[serde(default)]
    pub graph_backend: Option<GraphProxyConfig>,
}
