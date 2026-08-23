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
            match serde_json::from_str::<JsonRpcRequest>(&raw) {
                Ok(req) => {
                    // Notifications have no id and must never receive a response.
                    if req.id.is_none()
                        || req.method == "initialized"
                        || req.method.starts_with("notifications/")
                    {
                        continue;
                    }

                    let response = self.process_request(req).await;
                    let out_bytes = serde_json::to_vec(&response).unwrap_or_default();
                    stdout.write_all(&out_bytes).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
                Err(_) => {
                    if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                        if let Some(id) = val.get("id").cloned() {
                            if !id.is_null() {
                                let err = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "error": { "code": -32700, "message": "Parse error" }
                                });
                                let out_bytes = serde_json::to_vec(&err).unwrap_or_default();
                                stdout.write_all(&out_bytes).await?;
                                stdout.write_all(b"\n").await?;
                                stdout.flush().await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn process_request(&self, req: JsonRpcRequest) -> Value {
        match req.method.as_str() {
            "initialize" => {
                let params = req.params.as_ref();
                // Extract workspace root from initialize params if provided
                if let Some(p) = params {
                    let mut detected_path: Option<String> = None;
                    if let Some(folders) = p.get("workspaceFolders").and_then(|f| f.as_array()) {
                        if let Some(first) = folders.first() {
                            if let Some(uri) = first.get("uri").and_then(|u| u.as_str()) {
                                detected_path = Some(
                                    uri.trim_start_matches("file://")
                                        .trim_start_matches("file:///")
                                        .to_string(),
                                );
                            }
                        }
                    }
                    if detected_path.is_none() {
                        if let Some(uri) = p.get("rootUri").and_then(|u| u.as_str()) {
                            detected_path = Some(
                                uri.trim_start_matches("file://")
                                    .trim_start_matches("file:///")
                                    .to_string(),
                            );
                        } else if let Some(path) = p.get("rootPath").and_then(|u| u.as_str()) {
                            detected_path = Some(path.to_string());
                        }
                    }

                    if let Some(raw_p) = detected_path {
                        #[cfg(windows)]
                        let clean_path = raw_p.trim_start_matches('/').replace('/', "\\");
                        #[cfg(not(windows))]
                        let clean_path = raw_p;

                        let p_buf = neuromesh_index::ProjectWalker::discover_workspace(
                            &std::path::PathBuf::from(&clean_path),
                        );
                        if p_buf.exists()
                            && neuromesh_index::ProjectWalker::is_safe_workspace(&p_buf)
                            && self.handler.graph().stats().total_nodes == 0
                        {
                            let p_name = p_buf
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "project".to_string());
                            let pid = neuromesh_core::ProjectId::new(&p_name);
                            self.handler.graph().set_project_id(pid.clone());
                            let bg_graph = self.handler.graph().clone();
                            let bg_dir = p_buf.clone();
                            let bg_pid = pid.clone();
                            let _ = self.handler.graph().load_persisted(&p_buf);
                            tokio::task::spawn_blocking(move || {
                                let walker =
                                    neuromesh_index::ProjectWalker::new(bg_dir.clone(), bg_pid);
                                if let Ok(scanned) = walker.scan() {
                                    bg_graph.ingest_workspace(&scanned);
                                    let _ = bg_graph.save_persisted(&bg_dir);
                                }
                            });
                        }
                    }
                }

                let protocol_version = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");

                json!({
                    "jsonrpc": "2.0",
                    "id": req.id,
                    "result": {
                        "protocolVersion": protocol_version,
                        "capabilities": {
                            "tools": {
                                "listChanged": false
                            },
                            "prompts": {
                                "listChanged": false
                            },
                            "resources": {
                                "subscribe": false,
                                "listChanged": false
                            },
                            "logging": {}
                        },
                        "serverInfo": {
                            "name": "neuromesh",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                })
            }
            "ping" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {}
            }),
            "logging/setLevel" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {}
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "tools": [
                        {
                            "name": "neuromesh_get_context",
                            "description": "Return one evidence packet: seeds, skeletonized files, unresolved gaps, coverage (no_recorded_gap|partial), budget, and next_actions. Never treats silence as completeness.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "task_description": {
                                        "type": "string",
                                        "description": "The user's prompt or coding task description"
                                    },
                                    "mode": {
                                        "type": "string",
                                        "enum": ["balanced", "max_quality", "max_savings"],
                                        "description": "Optimization mode (default: 'balanced')"
                                    }
                                },
                                "required": ["task_description"]
                            }
                        },
                        {
                            "name": "neuromesh_get_file_skeleton",
                            "description": "Skeletonize one file: seed symbols stay open as exons; sibling functions fold to reversible one-line markers. Token reduction is measured per request (original_tokens vs skeleton_tokens), not a global percentage.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "file_path": {
                                        "type": "string",
                                        "description": "Relative file path in workspace (e.g. 'src/components/Header.vue')"
                                    },
                                    "active_symbols": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "List of symbol/function names to keep unfolded"
                                    }
                                },
                                "required": ["file_path"]
                            }
                        },
                        {
                            "name": "neuromesh_expand_fold",
                            "description": "Reversibly expand a folded intron or inactive node to retrieve full source code.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "node_id": {
                                        "type": "string",
                                        "description": "Fold id from the packet (fold_*) or an inactive node id"
                                    },
                                    "fold_id": {
                                        "type": "string",
                                        "description": "Alias for node_id when expanding a [neuromesh:fold] marker"
                                    },
                                    "reason": {
                                        "type": "string",
                                        "description": "Reason for expansion"
                                    }
                                }
                            }
                        },
                        {
                            "name": "neuromesh_search_symbols",
                            "description": "Search the Neural Project Graph for symbol definitions, function signatures, classes, and types.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": {
                                        "type": "string",
                                        "description": "Symbol name or keyword to search"
                                    },
                                    "limit": {
                                        "type": "integer",
                                        "description": "Max results (default 20)"
                                    }
                                },
                                "required": ["query"]
                            }
                        },
                        {
                            "name": "neuromesh_get_dependencies",
                            "description": "Get weighted graph dependencies, synaptic connections, and imports for a symbol or file.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "symbol_or_path": {
                                        "type": "string",
                                        "description": "Symbol name or relative file path"
                                    }
                                },
                                "required": ["symbol_or_path"]
                            }
                        },
                        {
                            "name": "neuromesh_record_feedback",
                            "description": "Required after a successful edit: spike the nodes you touched so STDP/pheromone can change the next get_context packet. Without this call there is no synaptic learning.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "task_success": {
                                        "type": "boolean",
                                        "description": "Whether the code change succeeded without errors"
                                    },
                                    "touched_nodes": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "List of node or symbol IDs modified or verified"
                                    }
                                },
                                "required": ["task_success", "touched_nodes"]
                            }
                        },
                        {
                            "name": "neuromesh_trace",
                            "description": "Trace inbound/outbound call and import chains for a symbol.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": {
                                        "type": "string",
                                        "description": "Function, type, or file to trace"
                                    },
                                    "direction": {
                                        "type": "string",
                                        "enum": ["inbound", "outbound", "both"],
                                        "description": "Traversal direction (default both)"
                                    },
                                    "depth": {
                                        "type": "integer",
                                        "description": "Max hops, 1-6 (default 3)"
                                    }
                                },
                                "required": ["query"]
                            }
                        },
                        {
                            "name": "neuromesh_analyze_impact",
                            "description": "Compute the blast radius of changing a symbol or file.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": {
                                        "type": "string",
                                        "description": "Symbol or file path"
                                    },
                                    "depth": {
                                        "type": "integer",
                                        "description": "Max hops (default 3)"
                                    }
                                },
                                "required": ["query"]
                            }
                        },
                        {
                            "name": "neuromesh_get_architecture",
                            "description": "Summarize languages, packages, entry points, and graph hotspots from the live index.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "neuromesh_get_project_memory",
                            "description": "Retrieve project architectural rules, tech stack decisions, and framework conventions.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "neuromesh_get_stats",
                            "description": "Graph size plus honest biomimetic flags: physarum_solver is active only when the last get_context actually ran neighborhood tubes.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                }
            }),
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params["name"]
                    .as_str()
                    .or_else(|| params["tool"].as_str())
                    .unwrap_or("");

                let mut args = params
                    .get("arguments")
                    .or_else(|| params.get("args"))
                    .or_else(|| params.get("parameters"))
                    .cloned()
                    .unwrap_or(Value::Null);

                // If arguments was passed as a JSON string, parse it into an Object
                if let Some(s) = args.as_str() {
                    if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                        args = parsed;
                    }
                }

                match self.handler.handle_tool_call(tool_name, &args).await {
                    Ok(val) => json!({
                        "jsonrpc": "2.0",
                        "id": req.id,
                        "result": { "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string()) }] }
                    }),
                    Err(e) => json!({
                        "jsonrpc": "2.0",
                        "id": req.id,
                        "error": { "code": -32603, "message": e.to_string() }
                    }),
                }
            }
            "prompts/list" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "prompts": [
                        {
                            "name": "activate_context",
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
                let prompt_name = params["name"].as_str().unwrap_or("");
                let task_arg = params["arguments"]["task"].as_str().unwrap_or("");
                json!({
                    "jsonrpc": "2.0",
                    "id": req.id,
                    "result": {
                        "description": format!("Optimized prompt for {}", prompt_name),
                        "messages": [
                            {
                                "role": "user",
                                "content": {
                                    "type": "text",
                                    "text": format!("Please activate NeuroMesh context and solve: {}", task_arg)
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
