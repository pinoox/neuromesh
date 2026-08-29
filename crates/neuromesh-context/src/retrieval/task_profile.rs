use neuromesh_core::{ContextView, NodeType, TaskSignature};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskProfileKind {
    General,
    Routing,
    Middleware,
    Rendering,
    StaticAssets,
    QueryDb,
    SessionAuth,
    Configuration,
    Testing,
    DependencyTrace,
    Debugging,
    Refactoring,
    Impact,
}

impl TaskProfileKind {
    pub fn role_keywords(self) -> &'static [&'static str] {
        match self {
            Self::General => &[],
            Self::Routing => &["route", "router", "register", "handler", "endpoint", "api"],
            Self::Middleware => &["middleware", "use(", "pipeline", "next("],
            Self::Rendering => &["render", "template", "view", "engine"],
            Self::StaticAssets => &["static", "assets", "public"],
            Self::QueryDb => &["model", "repository", "query", "database", "adapter"],
            Self::SessionAuth => &["session", "cookie", "auth", "login", "sign"],
            Self::Configuration => &["config", "env", "settings", "setup"],
            Self::Testing => &["test", "spec", "mock"],
            Self::DependencyTrace => &["caller", "callee", "depend", "import", "trace"],
            Self::Debugging => &["error", "exception", "bug", "fix"],
            Self::Refactoring => &["refactor", "rename", "export", "interface"],
            Self::Impact => &["affect", "break", "impact", "change", "modify"],
        }
    }
}

pub fn detect_task_profile(signature: &TaskSignature) -> TaskProfileKind {
    let lower = signature.raw_prompt.to_lowercase();
    if lower.contains("middleware") || lower.contains("pipeline") {
        return TaskProfileKind::Middleware;
    }
    if lower.contains("route") || lower.contains("router") || lower.contains("endpoint") {
        return TaskProfileKind::Routing;
    }
    if lower.contains("session") || lower.contains("auth") || lower.contains("cookie") {
        return TaskProfileKind::SessionAuth;
    }
    if lower.contains("database") || lower.contains("model") || lower.contains("query") {
        return TaskProfileKind::QueryDb;
    }
    if lower.contains("test") || lower.contains("spec") {
        return TaskProfileKind::Testing;
    }
    if lower.contains("affect") || lower.contains("break") || lower.contains("impact") {
        return TaskProfileKind::Impact;
    }
    if lower.contains("caller") || lower.contains("depend") || lower.contains("trace") {
        return TaskProfileKind::DependencyTrace;
    }
    if lower.contains("refactor") || lower.contains("rename") {
        return TaskProfileKind::Refactoring;
    }
    if lower.contains("config") || lower.contains("env") {
        return TaskProfileKind::Configuration;
    }
    if lower.contains("render") || lower.contains("template") {
        return TaskProfileKind::Rendering;
    }
    if lower.contains("static") || lower.contains("asset") {
        return TaskProfileKind::StaticAssets;
    }
    if lower.contains("bug") || lower.contains("error") || lower.contains("debug") {
        return TaskProfileKind::Debugging;
    }
    TaskProfileKind::General
}

/// Fraction of task-profile role keywords satisfied by selected file paths (0..1).
pub fn task_role_coverage(view: &ContextView, profile: TaskProfileKind) -> f32 {
    let keywords = profile.role_keywords();
    if keywords.is_empty() {
        return 1.0;
    }
    let paths: HashSet<String> = view
        .active_nodes
        .iter()
        .filter(|n| n.node.node_type == NodeType::File)
        .map(|n| n.node.file_path.to_string_lossy().to_lowercase())
        .collect();
    if paths.is_empty() {
        return 0.0;
    }
    let hits = keywords
        .iter()
        .filter(|kw| paths.iter().any(|p| p.contains(*kw)))
        .count();
    hits as f32 / keywords.len() as f32
}
