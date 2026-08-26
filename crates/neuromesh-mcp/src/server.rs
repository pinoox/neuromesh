use crate::descriptors::tools_list;
use crate::protocol::{
    canonical_tool_name, extract_tool_arguments, extract_tool_name, initialize_instructions,
    is_notification, negotiate_protocol_version, tool_error, tool_success,
    workspace_path_from_params,
};
use crate::tools::McpToolHandler;
use neuromesh_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

fn default_jsonrpc() -> String {
    "2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

pub struct McpServer {
    handler: Arc<McpToolHandler>,
}

impl McpServer {
    pub fn new(handler: Arc<McpToolHandler>) -> Self {
        Self { handler }
    }

    pub async fn run_stdio(&self) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        std::thread::Builder::new()
            .name("neuromesh-mcp-stdin".into())
            .spawn(move || {
                let stdin = std::io::stdin();
                let mut reader = std::io::BufReader::new(stdin.lock());
                while let Ok(Some(msg)) = crate::stdio::read_message(&mut reader) {
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
            })?;

        let mut stdout = tokio::io::stdout();
        while let Some(raw) = rx.recv().await {
            let responses = self.dispatch_raw(&raw).await;
            for response in responses {
                let out_bytes = serde_json::to_vec(&response).unwrap_or_default();
                stdout.write_all(&out_bytes).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    async fn dispatch_raw(&self, raw: &str) -> Vec<Value> {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') {
            return match serde_json::from_str::<Vec<JsonRpcRequest>>(trimmed) {
                Ok(reqs) => {
                    let mut out = Vec::new();
                    for req in reqs {
                        if is_notification(req.id.as_ref(), &req.method) {
                            continue;
                        }
                        out.push(self.process_request(req).await);
                    }
                    out
                }
                Err(_) => Vec::new(),
            };
        }

        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => {
                if is_notification(req.id.as_ref(), &req.method) {
                    return Vec::new();
                }
                vec![self.process_request(req).await]
            }
            Err(_) => {
                if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(id) = val.get("id").cloned() {
                        if !id.is_null() {
                            return vec![json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": { "code": -32700, "message": "Parse error" }
                            })];
                        }
                    }
                }
                Vec::new()
            }
        }
    }

    fn maybe_reindex_from_initialize(&self, params: Option<&Value>) {
        let Some(raw) = workspace_path_from_params(params) else {
            return;
        };
        let p_buf = neuromesh_index::ProjectWalker::discover_workspace(&raw);
        if !p_buf.exists()
            || !neuromesh_index::ProjectWalker::is_safe_workspace(&p_buf)
            || self.handler.graph().stats().total_nodes != 0
        {
            return;
        }
        let p_name = p_buf
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".to_string());
        let pid = neuromesh_core::ProjectId::new(&p_name);
        self.handler.graph().set_project_id(pid.clone());
        let _ = self.handler.graph().load_persisted(&p_buf);
        let bg_graph = self.handler.graph().clone();
        let bg_dir = p_buf.clone();
        let bg_pid = pid.clone();
        tokio::task::spawn_blocking(move || {
            bg_graph.reindex_incremental(&bg_dir, bg_pid, neuromesh_core::Config::load().max_files);
        });
        let watch_graph = self.handler.graph().clone();
        let watch_dir = p_buf;
        let watch_pid = pid;
        tokio::spawn(async move {
            let mut watcher = neuromesh_index::WorkspaceWatcher::new(watch_dir.clone(), watch_pid);
            let (mut rx, _running) = watcher.start();
            while let Some(ev) = rx.recv().await {
                watch_graph.apply_file_event(ev);
                let _ = watch_graph.save_persisted(&watch_dir);
            }
        });
    }

    pub async fn process_request(&self, req: JsonRpcRequest) -> Value {
        match req.method.as_str() {
            "initialize" => {
                self.maybe_reindex_from_initialize(req.params.as_ref());
                let protocol_version = negotiate_protocol_version(
                    req.params
                        .as_ref()
                        .and_then(|p| p.get("protocolVersion"))
                        .and_then(Value::as_str),
                );
                json!({
                    "jsonrpc": "2.0",
                    "id": req.id,
                    "result": {
                        "protocolVersion": protocol_version,
                        "capabilities": {
                            "tools": { "listChanged": false },
                            "prompts": { "listChanged": false },
                            "resources": {
                                "subscribe": false,
                                "listChanged": false
                            },
                            "logging": {},
                            "completions": {}
                        },
                        "serverInfo": {
                            "name": "neuromesh",
                            "title": "NeuroMesh",
                            "version": env!("CARGO_PKG_VERSION")
                        },
                        "instructions": initialize_instructions()
                    }
                })
            }
            "ping" | "shutdown" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {}
            }),
            "logging/setLevel" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {}
            }),
            "tools/list" | "listTools" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": { "tools": tools_list() }
            }),
            "tools/call" | "callTool" | "tools/callTool" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = canonical_tool_name(&extract_tool_name(&params));
                let args = extract_tool_arguments(&params);
                if tool_name.is_empty() {
                    return tool_error(req.id, "tools/call is missing a tool name");
                }
                match self.handler.handle_tool_call(&tool_name, &args).await {
                    Ok(val) => {
                        if val.get("error").and_then(Value::as_str).is_some()
                            && val.get("success") == Some(&json!(false))
                        {
                            tool_error(
                                req.id,
                                val.get("error").and_then(Value::as_str).unwrap_or("error"),
                            )
                        } else if let Some(err) = val.get("error").and_then(Value::as_str) {
                            if val.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                                tool_error(req.id, err)
                            } else {
                                tool_success(req.id, &val)
                            }
                        } else {
                            tool_success(req.id, &val)
                        }
                    }
                    Err(e) => tool_error(req.id, &e.to_string()),
                }
            }
            "prompts/list" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "prompts": [
                        {
                            "name": "activate_context",
                            "title": "Activate NeuroMesh context",
                            "description": "Extract and inject optimized neural context for a task",
                            "arguments": [
                                {
                                    "name": "task",
                                    "description": "Task description or user prompt",
                                    "required": true
                                }
                            ]
                        }
                    ]
                }
            }),
            "prompts/get" => {
                let params = req.params.unwrap_or(Value::Null);
                let prompt_name = params["name"].as_str().unwrap_or("activate_context");
                let task_arg = params["arguments"]["task"]
                    .as_str()
                    .or_else(|| params["arguments"]["task_description"].as_str())
                    .or_else(|| params["arguments"]["prompt"].as_str())
                    .unwrap_or("");
                json!({
                    "jsonrpc": "2.0",
                    "id": req.id,
                    "result": {
                        "description": format!("Optimized prompt for {prompt_name}"),
                        "messages": [
                            {
                                "role": "user",
                                "content": {
                                    "type": "text",
                                    "text": format!("Please activate NeuroMesh context and solve: {task_arg}")
                                }
                            }
                        ]
                    }
                })
            }
            "resources/list" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": { "resources": [] }
            }),
            "resources/templates/list" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": { "resourceTemplates": [] }
            }),
            "resources/read" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "error": {
                    "code": -32002,
                    "message": "NeuroMesh exposes context via tools, not resources"
                }
            }),
            "completion/complete" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "completion": {
                        "values": [],
                        "hasMore": false
                    }
                }
            }),
            _ => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "error": { "code": -32601, "message": format!("Method '{}' not implemented", req.method) }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::McpToolHandler;
    use neuromesh_context::{ContextActivator, ExpansionEngine, ReversibleContextRegistry};
    use neuromesh_core::ProjectId;
    use neuromesh_graph::NeuralProjectGraph;
    use neuromesh_memory::{MemoryDatabase, WorkingMemory};
    use parking_lot::RwLock;

    fn server() -> McpServer {
        let graph = Arc::new(NeuralProjectGraph::new(ProjectId::new("neuromesh")));
        let registry = Arc::new(ReversibleContextRegistry::new());
        let handler = Arc::new(McpToolHandler::new(
            graph,
            Arc::new(ContextActivator::new(registry.clone())),
            Arc::new(ExpansionEngine::new(registry)),
            Arc::new(MemoryDatabase::open_in_memory().unwrap()),
            Arc::new(RwLock::new(WorkingMemory::default())),
        ));
        McpServer::new(handler)
    }

    #[test]
    fn initialize_echoes_dated_version_and_instructions() {
        let srv = server();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let resp = rt.block_on(srv.process_request(JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "initialize".into(),
            params: Some(json!({
                "protocolVersion": "2025-03-26",
                "rootUri": "file:///C:/neuromesh-missing-workspace-zzzz"
            })),
        }));
        assert_eq!(resp["result"]["protocolVersion"], "2025-03-26");
        assert!(resp["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("get_context"));
        assert_eq!(resp["result"]["serverInfo"]["name"], "neuromesh");
    }

    #[test]
    fn tools_list_get_context_is_not_picky_about_key_name() {
        let srv = server();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let resp = rt.block_on(srv.process_request(JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(2)),
            method: "tools/list".into(),
            params: None,
        }));
        let tools = resp["result"]["tools"].as_array().unwrap();
        let ctx = tools
            .iter()
            .find(|t| t["name"] == "neuromesh_get_context")
            .unwrap();
        assert!(ctx["inputSchema"].get("required").is_none());
    }

    #[test]
    fn tools_call_accepts_input_and_short_name() {
        let srv = server();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let resp = rt.block_on(srv.process_request(JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(3)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "get_context",
                "input": { "task": "How does Router work?" }
            })),
        }));
        assert_eq!(resp["result"]["isError"], false);
        assert!(resp["result"]["structuredContent"]
            .get("packet_id")
            .is_some());
    }

    #[test]
    fn empty_get_context_is_tool_error_not_rpc_error() {
        let srv = server();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let resp = rt.block_on(srv.process_request(JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(4)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "neuromesh_get_context",
                "arguments": {}
            })),
        }));
        assert_eq!(resp["result"]["isError"], true);
        assert!(resp.get("error").is_none());
    }
}
