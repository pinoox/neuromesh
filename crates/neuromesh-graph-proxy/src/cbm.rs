use crate::mcp_client::{tool_structured, tool_text, McpStdioClientHandle};
use crate::packet::{
    compute_retrieval_hints, ProxyContextFile, ProxyContextPacket, ProxyRetrievalHints,
    ProxySearchContext,
};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

pub struct CbmGraphProxy {
    client: McpStdioClientHandle,
}

impl CbmGraphProxy {
    pub fn new(client: McpStdioClientHandle) -> Self {
        Self { client }
    }

    /// Resolve CBM project id from workspace path via list_projects.
    pub async fn resolve_project(
        client: &McpStdioClientHandle,
        workspace: &Path,
    ) -> neuromesh_core::Result<String> {
        let folder = workspace
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_ascii_lowercase();
        let path_norm = workspace
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();

        let listed = client.call_tool("list_projects", json!({})).await.ok();
        let entries = parse_cbm_projects(&listed);
        if entries.is_empty() {
            return Ok(path_norm);
        }

        // Match CBM root_path to workspace (most reliable).
        if let Some(hit) = entries
            .iter()
            .find(|p| paths_match(workspace, p.root_path.as_deref()))
        {
            return Ok(hit.name.clone());
        }

        let projects: Vec<String> = entries.iter().map(|p| p.name.clone()).collect();

        // Exact project id equals normalized path (legacy).
        if let Some(hit) = projects
            .iter()
            .find(|p| p.to_ascii_lowercase() == path_norm)
        {
            return Ok(hit.clone());
        }

        // Path suffix / contains folder name.
        if let Some(hit) = projects.iter().find(|p| {
            let pl = p.to_ascii_lowercase();
            pl.ends_with(&path_norm) || pl.contains(&folder) || path_norm.contains(&pl)
        }) {
            return Ok(hit.clone());
        }

        // Common suffix patterns: neuromesh → neuromesh-repo
        for suffix in ["-repo", "-project", "_repo"] {
            let candidate = format!("{folder}{suffix}");
            if let Some(hit) = projects
                .iter()
                .find(|p| p.to_ascii_lowercase() == candidate)
            {
                return Ok(hit.clone());
            }
        }

        // Folder name as substring of project id (e.g. neuromesh in neuromesh-repo).
        if let Some(hit) = projects
            .iter()
            .find(|p| p.to_ascii_lowercase().contains(&folder))
        {
            return Ok(hit.clone());
        }

        Ok(projects[0].clone())
    }

    pub async fn build_packet(
        &self,
        project: &str,
        ctx: &ProxySearchContext,
        limit: u32,
    ) -> neuromesh_core::Result<ProxyContextPacket> {
        let search_args = build_search_request(project, ctx, limit);
        let search = self.client.call_tool("search_graph", search_args).await?;

        let hits: Vec<SearchHit> = parse_search_hits(&search)
            .into_iter()
            .filter(should_emit_hit)
            .collect();
        let mut files = Vec::new();
        let mut seen_paths = HashSet::new();
        let mut symbols_found = 0usize;

        for hit in hits.into_iter().take(limit as usize) {
            symbols_found += 1;
            let snippet = if let Some(qn) = hit.qualified_name.as_deref() {
                self.fetch_snippet(project, qn).await.unwrap_or_default()
            } else {
                String::new()
            };
            let code = if snippet.is_empty() {
                hit.summary.clone()
            } else {
                snippet
            };
            if seen_paths.insert(hit.path.clone()) {
                let tokens = code.split_whitespace().count();
                files.push(ProxyContextFile {
                    path: hit.path.clone(),
                    code,
                    tokens,
                    why: format!("cbm:search_graph ({})", hit.label),
                    qualified_name: hit.qualified_name,
                });
            }
        }

        let packet_tokens = files.iter().map(|f| f.tokens).sum();
        let retrieval = compute_retrieval_hints(ctx, &files);
        let coverage = proxy_coverage_label(&files, limit, &retrieval, ctx);
        Ok(ProxyContextPacket {
            task: ctx.raw_prompt.clone(),
            provider: "cbm".into(),
            coverage: coverage.into(),
            files,
            packet_tokens,
            symbols_found,
            retrieval,
        })
    }

    async fn fetch_snippet(
        &self,
        project: &str,
        qualified_name: &str,
    ) -> neuromesh_core::Result<String> {
        let result = self
            .client
            .call_tool(
                "get_code_snippet",
                json!({ "project": project, "qualified_name": qualified_name }),
            )
            .await?;
        Ok(tool_text(&result))
    }
}

fn build_search_request(project: &str, ctx: &ProxySearchContext, limit: u32) -> Value {
    let query = ctx.cbm_query_string();

    let mut args = json!({
        "project": project,
        "query": query,
        "limit": limit,
        "format": "json"
    });

    let semantic = ctx.cbm_semantic_terms();
    if !semantic.is_empty() {
        args["semantic_query"] = json!(semantic);
    }

    if !ctx.path_hints.is_empty() {
        if let Some(first) = ctx.path_hints.first() {
            args["file_pattern"] = json!(first);
        }
    }

    args
}

fn proxy_coverage_label(
    files: &[ProxyContextFile],
    limit: u32,
    retrieval: &ProxyRetrievalHints,
    ctx: &ProxySearchContext,
) -> &'static str {
    if files.is_empty() {
        return "no_seed_resolved";
    }
    let expected = ctx.expected_terms();
    if !expected.is_empty() && retrieval.matched_terms.is_empty() {
        return "partial";
    }
    if !expected.is_empty() && retrieval.confidence < 0.25 {
        return "partial";
    }
    if files.len() >= limit as usize {
        return "partial";
    }
    "bounded"
}

/// Skip Route nodes and other hits with no resolvable file path (phantom `unknown` files).
fn should_emit_hit(hit: &SearchHit) -> bool {
    if hit.path.is_empty() || hit.path == "unknown" {
        return false;
    }
    if hit.label.eq_ignore_ascii_case("Route") {
        return false;
    }
    if hit
        .qualified_name
        .as_ref()
        .is_some_and(|q| q.contains("__route__"))
    {
        return false;
    }
    true
}

struct SearchHit {
    path: String,
    label: String,
    qualified_name: Option<String>,
    summary: String,
}

fn parse_search_hits(result: &Value) -> Vec<SearchHit> {
    if let Some(structured) = tool_structured(result) {
        return parse_structured_search(&structured);
    }
    let text = tool_text(result);
    parse_tree_search(&text)
}

fn parse_structured_search(value: &Value) -> Vec<SearchHit> {
    let mut out = Vec::new();
    if let Some(groups) = value.get("groups").and_then(|g| g.as_array()) {
        for group in groups {
            let file = group
                .get("file")
                .or_else(|| group.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .replace('\\', "/");
            let prefix = group.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(rows) = group.get("rows").and_then(|r| r.as_array()) {
                for row in rows {
                    if let Some(hit) = row_from_json(row, &file, prefix) {
                        out.push(hit);
                    }
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(results) = value.get("results").and_then(|r| r.as_array()) {
            for row in results {
                if let Some(hit) = row_from_json(row, "unknown", "") {
                    out.push(hit);
                }
            }
        }
    }
    if out.is_empty() {
        if let (Some(cols), Some(rows)) = (
            value.get("cols").and_then(|c| c.as_array()),
            value.get("rows").and_then(|r| r.as_array()),
        ) {
            let colmap: Vec<&str> = cols.iter().filter_map(|c| c.as_str()).collect();
            let idx = |name: &str| colmap.iter().position(|c| *c == name);
            for row in rows {
                let cells: Vec<&Value> = row
                    .as_array()
                    .map(|a| a.iter().collect())
                    .unwrap_or_default();
                let cell = |name: &str| {
                    idx(name)
                        .and_then(|i| cells.get(i))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                };
                let name = cell("qn");
                let qn = if name.is_empty() { cell("name") } else { name };
                if qn.is_empty() {
                    continue;
                }
                let path = cell("file");
                let label = if cell("label").is_empty() {
                    "Symbol".to_string()
                } else {
                    cell("label").to_string()
                };
                let hit = SearchHit {
                    path: path.replace('\\', "/"),
                    label: label.clone(),
                    qualified_name: Some(qn.into()),
                    summary: format!("{label} {qn}"),
                };
                if should_emit_hit(&hit) {
                    out.push(hit);
                }
            }
        }
    }
    out
}

fn row_from_json(row: &Value, file: &str, prefix: &str) -> Option<SearchHit> {
    if let Some(obj) = row.as_object() {
        let name = obj
            .get("name")
            .or_else(|| obj.get("qn"))
            .and_then(|v| v.as_str())?;
        let label = obj
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("Symbol")
            .to_string();
        let path = obj
            .get("file")
            .and_then(|v| v.as_str())
            .unwrap_or(file)
            .replace('\\', "/");
        let qn = if !name.contains('.') && !prefix.is_empty() {
            format!("{prefix}.{name}")
        } else {
            name.to_string()
        };
        return Some(SearchHit {
            path,
            label: label.clone(),
            qualified_name: Some(qn.clone()),
            summary: format!("{label} {qn}"),
        });
    }
    if let Some(arr) = row.as_array() {
        let name = arr.first()?.as_str()?;
        let label = arr.get(1).and_then(|v| v.as_str()).unwrap_or("Symbol");
        let qn = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}.{name}")
        };
        return Some(SearchHit {
            path: file.to_string(),
            label: label.to_string(),
            qualified_name: Some(qn.clone()),
            summary: format!("{label} {qn}"),
        });
    }
    None
}

struct CbmProjectEntry {
    name: String,
    root_path: Option<String>,
}

fn paths_match(workspace: &Path, root_path: Option<&str>) -> bool {
    let Some(root) = root_path else {
        return false;
    };
    let ws = workspace
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let root = root.replace('\\', "/").to_ascii_lowercase();
    ws.trim_end_matches('/') == root.trim_end_matches('/')
}

fn parse_cbm_projects(result: &Option<Value>) -> Vec<CbmProjectEntry> {
    let Some(result) = result else {
        return Vec::new();
    };
    if let Some(structured) = tool_structured(result) {
        if let Some(arr) = structured.get("projects").and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| {
                    let name = v
                        .get("name")
                        .or_else(|| v.get("id"))
                        .and_then(|n| n.as_str())?
                        .to_string();
                    let root_path = v
                        .get("root_path")
                        .or_else(|| v.get("path"))
                        .and_then(|p| p.as_str())
                        .map(str::to_string);
                    Some(CbmProjectEntry { name, root_path })
                })
                .collect();
        }
    }
    parse_project_ids(&Some(result.clone()))
        .into_iter()
        .map(|name| CbmProjectEntry {
            name,
            root_path: None,
        })
        .collect()
}

fn parse_project_ids(result: &Option<Value>) -> Vec<String> {
    let Some(result) = result else {
        return Vec::new();
    };
    if let Some(structured) = tool_structured(result) {
        if let Some(arr) = structured
            .get("projects")
            .or_else(|| structured.get("available_projects"))
            .and_then(|v| v.as_array())
        {
            return arr
                .iter()
                .filter_map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .or_else(|| v.get("id").and_then(|id| id.as_str()).map(str::to_string))
                })
                .collect();
        }
    }
    let text = tool_text(result);
    if let Ok(v) = serde_json::from_str::<Value>(&text) {
        if let Some(arr) = v
            .get("available_projects")
            .or_else(|| v.get("projects"))
            .and_then(|a| a.as_array())
        {
            return arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
        }
    }
    Vec::new()
}

fn parse_tree_search(text: &str) -> Vec<SearchHit> {
    let mut out = Vec::new();
    let mut current_file = "unknown".to_string();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains('/') && !trimmed.contains(' ') {
            current_file = trimmed.replace('\\', "/");
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0];
            let label = parts[1];
            out.push(SearchHit {
                path: current_file.clone(),
                label: label.to_string(),
                qualified_name: Some(name.to_string()),
                summary: trimmed.to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tree_lines() {
        let text = "src/app.ts\nhandle Route 10 2\nmiddleware Function 5 1";
        let hits = parse_tree_search(text);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "src/app.ts");
    }

    #[test]
    fn paths_match_root() {
        assert!(paths_match(
            Path::new(r"C:\projects\neuromesh"),
            Some("C:/projects/neuromesh")
        ));
    }

    #[test]
    fn parse_cbm_cols_rows() {
        let value = serde_json::json!({
            "total": 2,
            "search_mode": "bm25",
            "cols": ["qn", "label", "file", "lines", "rank"],
            "rows": [
                ["express-corpus.lib.response.redirect", "Function", "lib/response.js", "819-870", -18.6],
                ["express-corpus.test.res.location.testRedirect", "Function", "test/res.location.js", "153-182", -15.3]
            ]
        });
        let hits = parse_structured_search(&value);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "lib/response.js");
        assert_eq!(
            hits[0].qualified_name.as_deref(),
            Some("express-corpus.lib.response.redirect")
        );
        assert_eq!(hits[1].path, "test/res.location.js");
    }

    #[test]
    fn skips_phantom_route_without_file() {
        let value = serde_json::json!({
            "cols": ["qn", "label", "file", "lines", "rank"],
            "rows": [
                ["express-corpus.__route__.GET /", "Route", "", "0-0", -12.1],
                ["express-corpus.lib.application.use", "Function", "lib/application.js", "210-240", -18.6]
            ]
        });
        let hits = parse_structured_search(&value);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "lib/application.js");
    }

    #[test]
    fn build_search_request_uses_extracted_terms_not_raw_prompt() {
        let ctx = ProxySearchContext {
            raw_prompt: "How does the system estimate how many tokens a file uses?".into(),
            client_keywords: vec!["estimate".into(), "tokens".into()],
            ..Default::default()
        };
        let args = build_search_request("neuromesh", &ctx, 8);
        let query = args["query"].as_str().unwrap();
        assert!(query.contains("estimate"));
        assert!(query.contains("tokens"));
        assert!(!query.to_lowercase().contains("how does"));
    }

    #[test]
    fn build_search_request_differs_for_distinct_nl_tasks() {
        let q1 = ProxySearchContext {
            raw_prompt: "How does the system estimate how many tokens a file or prompt uses?"
                .into(),
            client_keywords: vec!["estimate".into(), "tokens".into()],
            ..Default::default()
        };
        let q2 = ProxySearchContext {
            raw_prompt:
                "How does file importance get reinforced after being edited multiple times?".into(),
            client_keywords: vec!["importance".into(), "reinforced".into(), "edited".into()],
            ..Default::default()
        };
        let q3 = ProxySearchContext {
            raw_prompt:
                "How does the system decide the maximum number of files to index automatically?"
                    .into(),
            client_keywords: vec!["maximum".into(), "index".into(), "files".into()],
            ..Default::default()
        };
        let a1 = build_search_request("neuromesh", &q1, 8)["query"]
            .as_str()
            .unwrap()
            .to_string();
        let a2 = build_search_request("neuromesh", &q2, 8)["query"]
            .as_str()
            .unwrap()
            .to_string();
        let a3 = build_search_request("neuromesh", &q3, 8)["query"]
            .as_str()
            .unwrap()
            .to_string();
        assert_ne!(a1, a2);
        assert_ne!(a1, a3);
        assert_ne!(a2, a3);
        for query in [&a1, &a2, &a3] {
            assert!(!query.contains("is_how_does_ident"));
            assert!(!query.to_lowercase().contains("how does"));
        }
    }

    #[test]
    fn build_search_request_includes_keywords_and_semantic() {
        let ctx = ProxySearchContext {
            raw_prompt: "Explain middleware".into(),
            client_keywords: vec!["app.use".into(), "next".into()],
            client_expansion: vec!["pipeline".into(), "middleware".into()],
            related_concepts: vec!["middleware".into()],
            ..Default::default()
        };
        let args = build_search_request("express-corpus", &ctx, 8);
        assert!(args["query"].as_str().unwrap().contains("app.use"));
        assert_eq!(args["semantic_query"].as_array().map(|a| a.len()), Some(2));
    }

    #[test]
    fn fixture_express_search_parses_without_phantom_routes() {
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/cbm_search_express.json"
        ))
        .expect("fixture");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let hits = parse_structured_search(&value);
        assert_eq!(hits.len(), 2, "Route row with empty file must be dropped");
        assert!(hits.iter().all(|h| !h.path.is_empty()));
        assert!(hits.iter().any(|h| h.path.contains("application.js")));
    }
}
