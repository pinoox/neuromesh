use neuromesh_core::{GraphProxyLaunchSpec, GraphProxyProvider};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct DetectReport {
    pub candidates: Vec<DetectedProxy>,
}

#[derive(Debug, Clone)]
pub struct DetectedProxy {
    pub spec: GraphProxyLaunchSpec,
    pub score: u8,
}

pub fn detect_proxy_launch_specs(workspace: &Path) -> DetectReport {
    let mut candidates = Vec::new();
    for path in config_paths(workspace) {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            scan_config_text(&raw, &path, &mut candidates);
        }
    }
    candidates.sort_by_key(|b| std::cmp::Reverse(b.score));
    candidates.dedup_by(|a, b| {
        a.spec.command == b.spec.command
            && a.spec.args == b.spec.args
            && a.spec.provider == b.spec.provider
    });
    DetectReport { candidates }
}

fn config_paths(workspace: &Path) -> Vec<PathBuf> {
    let mut out = vec![
        workspace.join(".cursor").join("mcp.json"),
        workspace.join(".vscode").join("mcp.json"),
        workspace.join(".mcp.json"),
        workspace.join(".codex").join("config.toml"),
    ];
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".cursor").join("mcp.json"));
        out.push(home.join(".codex").join("config.toml"));
        out.push(home.join(".config").join("opencode").join("opencode.jsonc"));
    }
    out
}

fn scan_config_text(raw: &str, path: &Path, out: &mut Vec<DetectedProxy>) {
    if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        scan_toml_servers(raw, path, out);
        return;
    }
    let cleaned = strip_json_comments(raw);
    let Ok(value) = serde_json::from_str::<Value>(&cleaned) else {
        return;
    };
    scan_mcp_servers_object(&value, path, out);
    if let Some(mcp) = value.get("mcp").and_then(Value::as_object) {
        for (name, entry) in mcp {
            if let Some(spec) = entry_from_opencode(name, entry, path) {
                let score = provider_score(spec.provider, name, &spec.command);
                push_candidate(out, spec, score);
            }
        }
    }
}

fn scan_toml_servers(raw: &str, path: &Path, out: &mut Vec<DetectedProxy>) {
    for line in raw.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("command") {
            if lower.contains("cbm") || lower.contains("codebase-memory") {
                push_candidate(
                    out,
                    GraphProxyLaunchSpec {
                        provider: GraphProxyProvider::Cbm,
                        server_name: "codex".into(),
                        command: extract_quoted(line).unwrap_or_else(|| "cbm".into()),
                        args: vec!["mcp".into()],
                        env: HashMap::new(),
                        config_path: Some(path.to_path_buf()),
                    },
                    70,
                );
            }
            if lower.contains("graphify") {
                push_candidate(
                    out,
                    GraphProxyLaunchSpec {
                        provider: GraphProxyProvider::Graphify,
                        server_name: "codex".into(),
                        command: extract_quoted(line).unwrap_or_else(|| "graphify".into()),
                        args: vec!["mcp".into()],
                        env: HashMap::new(),
                        config_path: Some(path.to_path_buf()),
                    },
                    65,
                );
            }
        }
    }
}

fn scan_mcp_servers_object(value: &Value, path: &Path, out: &mut Vec<DetectedProxy>) {
    let Some(servers) = value
        .get("mcpServers")
        .or_else(|| value.get("servers"))
        .and_then(Value::as_object)
    else {
        return;
    };
    for (name, entry) in servers {
        if let Some(spec) = entry_from_mcp_server(name, entry, path) {
            let score = provider_score(spec.provider, name, &spec.command);
            push_candidate(out, spec, score);
        }
    }
}

fn entry_from_mcp_server(name: &str, entry: &Value, path: &Path) -> Option<GraphProxyLaunchSpec> {
    let (command, args, env) = launch_from_entry(entry)?;
    let provider = classify_provider(name, &command, &args)?;
    Some(GraphProxyLaunchSpec {
        provider,
        server_name: name.to_string(),
        command,
        args,
        env,
        config_path: Some(path.to_path_buf()),
    })
}

fn entry_from_opencode(name: &str, entry: &Value, path: &Path) -> Option<GraphProxyLaunchSpec> {
    let cmd = entry.get("command")?;
    let (command, args) = if let Some(s) = cmd.as_str() {
        let args = entry
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        (s.to_string(), args)
    } else if let Some(arr) = cmd.as_array() {
        let mut parts = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }
        let command = parts.remove(0);
        (command, parts)
    } else {
        return None;
    };
    let provider = classify_provider(name, &command, &args)?;
    Some(GraphProxyLaunchSpec {
        provider,
        server_name: name.to_string(),
        command,
        args,
        env: HashMap::new(),
        config_path: Some(path.to_path_buf()),
    })
}

fn launch_from_entry(entry: &Value) -> Option<(String, Vec<String>, HashMap<String, String>)> {
    let command = entry.get("command")?;
    let (cmd, args) = if let Some(s) = command.as_str() {
        let args = entry
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        (s.to_string(), args)
    } else if let Some(arr) = command.as_array() {
        let mut parts = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }
        let cmd = parts.remove(0);
        (cmd, parts)
    } else {
        return None;
    };
    let mut env = HashMap::new();
    if let Some(map) = entry.get("env").and_then(Value::as_object) {
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                env.insert(k.clone(), s.to_string());
            }
        }
    }
    Some((cmd, args, env))
}

fn classify_provider(name: &str, command: &str, args: &[String]) -> Option<GraphProxyProvider> {
    let blob = format!("{name} {command} {}", args.join(" ")).to_ascii_lowercase();
    if blob.contains("graphify") {
        return Some(GraphProxyProvider::Graphify);
    }
    if blob.contains("codebase-memory")
        || blob.contains("codebase_memory")
        || blob.contains("cbm")
        || command.eq_ignore_ascii_case("cbm")
    {
        return Some(GraphProxyProvider::Cbm);
    }
    None
}

fn provider_score(provider: GraphProxyProvider, name: &str, command: &str) -> u8 {
    let n = name.to_ascii_lowercase();
    let c = command.to_ascii_lowercase();
    match provider {
        GraphProxyProvider::Cbm => {
            if n.contains("codebase-memory") || c.contains("cbm") {
                100
            } else {
                80
            }
        }
        GraphProxyProvider::Graphify => {
            if n.contains("graphify") || c.contains("graphify") {
                95
            } else {
                75
            }
        }
    }
}

fn push_candidate(out: &mut Vec<DetectedProxy>, spec: GraphProxyLaunchSpec, score: u8) {
    out.push(DetectedProxy { spec, score });
}

fn extract_quoted(line: &str) -> Option<String> {
    line.split('"')
        .nth(1)
        .map(str::to_string)
        .or_else(|| line.split('\'').nth(1).map(str::to_string))
}

fn strip_json_comments(raw: &str) -> String {
    raw.lines()
        .map(|l| l.split("//").next().unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cbm_in_mcp_json() {
        let raw = r#"{
            "mcpServers": {
                "codebase-memory": {
                    "command": "cbm",
                    "args": ["mcp"]
                }
            }
        }"#;
        let mut out = Vec::new();
        scan_config_text(raw, Path::new("mcp.json"), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].spec.provider, GraphProxyProvider::Cbm);
        assert_eq!(out[0].spec.command, "cbm");
    }

    #[test]
    fn ignores_neuromesh_server() {
        let raw = r#"{"mcpServers":{"neuromesh":{"command":"neuromesh","args":["mcp"]}}}"#;
        let mut out = Vec::new();
        scan_config_text(raw, Path::new("mcp.json"), &mut out);
        assert!(out.is_empty());
    }
}
