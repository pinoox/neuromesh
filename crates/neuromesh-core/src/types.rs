use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub String);

impl ProjectId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn random() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn from_file_path(path: &str) -> Self {
        let normalized = path.replace('\\', "/");
        Self(format!("file:{}", normalized))
    }

    pub fn from_symbol(file_path: &str, symbol: &str) -> Self {
        let normalized = file_path.replace('\\', "/");
        Self(format!("sym:{}:{}", normalized, symbol))
    }

    pub fn random() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub String);

impl EdgeId {
    pub fn new(source: &NodeId, target: &NodeId, edge_type: &EdgeType) -> Self {
        Self(format!("{}->{}::{:?}", source.0, target.0, edge_type))
    }
}

impl std::fmt::Display for EdgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Project,
    Directory,
    File,
    Component,
    Class,
    Function,
    Symbol,
    Import,
    Dependency,
    Api,
    DbModel,
    Test,
    Config,
    Doc,
    Task,
    Decision,
    Memory,
    StyleToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Imports,
    Calls,
    References,
    Contains,
    DependsOn,
    ModifiedWith,
    TestedBy,
    RelatedTo,
    UsedBy,
    PreviouslySuccessfulWith,
}

impl EdgeType {
    /// Base propagation attenuation factor for spreading activation
    pub fn attenuation(&self) -> f32 {
        match self {
            EdgeType::Imports => 1.0,
            EdgeType::DependsOn => 0.95,
            EdgeType::Calls => 0.85,
            EdgeType::PreviouslySuccessfulWith => 0.90,
            EdgeType::Contains => 0.80,
            EdgeType::References => 0.75,
            EdgeType::ModifiedWith => 0.70,
            EdgeType::UsedBy => 0.65,
            EdgeType::TestedBy => 0.50,
            EdgeType::RelatedTo => 0.40,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNode {
    pub id: NodeId,
    pub project_id: ProjectId,
    pub file_path: PathBuf,
    pub node_type: NodeType,
    pub name: String,
    pub signature: Option<String>,
    pub line_range: Option<Range<usize>>,
    pub token_cost: usize,
    pub content: Option<String>,
    pub content_hash: String,
    pub base_relevance: f32,
    pub access_count: u64,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEdge {
    pub id: EdgeId,
    pub project_id: ProjectId,
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: EdgeType,
    pub pheromone_weight: f32,
    pub reinforcement_count: u64,
    pub failure_count: u64,
    pub last_reinforced: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Active,
    Inactive,
    Expanded,
    Bypass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InactiveContextDescriptor {
    pub id: NodeId,
    pub file_path: PathBuf,
    pub line_range: Option<Range<usize>>,
    pub content_hash: String,
    pub version: u64,
    pub token_cost: usize,
    pub relevance: f32,
    pub confidence: f32,
    pub activation_score: f32,
    pub parent_node: Option<NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivatedNodeView {
    pub node: ContextNode,
    pub activation_score: f32,
    pub status: ContextStatus,
    pub expansion_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextView {
    pub project_id: ProjectId,
    pub active_nodes: Vec<ActivatedNodeView>,
    pub inactive_descriptors: Vec<InactiveContextDescriptor>,
    pub total_raw_tokens: usize,
    pub active_tokens: usize,
    pub reduction_percentage: f32,
    pub confidence_score: f32,
    pub bypass_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDiff {
    pub base_hash: String,
    pub new_hash: String,
    pub file_path: PathBuf,
    pub added_lines: Vec<(usize, String)>,
    pub removed_lines: Vec<(usize, String)>,
    pub net_token_change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationMetadata {
    pub request_id: String,
    pub task_id: Option<String>,
    pub project_id: ProjectId,
    pub mode: String,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub token_reduction_pct: f32,
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub expansions_count: usize,
    pub cache_hit: bool,
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}
