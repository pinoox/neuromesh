use crate::calls::{brace_delta, extract_calls_from_line};
use crate::imports::{expand_rust_use, record_import};
use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::NodeType;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct RustParser;

impl RustParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        static ITEM_RE: OnceLock<Regex> = OnceLock::new();
        static FN_RE: OnceLock<Regex> = OnceLock::new();
        static IMPL_RE: OnceLock<Regex> = OnceLock::new();
        static USE_RE: OnceLock<Regex> = OnceLock::new();

        let item_re = ITEM_RE.get_or_init(|| {
            Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(struct|enum|trait|type)\s+([A-Za-z0-9_]+)")
                .unwrap()
        });
        let fn_re = FN_RE.get_or_init(|| {
            Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:async\s+)?(?:unsafe\s+)?(?:const\s+)?fn\s+([A-Za-z0-9_]+)")
                .unwrap()
        });
        let impl_re = IMPL_RE.get_or_init(|| {
            Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?impl(?:\s*<[^>]+>)?\s+(?:(?:[A-Za-z0-9_:]+)\s+for\s+)?([A-Za-z0-9_]+)")
                .unwrap()
        });
        let use_re = USE_RE.get_or_init(|| Regex::new(r"^\s*(?:pub\s+)?use\s+(.+);?\s*$").unwrap());

        let mut current_fn: Option<String> = None;
        let mut current_impl: Option<String> = None;
        let mut fn_start_depth = 0i32;
        let mut impl_start_depth = 0i32;
        let mut depth = 0i32;
        let mut fn_line_start = 0usize;

        for (line_idx, line) in content.lines().enumerate() {
            let line_no = line_idx + 1;

            if let Some(cap) = use_re.captures(line) {
                if let Some(spec) = cap.get(1) {
                    for (imported, full) in expand_rust_use(spec.as_str()) {
                        record_import(&mut result, filename, imported, full, line_no);
                    }
                }
            }

            if let Some(cap) = impl_re.captures(line) {
                if !line.contains("fn ") {
                    current_impl = cap.get(1).map(|m| m.as_str().to_string());
                    impl_start_depth = depth;
                }
            }

            if let Some(cap) = item_re.captures(line) {
                let kind = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let name = cap
                    .get(2)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let node_type = match kind {
                    "trait" | "type" => NodeType::Symbol,
                    _ => NodeType::Class,
                };
                result.symbols.push(ParsedSymbol {
                    name,
                    symbol_type: node_type,
                    signature: Some(line.trim().to_string()),
                    line_range: line_no..(line_no + 1),
                    docstring: None,
                    exported: line.contains("pub"),
                    parent: None,
                    calls: Vec::new(),
                });
            }

            if let Some(cap) = fn_re.captures(line) {
                if let Some(fn_name) = cap.get(1) {
                    if let Some(prev) = current_fn.take() {
                        close_function(&mut result, &prev, fn_line_start, line_no);
                    }
                    let name = fn_name.as_str().to_string();
                    current_fn = Some(name.clone());
                    fn_start_depth = depth;
                    fn_line_start = line_no;
                    result.symbols.push(ParsedSymbol {
                        name,
                        symbol_type: NodeType::Function,
                        signature: Some(line.trim().to_string()),
                        line_range: line_no..(line_no + 1),
                        docstring: None,
                        exported: line.contains("pub"),
                        parent: current_impl.clone(),
                        calls: Vec::new(),
                    });
                }
            } else if let Some(caller) = current_fn.as_deref() {
                extract_calls_from_line(caller, line, &mut result);
            }

            depth += brace_delta(line);

            if current_fn.is_some() && depth <= fn_start_depth && line.contains('}') {
                if let Some(prev) = current_fn.take() {
                    close_function(&mut result, &prev, fn_line_start, line_no);
                }
            }
            if current_impl.is_some() && depth <= impl_start_depth && line.contains('}') {
                current_impl = None;
            }
        }

        if let Some(prev) = current_fn.take() {
            close_function(&mut result, &prev, fn_line_start, content.lines().count());
        }

        attach_calls(&mut result);
        result
    }
}

fn close_function(result: &mut AstAnalysisResult, name: &str, start: usize, end: usize) {
    if let Some(sym) = result
        .symbols
        .iter_mut()
        .rev()
        .find(|s| s.name == name && s.symbol_type == NodeType::Function)
    {
        sym.line_range = start..(end + 1);
    }
}

fn attach_calls(result: &mut AstAnalysisResult) {
    for rel in &result.relationships {
        if rel.relationship != neuromesh_core::EdgeType::Calls {
            continue;
        }
        if let Some(sym) = result
            .symbols
            .iter_mut()
            .rev()
            .find(|s| s.name == rel.source_symbol && s.symbol_type == NodeType::Function)
        {
            if !sym.calls.contains(&rel.target_symbol) {
                sym.calls.push(rel.target_symbol.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::EdgeType;
    use std::path::PathBuf;

    #[test]
    fn extracts_functions_imports_and_calls() {
        let code = r#"
use neuromesh_core::{NodeId, Result};
use neuromesh_task::TaskSignatureExtractor;

pub async fn handle_tool_call(&self, name: &str) -> Result<()> {
    let signature = TaskSignatureExtractor::extract("demo");
    QualityGate::evaluate(&signature, mode);
    self.activator.activate(&self.graph, &signature, mode);
    Ok(())
}

fn helper() {}
"#;
        let ast = RustParser::parse(&PathBuf::from("tools.rs"), code);
        assert!(ast.symbols.iter().any(|s| s.name == "handle_tool_call"));
        assert!(ast.imports.iter().any(|i| i.imported_symbols.contains(&"TaskSignatureExtractor".into())));
        assert!(ast.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "handle_tool_call"
                && r.target_symbol == "extract"
        }));
        assert!(ast.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "handle_tool_call"
                && (r.target_symbol == "activate" || r.target_symbol == "evaluate")
        }));
    }
}
