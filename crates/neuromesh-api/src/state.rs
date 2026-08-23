use neuromesh_cache::{SemanticCache, ToolCache};
use neuromesh_context::{ContextActivator, ExpansionEngine, ReversibleContextRegistry};
use neuromesh_core::Config;
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_local_ai::LocalAiEngine;
use neuromesh_mcp::McpToolHandler;
use neuromesh_memory::{MemoryDatabase, WorkingMemory};
use neuromesh_observability::MetricsCollector;
use neuromesh_provider::Provider;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub message: String,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub graph: Arc<NeuralProjectGraph>,
    pub registry: Arc<ReversibleContextRegistry>,
    pub activator: Arc<ContextActivator>,
    pub expansion_engine: Arc<ExpansionEngine>,
    pub memory_db: Arc<MemoryDatabase>,
    pub tool_cache: Arc<ToolCache>,
    pub semantic_cache: Arc<SemanticCache>,
    pub metrics: Arc<MetricsCollector>,
    pub provider: Arc<dyn Provider>,
    pub local_ai: Arc<LocalAiEngine>,
    pub working_memory: Arc<RwLock<WorkingMemory>>,
    pub mcp_handler: Arc<McpToolHandler>,
    pub workspace_path: Arc<RwLock<PathBuf>>,
    pub project_access_times: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    pub deleted_project_paths: Arc<RwLock<std::collections::HashSet<String>>>,
    pub audit_logs: Arc<RwLock<Vec<AuditLogEntry>>>,
}

impl AppState {
    pub fn new(
        config: Config,
        graph: Arc<NeuralProjectGraph>,
        memory_db: Arc<MemoryDatabase>,
        provider: Arc<dyn Provider>,
    ) -> Self {
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = Arc::new(ContextActivator::new(registry.clone()));
        let expansion_engine = Arc::new(ExpansionEngine::new(registry.clone()));
        let local_ai = Arc::new(LocalAiEngine::new(config.local_ai.clone()));
        let working_memory = Arc::new(RwLock::new(WorkingMemory::default()));

        let mcp_handler = Arc::new(McpToolHandler::new(
            graph.clone(),
            activator.clone(),
            expansion_engine.clone(),
            memory_db.clone(),
            working_memory.clone(),
        ));

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut access_times = std::collections::HashMap::new();
        access_times.insert(
            current_dir.display().to_string(),
            chrono::Utc::now().timestamp_millis() as u64,
        );

        let initial_logs = vec![
            AuditLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                level: "INFO".to_string(),
                category: "CORE".to_string(),
                message: "Biomimetic MCP Engine & Neural Graph initialized".to_string(),
            },
            AuditLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                level: "SUCCESS".to_string(),
                category: "AST".to_string(),
                message: format!("Indexed workspace: {}", current_dir.display()),
            },
            AuditLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                level: "INFO".to_string(),
                category: "PHYSARUM".to_string(),
                message: "Neighborhood Physarum armed (active only with 2+ seeds, <20ms SLA)"
                    .to_string(),
            },
            AuditLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                level: "SUCCESS".to_string(),
                category: "STDP".to_string(),
                message: "Synaptic STDP Plasticity learning pipeline armed".to_string(),
            },
        ];

        Self {
            config: Arc::new(RwLock::new(config)),
            graph,
            registry,
            activator,
            expansion_engine,
            memory_db,
            tool_cache: Arc::new(ToolCache::new()),
            semantic_cache: Arc::new(SemanticCache::new()),
            metrics: Arc::new(MetricsCollector::new()),
            provider,
            local_ai,
            working_memory,
            mcp_handler,
            workspace_path: Arc::new(RwLock::new(current_dir)),
            project_access_times: Arc::new(RwLock::new(access_times)),
            deleted_project_paths: Arc::new(RwLock::new(std::collections::HashSet::new())),
            audit_logs: Arc::new(RwLock::new(initial_logs)),
        }
    }

    pub fn log(&self, level: &str, category: &str, message: &str) {
        let mut logs = self.audit_logs.write();
        logs.push(AuditLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            category: category.to_string(),
            message: message.to_string(),
        });
        if logs.len() > 500 {
            logs.remove(0);
        }
    }
}
