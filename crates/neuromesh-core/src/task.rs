use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskIntent {
    Create,
    Modify,
    Refactor,
    Fix,
    Optimize,
    Test,
    Explain,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSignature {
    pub id: String,
    pub intent: TaskIntent,
    pub domain: String,
    pub technology: String,
    pub style: Option<String>,
    pub entity: String,
    pub goal: String,
    pub risk: TaskRisk,
    pub related_concepts: Vec<String>,
    pub identifiers: Vec<String>,
    pub file_hints: Vec<String>,
    pub confidence: f32,
    pub raw_prompt: String,
}

impl TaskSignature {
    pub fn new(raw_prompt: impl Into<String>) -> Self {
        let prompt = raw_prompt.into();
        Self {
            id: Uuid::new_v4().to_string(),
            intent: TaskIntent::Modify,
            domain: "fullstack".into(),
            technology: "generic".into(),
            style: None,
            entity: "workspace".into(),
            goal: "execute task".into(),
            risk: TaskRisk::Low,
            related_concepts: Vec::new(),
            identifiers: Vec::new(),
            file_hints: Vec::new(),
            confidence: 0.85,
            raw_prompt: prompt,
        }
    }

    /// Determines whether the task requires strict conservative optimization due to risk
    pub fn requires_conservative_mode(&self) -> bool {
        if matches!(self.risk, TaskRisk::Critical) {
            return true;
        }
        let lower = self.raw_prompt.to_lowercase();
        lower.contains("auth")
            || lower.contains("security")
            || lower.contains("payment")
            || lower.contains("migration")
            || lower.contains("secret")
            || lower.contains("credential")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskNode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: SubtaskStatus,
    pub dependencies: Vec<String>,
    pub relevant_files: Vec<String>,
    pub relevant_symbols: Vec<String>,
    pub context_tokens_used: usize,
    pub children: Vec<SubtaskNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub id: String,
    pub root_task: String,
    pub signature: TaskSignature,
    pub subtasks: HashMap<String, SubtaskNode>,
    pub execution_order: Vec<String>,
}
