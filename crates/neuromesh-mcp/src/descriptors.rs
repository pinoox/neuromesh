use serde_json::{json, Value};

fn read_only() -> Value {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    })
}

fn mutating() -> Value {
    json!({
        "readOnlyHint": false,
        "destructiveHint": false,
        "idempotentHint": false,
        "openWorldHint": false
    })
}

fn tool(name: &str, title: &str, description: &str, schema: Value, annotations: Value) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": schema,
        "annotations": annotations
    })
}

/// MCP `tools/list` payload. `get_context_packet` accepts `query`, `task_description`, `prompt`, or `task`.
pub fn tools_list() -> Vec<Value> {
    vec![
        tool(
            "get_context_packet",
            "Get evidence packet",
            "Return a compact evidence packet by default: packet_id, coverage (claim: no_recorded_gap | bounded | partial | no_seed_resolved; covered, skipped, sidecar_files, unsure, packet_gaps), selected/packet tokens, skeletonized files with optional sidecar:true, fold ids without bodies, and missing seeds only when coverage is partial or no_seed_resolved. no_recorded_gap only when seeds resolve, gaps empty, no sidecars, and packet not budget-truncated. bounded means seeds resolved but connector/sidecar fill or budget cut applied. mode controls file selection quality; response_detail controls metadata. Pass diagnostic on-demand via neuromesh_explain_packet. Never treats silence as completeness. Compound tasks seed each topical cluster independently; a named cluster with zero hits is partial, not no_recorded_gap. no_seed_resolved means every identifier missed — Grep immediately. Pass the user prompt as query, task_description, prompt, or task. The server auto-extracts English code keywords and related expansion by default (auto_extract_keywords=true); client keywords/expansion are optional overrides for NL or multilingual prompts.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Primary user prompt (alias: task_description, prompt, task)"
                    },
                    "task_description": {
                        "type": "string",
                        "description": "The user's prompt or coding task"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Alias for task_description"
                    },
                    "task": {
                        "type": "string",
                        "description": "Alias for task_description"
                    },
                    "keywords": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional override: 3–5 core English code terms (server auto-extracts when omitted)"
                    },
                    "expansion": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional override: 3–5 related concepts (server auto-extracts when omitted)"
                    },
                    "path_hints": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional glob path patterns (e.g. **/auth/**)"
                    },
                    "entity_types": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional AST entity hints (class, function, model, controller, …)"
                    },
                    "intent": {
                        "type": "string",
                        "enum": ["explain", "debug", "refactor", "general"],
                        "description": "Client fold-density hint; separate from server task intent"
                    },
                    "engine": {
                        "type": "string",
                        "enum": ["off", "keywords", "keywords_expanded", "semantic_lite", "hybrid"],
                        "description": "Per-call seed resolution engine override"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["balanced", "max_quality", "max_savings"],
                        "description": "File-selection quality (default: balanced). Independent of response_detail."
                    },
                    "response_detail": {
                        "type": "string",
                        "enum": ["minimal", "standard", "diagnostic"],
                        "description": "Metadata verbosity (default: minimal). max_quality does not imply more metadata."
                    },
                    "auto_extract_keywords": {
                        "type": "boolean",
                        "description": "When true (default), server infers keywords/expansion from the prompt for missing sides. Set false for raw semantics."
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_get_file_skeleton",
            "Skeletonize file",
            "Skeletonize one file: seed symbols stay open as exons; sibling functions fold to reversible one-line markers. Fold metadata is fold_id/signature/lines only — original bodies stay in the session registry and return via neuromesh_expand_fold.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Relative file path in workspace"
                    },
                    "path": {
                        "type": "string",
                        "description": "Alias for file_path"
                    },
                    "active_symbols": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Symbol/function names to keep unfolded"
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_explain_packet",
            "Explain packet",
            "On-demand diagnostic metadata for a packet_id from get_context_packet (seeds, selection, budget, physarum, membrane). Does not return folded source bodies. Graph stats are included only when include contains graph. Expired or unknown packet_id is an error — call get_context_packet again.",
            json!({
                "type": "object",
                "properties": {
                    "packet_id": {
                        "type": "string",
                        "description": "packet_id from the last get_context response"
                    },
                    "include": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["seeds", "selection", "budget", "graph", "physarum", "membrane"]
                        },
                        "description": "Sections to return. Omit for all except graph."
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_expand_fold",
            "Expand fold",
            "Reversibly expand a folded intron or inactive node to retrieve full source code. Pass the fold_id printed in the packet as fold_id, node_id, or query (next_actions uses query).",
            json!({
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
                    "query": {
                        "type": "string",
                        "description": "Alias used by next_actions — the fold_id printed in the packet"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for expansion"
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_search_symbols",
            "Search symbols",
            "Search the Neural Project Graph for symbol definitions, function signatures, classes, and types.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Symbol name or keyword to search"
                    },
                    "name": {
                        "type": "string",
                        "description": "Alias for query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default 20)"
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_get_dependencies",
            "Get dependencies",
            "Get weighted graph dependencies, synaptic connections, and imports for a symbol or file.",
            json!({
                "type": "object",
                "properties": {
                    "symbol_or_path": {
                        "type": "string",
                        "description": "Symbol name or relative file path"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Alias for symbol_or_path"
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_record_feedback",
            "Record feedback",
            "Required after a successful edit: spike the nodes you touched so STDP/pheromone can change the next get_context packet. Without this call there is no synaptic learning.",
            json!({
                "type": "object",
                "properties": {
                    "task_success": {
                        "type": "boolean",
                        "description": "Whether the code change succeeded without errors"
                    },
                    "touched_nodes": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Node or symbol IDs modified or verified"
                    }
                },
                "required": ["task_success"]
            }),
            mutating(),
        ),
        tool(
            "neuromesh_trace",
            "Trace calls",
            "Trace inbound/outbound call and import chains for a symbol.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Function, type, or file to trace"
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["inbound", "outbound", "both", "in", "out"],
                        "description": "Traversal direction (default both)"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Max hops, 1-6 (default 3)"
                    }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_analyze_impact",
            "Analyze impact",
            "Compute the blast radius of changing a symbol or file.",
            json!({
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
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_get_architecture",
            "Architecture",
            "Summarize languages, packages, entry points, and graph hotspots from the live index.",
            json!({ "type": "object", "properties": {} }),
            read_only(),
        ),
        tool(
            "neuromesh_get_project_memory",
            "Project memory",
            "Retrieve project architectural rules, tech stack decisions, and framework conventions.",
            json!({ "type": "object", "properties": {} }),
            read_only(),
        ),
        tool(
            "neuromesh_get_node_weights",
            "Node learning weights",
            "Read access_count, base_relevance, learning_bonus, and neighbor pheromone weights for a symbol or path. Use before/after record_feedback to verify learning.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Symbol name or file path" },
                    "symbol": { "type": "string", "description": "Alias for query" },
                    "path": { "type": "string", "description": "Alias for query" }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_expand_gap",
            "Expand packet gap",
            "Cheap skeleton for a path listed in packet_gaps or unsure — avoids blind Grep when coverage is partial.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative file path from packet_gaps" },
                    "file_path": { "type": "string", "description": "Alias for path" },
                    "token_cap": { "type": "integer", "description": "Max skeleton tokens (default 200)" }
                }
            }),
            read_only(),
        ),
        tool(
            "neuromesh_get_stats",
            "Graph stats",
            "Graph size plus honest biomimetic flags: physarum_solver is active only when the last get_context actually ran neighborhood tubes.",
            json!({ "type": "object", "properties": {} }),
            read_only(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::tools_list;

    #[test]
    fn get_context_packet_schema_is_flexible() {
        let tools = tools_list();
        let ctx = tools
            .iter()
            .find(|t| t["name"] == "get_context_packet")
            .unwrap();
        assert!(ctx["inputSchema"].get("required").is_none());
        assert!(ctx["inputSchema"]["properties"].get("query").is_some());
        assert!(ctx["inputSchema"]["properties"].get("prompt").is_some());
        assert!(ctx["inputSchema"]["properties"].get("task").is_some());
        assert!(ctx["inputSchema"]["properties"].get("keywords").is_some());
        assert!(ctx["inputSchema"]["properties"].get("expansion").is_some());
        assert!(ctx["inputSchema"]["properties"].get("engine").is_some());
        assert!(ctx["inputSchema"]["properties"]
            .get("response_detail")
            .is_some());
        assert_eq!(ctx["annotations"]["readOnlyHint"], true);
    }

    #[test]
    fn explain_packet_is_listed() {
        let tools = tools_list();
        assert!(tools
            .iter()
            .any(|t| t["name"] == "neuromesh_explain_packet"));
    }

    #[test]
    fn expand_fold_accepts_query_alias() {
        let tools = tools_list();
        let expand = tools
            .iter()
            .find(|t| t["name"] == "neuromesh_expand_fold")
            .unwrap();
        assert!(expand["inputSchema"]["properties"].get("query").is_some());
        assert!(expand["inputSchema"]["properties"].get("fold_id").is_some());
    }
}
