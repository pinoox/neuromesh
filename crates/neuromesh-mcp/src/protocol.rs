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
        "get_context" | "activate_context" | "context" => "neuromesh_get_context".into(),
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
        _ if trimmed.starts_with("neuromesh_") => trimmed.to_string(),
        _ => trimmed.to_string(),
    }
}

pub fn initialize_instructions() -> &'static str {
    "Start every coding task with neuromesh_get_context. Pass the user prompt as task_description, prompt, or task. If coverage.claim is partial or no_seed_resolved, call neuromesh_search_symbols — do not treat a utility fallback file as the answer. Expand folds with neuromesh_expand_fold. After a successful edit, call neuromesh_record_feedback with the nodes you touched."
}

pub fn tool_success(id: Option<Value>, val: &Value) -> Value {
    let text = serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string());
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
        assert_eq!(canonical_tool_name("get_context"), "neuromesh_get_context");
        assert_eq!(
            canonical_tool_name("neuromesh_get_context"),
            "neuromesh_get_context"
        );
        assert_eq!(canonical_tool_name("search"), "neuromesh_search_symbols");
    }
}
