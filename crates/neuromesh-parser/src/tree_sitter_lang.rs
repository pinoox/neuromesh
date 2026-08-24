use crate::query_extract::{self, Grammar, QueryOptions, RUST_QUERIES, TYPESCRIPT_QUERIES};
use crate::types::AstAnalysisResult;
use std::path::Path;

/// Tree-sitter Rust parse. Returns None if the grammar or query fails to load.
pub fn parse_rust(path: &Path, content: &str) -> Option<AstAnalysisResult> {
    query_extract::parse(
        path,
        content,
        Grammar::Rust,
        RUST_QUERIES,
        QueryOptions {
            rust_use: true,
            skip_cfg_test: true,
            ts_import: false,
        },
    )
}

/// Tree-sitter TypeScript parse. Returns None if the grammar or query fails to load.
pub fn parse_typescript(path: &Path, content: &str) -> Option<AstAnalysisResult> {
    query_extract::parse(
        path,
        content,
        Grammar::TypeScript,
        TYPESCRIPT_QUERIES,
        QueryOptions {
            rust_use: false,
            skip_cfg_test: false,
            ts_import: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rust_calls_stay_inside_function() {
        let code = r#"
pub struct Handler;
impl Handler {
    pub async fn handle_tool_call(&self, name: &str) {
        TaskSignatureExtractor::extract("demo");
        self.activator.activate(&self.graph);
    }
    pub fn search_symbols(&self, q: &str) {
        self.graph.search_symbols(q, 10);
    }
}
"#;
        let ast = parse_rust(&PathBuf::from("tools.rs"), code).expect("tree-sitter rust");
        let handle = ast
            .symbols
            .iter()
            .find(|s| s.name == "handle_tool_call")
            .expect("handle_tool_call");
        assert!(handle.calls.iter().any(|c| c == "extract"));
        assert!(handle.calls.iter().any(|c| c == "activate"));
        assert!(
            !handle.calls.iter().any(|c| c == "search_symbols"),
            "search_symbols leaked into handle_tool_call: {:?}",
            handle.calls
        );
        assert_eq!(handle.parent.as_deref(), Some("Handler"));
        let search = ast
            .symbols
            .iter()
            .find(|s| s.name == "search_symbols")
            .expect("search_symbols fn");
        assert_eq!(search.parent.as_deref(), Some("Handler"));
        assert!(search.line_range.start > handle.line_range.start);
        assert!(
            ast.relationships.iter().any(|r| {
                r.source_symbol == "handle_tool_call"
                    && r.target_symbol == "activate"
                    && r.receiver_hint.as_deref() == Some("field:activator")
            }),
            "self.activator.activate should carry a field hint: {:?}",
            ast.relationships
        );
    }

    #[test]
    fn typescript_import_and_call_stay_scoped() {
        let lib = "export function extractIntent() { return 1; }\n";
        let app =
            "import { extractIntent } from './lib';\nexport function run() { extractIntent(); }\n";
        let lib_ast =
            parse_typescript(&PathBuf::from("lib.ts"), lib).expect("tree-sitter typescript");
        assert!(lib_ast.symbols.iter().any(|s| s.name == "extractIntent"));
        let app_ast =
            parse_typescript(&PathBuf::from("app.ts"), app).expect("tree-sitter typescript");
        assert!(app_ast
            .imports
            .iter()
            .any(|i| i.imported_symbols.iter().any(|n| n == "extractIntent")));
        let run = app_ast
            .symbols
            .iter()
            .find(|s| s.name == "run")
            .expect("run");
        assert!(run.calls.iter().any(|c| c == "extractIntent"));
    }
}
