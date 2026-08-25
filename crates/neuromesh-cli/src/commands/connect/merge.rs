use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
}

impl LaunchSpec {
    pub fn mcp_servers_entry(&self) -> Value {
        json!({
            "command": self.command,
            "args": self.args,
            "cwd": self.cwd,
            "env": self.env,
        })
    }

    pub fn vscode_entry(&self) -> Value {
        json!({
            "type": "stdio",
            "command": self.command,
            "args": self.args,
            "cwd": self.cwd,
            "env": self.env,
        })
    }

    pub fn kilo_entry(&self) -> Value {
        let mut command = Vec::with_capacity(1 + self.args.len());
        command.push(self.command.clone());
        command.extend(self.args.iter().cloned());
        json!({
            "type": "local",
            "command": command,
            "environment": self.env,
        })
    }

    pub fn toml_block(&self) -> String {
        let mut out = String::from("[mcp_servers.neuromesh]\n");
        out.push_str(&format!("command = {}\n", toml_string(&self.command)));
        out.push_str(&format!("args = [{}]\n", toml_string_array(&self.args)));
        out.push_str(&format!("cwd = {}\n", toml_string(&self.cwd)));
        if !self.env.is_empty() {
            out.push_str("\n[mcp_servers.neuromesh.env]\n");
            for (k, v) in &self.env {
                out.push_str(&format!("{k} = {}\n", toml_string(v)));
            }
        }
        out
    }
}

pub fn upsert_mcp_servers(root: &mut Value, name: &str, entry: Value) {
    let obj = ensure_object(root);
    let servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
    let map = ensure_object(servers);
    map.insert(name.to_string(), entry);
}

pub fn upsert_vscode_servers(root: &mut Value, name: &str, entry: Value) {
    let obj = ensure_object(root);
    let servers = obj.entry("servers").or_insert_with(|| json!({}));
    ensure_object(servers).insert(name.to_string(), entry.clone());
    let mcp = obj.entry("mcpServers").or_insert_with(|| json!({}));
    ensure_object(mcp).insert(name.to_string(), entry);
}

pub fn upsert_kilo_mcp(root: &mut Value, name: &str, entry: Value) {
    let obj = ensure_object(root);
    let mcp = obj.entry("mcp").or_insert_with(|| json!({}));
    ensure_object(mcp).insert(name.to_string(), entry);
}

pub fn upsert_toml_table(src: &str, table: &str, body: &str) -> String {
    let mut kept = String::new();
    let mut skipping = false;
    if !src.is_empty() {
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.starts_with("[[") {
                let name = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
                skipping = name == table || name.starts_with(&format!("{table}."));
            }
            if !skipping {
                kept.push_str(line);
                kept.push('\n');
            }
        }
    }
    let kept = kept.trim_end();
    let mut out = String::new();
    if !kept.is_empty() {
        out.push_str(kept);
        out.push_str("\n\n");
    }
    out.push_str(body.trim_end());
    out.push('\n');
    out
}

pub fn parse_json_lenient(raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Ok(v);
    }
    serde_json::from_str::<Value>(&strip_jsonc(trimmed)).map_err(|e| format!("invalid JSON: {e}"))
}

pub fn load_json_object(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value = parse_json_lenient(&raw)?;
    if value.is_object() || value.is_null() {
        Ok(if value.is_null() { json!({}) } else { value })
    } else {
        Err("config root must be a JSON object".into())
    }
}

pub fn write_pretty_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut body = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    body.push('\n');
    std::fs::write(path, body).map_err(|e| e.to_string())
}

pub fn load_text(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

pub fn write_text(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, body).map_err(|e| e.to_string())
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("object")
}

fn toml_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn toml_string_array(items: &[String]) -> String {
    items
        .iter()
        .map(|s| toml_string(s))
        .collect::<Vec<_>>()
        .join(", ")
}

fn strip_jsonc(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_str = false;
    let mut escape = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn spec() -> LaunchSpec {
        let mut env = BTreeMap::new();
        env.insert("NEUROMESH_WORKSPACE".into(), "/tmp/proj".into());
        LaunchSpec {
            command: "/usr/bin/neuromesh".into(),
            args: vec!["mcp".into(), "/tmp/proj".into()],
            cwd: "/tmp/proj".into(),
            env,
        }
    }

    #[test]
    fn merges_mcp_servers_without_clobbering() {
        let mut root = json!({ "mcpServers": { "other": { "command": "x" } } });
        upsert_mcp_servers(&mut root, "neuromesh", spec().mcp_servers_entry());
        assert_eq!(root["mcpServers"]["other"]["command"], "x");
        assert_eq!(
            root["mcpServers"]["neuromesh"]["command"],
            "/usr/bin/neuromesh"
        );
        assert_eq!(root["mcpServers"]["neuromesh"]["args"][0], "mcp");
    }

    #[test]
    fn vscode_gets_servers_and_mcp_servers() {
        let mut root = json!({});
        upsert_vscode_servers(&mut root, "neuromesh", spec().vscode_entry());
        assert_eq!(root["servers"]["neuromesh"]["type"], "stdio");
        assert_eq!(root["mcpServers"]["neuromesh"]["type"], "stdio");
    }

    #[test]
    fn kilo_uses_command_array_and_environment() {
        let mut root = json!({ "other": 1 });
        upsert_kilo_mcp(&mut root, "neuromesh", spec().kilo_entry());
        assert_eq!(root["other"], 1);
        let cmd = root["mcp"]["neuromesh"]["command"].as_array().unwrap();
        assert_eq!(cmd[0], "/usr/bin/neuromesh");
        assert_eq!(cmd[1], "mcp");
        assert_eq!(root["mcp"]["neuromesh"]["type"], "local");
        assert_eq!(
            root["mcp"]["neuromesh"]["environment"]["NEUROMESH_WORKSPACE"],
            "/tmp/proj"
        );
    }

    #[test]
    fn toml_replaces_only_neuromesh_tables() {
        let src = "[mcp_servers.other]\ncommand = \"keep\"\n\n[mcp_servers.neuromesh]\ncommand = \"old\"\n\n[mcp_servers.neuromesh.env]\nFOO = \"1\"\n";
        let out = upsert_toml_table(src, "mcp_servers.neuromesh", &spec().toml_block());
        assert!(out.contains("command = \"keep\""));
        assert!(out.contains("[mcp_servers.neuromesh]"));
        assert!(out.contains("NEUROMESH_WORKSPACE"));
        assert!(!out.contains("command = \"old\""));
        assert!(!out.contains("FOO = \"1\""));
    }

    #[test]
    fn jsonc_comments_are_stripped() {
        let raw = "{\n  // cursor\n  \"mcpServers\": { /* x */ }\n}\n";
        let v = parse_json_lenient(raw).unwrap();
        assert!(v.get("mcpServers").is_some());
    }
}
