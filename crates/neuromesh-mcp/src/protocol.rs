use serde_json::{json, Value};

use crate::uri::workspace_from_initialize;

const FALLBACK_PROTOCOL: &str = "2024-11-05";

/// Echo a dated MCP protocol version; unknown strings fall back to 2024-11-05.
pub fn negotiate_protocol_version(requested: Option<&str>) -> String {
    let Some(v) = requested.map(str::trim).filter(|s| !s.is_empty()) else {
        return FALLBACK_PROTOCOL.to_string();
    };
    if is_dated_protocol(v) {
        return v.to_string();
    }
    FALLBACK_PROTOCOL.to_string()
}

fn is_dated_protocol(v: &str) -> bool {
    let mut parts = v.split('-');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(y), Some(m), Some(d), None)
            if y.len() == 4
                && m.len() == 2
                && d.len() == 2
                && y.bytes().all(|c| c.is_ascii_digit())
                && m.bytes().all(|c| c.is_ascii_digit())
                && d.bytes().all(|c| c.is_ascii_digit())
    )
}

pub fn is_notification(id: Option<&Value>, method: &str) -> bool {
    if method == "initialized"
        || method == "exit"
        || method.starts_with("notifications/")
        || method.starts_with("$/")
    {
        return true;
    }
    matches!(id, None | Some(Value::Null))
}

pub fn extract_tool_name(params: &Value) -> String {
    params
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| params.get("tool").and_then(Value::as_str))
        .or_else(|| params.get("toolName").and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// MCP `arguments`, plus `args` / `parameters` / `input` used by some SDKs.
pub fn extract_tool_arguments(params: &Value) -> Value {
    let mut args = params
        .get("arguments")
        .or_else(|| params.get("args"))
        .or_else(|| params.get("parameters"))
        .or_else(|| params.get("input"))
        .cloned()
        .unwrap_or(Value::Null);

    if let Some(s) = args.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            args = parsed;
        }
    }

    if args.is_null() && params.is_object() {
        let mut obj = params.clone();
        if let Some(map) = obj.as_object_mut() {
            for k in ["name", "tool", "toolName"] {
                map.remove(k);
            }
            if !map.is_empty() {
                args = obj;
            }
        }
    }
    args
}

pub fn canonical_tool_name(name: &str) -> String {
    let trimmed = name.trim();
    let stripped = trimmed
        .strip_prefix("neuromesh_")
        .or_else(|| trimmed.strip_prefix("neuromesh."))
        .unwrap_or(trimmed);
    match stripped {
        "get_context_packet" | "get_context" | "activate_context" | "context" => {
            "get_context_packet".into()
        }
        "get_file_skeleton" | "file_skeleton" | "skeleton" => "neuromesh_get_file_skeleton".into(),
        "expand_fold" | "expand_context" | "expand" => "neuromesh_expand_fold".into(),
        "search_symbols" | "search_context" | "get_symbol" | "search" => {
            "neuromesh_search_symbols".into()
        }
        "get_dependencies" | "get_dependency_graph" | "dependencies" => {
            "neuromesh_get_dependencies".into()
        }
        "record_feedback" | "feedback" => "neuromesh_record_feedback".into(),
        "trace" => "neuromesh_trace".into(),
        "analyze_impact" | "impact" => "neuromesh_analyze_impact".into(),
        "get_architecture" | "architecture" => "neuromesh_get_architecture".into(),
        "get_project_memory" | "project_memory" | "memory" => "neuromesh_get_project_memory".into(),
        "get_stats" | "stats" => "neuromesh_get_stats".into(),
        "get_node_weights" | "node_weights" | "weights" => "neuromesh_get_node_weights".into(),
        "expand_gap" | "gap" => "neuromesh_expand_gap".into(),
        "explain_packet" | "get_context_details" => "neuromesh_explain_packet".into(),
        _ if trimmed.starts_with("neuromesh_") => trimmed.to_string(),
        _ => trimmed.to_string(),
    }
}

pub fn initialize_instructions() -> &'static str {
    "NeuroMesh MCP v0.8.6 — agent loop: (1) get_context_packet with the user task as written (bundled MiniLM embed — prompt only; no client keywords unless the project enabled a custom lexical seed engine); (2) if coverage is partial/no_seed_resolved/no_confident_match, neuromesh_search_symbols or neuromesh_expand_gap; (3) neuromesh_expand_fold only when you need a folded body; (4) neuromesh_trace / neuromesh_get_dependencies for callers and blast radius; (5) after a successful edit, neuromesh_record_feedback with touched nodes. Check retrieval.resolution_tier and retrieval.cache_hit. Prefer these tools over reading whole files. neuromesh_get_context is deprecated — use get_context_packet."
}

pub fn tool_success(id: Option<Value>, val: &Value) -> Value {
    let text = serde_json::to_string(val).unwrap_or_else(|_| val.to_string());
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }],
            "structuredContent": val,
            "isError": false
        }
    })
}

pub fn tool_error(id: Option<Value>, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }
    })
}

pub fn workspace_path_from_params(params: Option<&Value>) -> Option<std::path::PathBuf> {
    params.and_then(workspace_from_initialize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_dated_versions() {
        assert_eq!(negotiate_protocol_version(None), "2024-11-05");
        assert_eq!(negotiate_protocol_version(Some("2025-03-26")), "2025-03-26");
        assert_eq!(negotiate_protocol_version(Some("nope")), "2024-11-05");
        assert_eq!(negotiate_protocol_version(Some("")), "2024-11-05");
    }

    #[test]
    fn notifications_have_no_id_or_null() {
        assert!(is_notification(None, "tools/list"));
        assert!(is_notification(Some(&Value::Null), "tools/list"));
        assert!(is_notification(
            Some(&json!(1)),
            "notifications/initialized"
        ));
        assert!(is_notification(Some(&json!(1)), "initialized"));
        assert!(!is_notification(Some(&json!(1)), "initialize"));
    }

    #[test]
    fn tool_args_from_input_and_string() {
        let params = json!({ "name": "neuromesh_get_context", "input": { "task": "fix login" } });
        assert_eq!(extract_tool_name(&params), "neuromesh_get_context");
        assert_eq!(extract_tool_arguments(&params)["task"], "fix login");

        let params = json!({
            "name": "neuromesh_get_context",
            "arguments": "{\"prompt\":\"hello\"}"
        });
        assert_eq!(extract_tool_arguments(&params)["prompt"], "hello");
    }

    #[test]
    fn canonical_names() {
        assert_eq!(canonical_tool_name("get_context"), "get_context_packet");
        assert_eq!(
            canonical_tool_name("neuromesh_get_context"),
            "get_context_packet"
        );
        assert_eq!(canonical_tool_name("search"), "neuromesh_search_symbols");
        assert_eq!(
            canonical_tool_name("explain_packet"),
            "neuromesh_explain_packet"
        );
        assert_eq!(
            canonical_tool_name("get_context_details"),
            "neuromesh_explain_packet"
        );
    }

    #[test]
    fn structured_content_is_not_pretty_duplicated() {
        let val = json!({
            "packet_id": "ctx_x",
            "coverage": "partial",
            "files": [{ "path": "a.rs", "code": "fn a() {}" }]
        });
        let resp = tool_success(Some(json!(1)), &val);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            !text.contains('\n'),
            "tool result text must be minified, got: {text}"
        );
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed, val);
        assert_eq!(resp["result"]["structuredContent"], val);
        assert!(!text.contains("original_body"));
    }
}
