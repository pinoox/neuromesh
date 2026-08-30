use chrono::Utc;
use neuromesh_core::{OptimizationMetadata, ProjectId};

use crate::metrics::{
    append_unique, load_persisted_history, notify_monitor, save_persisted_history,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetrySurface {
    Mcp,
    Cli,
    Monitor,
    OpenAiProxy,
}

impl TelemetrySurface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::Cli => "cli",
            Self::Monitor => "monitor",
            Self::OpenAiProxy => "openai",
        }
    }
}

/// Unified activity row for MCP tools, CLI commands, and monitor routes.
#[derive(Debug, Clone)]
pub struct ActivityRecord {
    pub request_id: String,
    pub project_id: ProjectId,
    pub mode: String,
    pub command: Option<String>,
    pub surface: TelemetrySurface,
    pub workspace_path: Option<String>,
    pub client_id: Option<String>,
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
    pub task_id: Option<String>,
}

impl ActivityRecord {
    pub fn into_metadata(self) -> OptimizationMetadata {
        OptimizationMetadata {
            request_id: self.request_id,
            task_id: self.task_id,
            project_id: self.project_id,
            mode: self.mode,
            tokens_before: self.tokens_before,
            tokens_after: self.tokens_after,
            token_reduction_pct: self.token_reduction_pct,
            nodes_before: self.nodes_before,
            nodes_after: self.nodes_after,
            expansions_count: self.expansions_count,
            cache_hit: self.cache_hit,
            provider: self.provider,
            model: self.model,
            latency_ms: self.latency_ms,
            success: self.success,
            timestamp: Utc::now(),
            workspace_path: self.workspace_path,
            surface: self.surface.as_str().into(),
            client_id: self.client_id,
            command: self.command,
        }
    }
}

/// Append one activity row to disk and notify the monitor (dedup by `request_id`).
pub fn record_activity(record: ActivityRecord) -> bool {
    record_metadata(record.into_metadata())
}

/// Append metadata directly (legacy MCP path).
pub fn record_metadata(meta: OptimizationMetadata) -> bool {
    let mut history = load_persisted_history();
    if append_unique(&mut history, meta.clone()) {
        save_persisted_history(&history);
        notify_monitor(meta);
        true
    } else {
        false
    }
}

pub fn cli_request_id(command: &str) -> String {
    format!("cli-{command}-{}", uuid::Uuid::new_v4())
}
