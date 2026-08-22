use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkingMemory {
    pub current_objective: String,
    pub current_hypothesis: Option<String>,
    pub active_files: Vec<String>,
    pub recent_tool_results: Vec<ToolResultSnippet>,
    pub current_errors: Vec<String>,
    pub pending_actions: Vec<String>,
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultSnippet {
    pub tool_name: String,
    pub summary: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl WorkingMemory {
    pub fn new(objective: impl Into<String>) -> Self {
        Self {
            current_objective: objective.into(),
            ..Default::default()
        }
    }

    pub fn add_active_file(&mut self, file: impl Into<String>) {
        let f = file.into();
        if !self.active_files.contains(&f) {
            self.active_files.push(f);
        }
    }

    pub fn record_tool_result(&mut self, tool_name: impl Into<String>, summary: impl Into<String>) {
        self.recent_tool_results.push(ToolResultSnippet {
            tool_name: tool_name.into(),
            summary: summary.into(),
            timestamp: chrono::Utc::now(),
        });
        if self.recent_tool_results.len() > 10 {
            self.recent_tool_results.remove(0);
        }
    }

    pub fn record_error(&mut self, error: impl Into<String>) {
        self.current_errors.push(error.into());
    }

    pub fn clear_errors(&mut self) {
        self.current_errors.clear();
    }
}
