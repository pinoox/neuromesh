use neuromesh_core::NeuroMeshError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

const PROTOCOL: &str = "2024-11-05";

#[derive(Clone)]
pub struct McpStdioClient {
    inner: Arc<McpClientInner>,
}

struct McpClientInner {
    _child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
    next_id: AtomicU64,
}

pub struct McpStdioClientHandle {
    inner: Arc<McpClientInner>,
}

impl McpStdioClient {
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> neuromesh_core::Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| NeuroMeshError::Config(format!("failed to spawn {command}: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| NeuroMeshError::Config("proxy MCP: no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| NeuroMeshError::Config("proxy MCP: no stdout".into()))?;
        Ok(Self {
            inner: Arc::new(McpClientInner {
                _child: Mutex::new(child),
                stdin: Mutex::new(stdin),
                reader: Mutex::new(BufReader::new(stdout)),
                next_id: AtomicU64::new(1),
            }),
        })
    }

    pub fn clone_handle(&self) -> McpStdioClientHandle {
        McpStdioClientHandle {
            inner: self.inner.clone(),
        }
    }

    pub async fn initialize(&mut self) -> neuromesh_core::Result<()> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL,
                "capabilities": {},
                "clientInfo": { "name": "neuromesh-graph-proxy", "version": env!("CARGO_PKG_VERSION") }
            }
        });
        self.call_raw(req).await?;
        self.notify(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await
    }

    pub async fn list_tool_names(&mut self) -> neuromesh_core::Result<Vec<String>> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {}
        });
        let resp = self.call_raw(req).await?;
        Ok(resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default())
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> neuromesh_core::Result<Value> {
        let id = self.inner.next_id.load(Ordering::Relaxed);
        self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        let resp = self.inner.call_raw(req).await?;
        extract_tool_result(resp)
    }

    fn next_id(&self) -> u64 {
        self.inner.next_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn notify(&self, value: Value) -> neuromesh_core::Result<()> {
        self.inner.write_line(&value).await
    }

    async fn call_raw(&self, req: Value) -> neuromesh_core::Result<Value> {
        self.inner.call_raw(req).await
    }
}

impl McpStdioClientHandle {
    pub async fn call_tool(&self, name: &str, arguments: Value) -> neuromesh_core::Result<Value> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        let resp = self.inner.call_raw(req).await?;
        extract_tool_result(resp)
    }
}

impl McpClientInner {
    async fn call_raw(&self, req: Value) -> neuromesh_core::Result<Value> {
        let expect_id = req.get("id").cloned();
        self.write_line(&req).await?;
        loop {
            let line = self.read_line().await?;
            let Some(line) = line else {
                return Err(NeuroMeshError::Config("proxy MCP: EOF".into()));
            };
            let msg: Value = serde_json::from_str(&line)
                .map_err(|e| NeuroMeshError::Config(format!("proxy MCP parse: {e}")))?;
            if msg.get("method").is_some() && msg.get("id").is_none() {
                continue;
            }
            if let Some(id) = expect_id.as_ref() {
                if msg.get("id") != Some(id) {
                    continue;
                }
            }
            if let Some(err) = msg.get("error") {
                return Err(NeuroMeshError::Config(format!("proxy MCP error: {err}")));
            }
            return Ok(msg);
        }
    }

    async fn write_line(&self, value: &Value) -> neuromesh_core::Result<()> {
        let mut line = serde_json::to_string(value)
            .map_err(|e| NeuroMeshError::Config(format!("proxy MCP encode: {e}")))?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| NeuroMeshError::Config(format!("proxy MCP write: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| NeuroMeshError::Config(format!("proxy MCP flush: {e}")))?;
        Ok(())
    }

    async fn read_line(&self) -> neuromesh_core::Result<Option<String>> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| NeuroMeshError::Config(format!("proxy MCP read: {e}")))?;
            if n == 0 {
                return Ok(None);
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                return Ok(Some(trimmed.to_string()));
            }
            if trimmed.to_ascii_lowercase().starts_with("content-length:") {
                let len: usize = trimmed
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u8; len];
                use tokio::io::AsyncReadExt;
                reader
                    .read_exact(&mut buf)
                    .await
                    .map_err(|e| NeuroMeshError::Config(format!("proxy MCP body: {e}")))?;
                let body = String::from_utf8_lossy(&buf).trim().to_string();
                if !body.is_empty() {
                    return Ok(Some(body));
                }
            }
        }
    }
}

fn extract_tool_result(resp: Value) -> neuromesh_core::Result<Value> {
    let result = resp
        .get("result")
        .cloned()
        .ok_or_else(|| NeuroMeshError::Config("proxy MCP: missing result".into()))?;
    if result.get("isError").and_then(|v| v.as_bool()) == Some(true) {
        let msg = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("tool error");
        return Err(NeuroMeshError::Config(format!("proxy tool error: {msg}")));
    }
    Ok(result)
}

pub fn tool_text(result: &Value) -> String {
    if let Some(text) = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
    {
        return text.to_string();
    }
    if let Some(structured) = result.get("structuredContent") {
        return structured.to_string();
    }
    result.to_string()
}

pub fn tool_structured(result: &Value) -> Option<Value> {
    result.get("structuredContent").cloned()
}
