use crate::calls::is_callable_name;
use crate::imports::{expand_rust_use, record_import};
use crate::types::{AstAnalysisResult, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use std::path::Path;
use tree_sitter::{Node, Parser, Tree};

struct Walk<'a> {
    src: &'a [u8],
    filename: &'a str,
    result: &'a mut AstAnalysisResult,
    current_fn: Option<String>,
    current_impl: Option<String>,
}

/// Tree-sitter Rust parse. Returns None if the grammar fails to load.
pub fn parse_rust(path: &Path, content: &str) -> Option<AstAnalysisResult> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(content, None)?;
    Some(walk_tree(path, content, &tree, true))
}

/// Tree-sitter TypeScript parse. Returns None if the grammar fails to load.
pub fn parse_typescript(path: &Path, content: &str) -> Option<AstAnalysisResult> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .ok()?;
    let tree = parser.parse(content, None)?;
    Some(walk_tree(path, content, &tree, false))
}

fn walk_tree(path: &Path, content: &str, tree: &Tree, is_rust: bool) -> AstAnalysisResult {
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let mut result = AstAnalysisResult::default();
    let mut ctx = Walk {
        src: content.as_bytes(),
        filename,
        result: &mut result,
        current_fn: None,
        current_impl: None,
    };
    visit(tree.root_node(), &mut ctx, is_rust);
    result.exports = result
        .symbols
        .iter()
        .filter(|s| s.exported)
        .map(|s| s.name.clone())
        .collect();
    attach_calls(&mut result);
    result
}

fn visit(node: Node, ctx: &mut Walk, is_rust: bool) {
    if is_rust && is_cfg_test_mod(node, ctx.src) {
        return;
    }
    match node.kind() {
        "function_item" | "function_declaration" | "method_definition" => {
            walk_function(node, ctx, is_rust);
            return;
        }
        "impl_item" if is_rust => {
            let prev = ctx.current_impl.clone();
            ctx.current_impl = type_name(node, ctx.src);
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    visit(child, ctx, is_rust);
                }
            }
            ctx.current_impl = prev;
            return;
        }
        "use_declaration" if is_rust => {
            let text = node.utf8_text(ctx.src).unwrap_or("");
            let spec = text.trim().trim_start_matches("pub ").trim_start_matches("use ");
            for (imported, full) in expand_rust_use(spec) {
                record_import(
                    ctx.result,
                    ctx.filename,
                    imported,
                    full,
                    node.start_position().row + 1,
                );
            }
        }
        "import_statement" if !is_rust => {
            collect_ts_import(node, ctx);
        }
        "call_expression" => {
            record_call(node, ctx);
        }
        "struct_item" | "enum_item" | "trait_item" | "type_item" | "class_declaration"
        | "interface_declaration" | "type_alias_declaration" => {
            if let Some(name) = field_text(node, "name", ctx.src) {
                let kind = match node.kind() {
                    "trait_item" | "type_item" | "interface_declaration" | "type_alias_declaration" => {
                        NodeType::Symbol
                    }
                    _ => NodeType::Class,
                };
                let exported = node.utf8_text(ctx.src).unwrap_or("").contains("pub ")
                    || node.utf8_text(ctx.src).unwrap_or("").contains("export ");
                ctx.result.symbols.push(ParsedSymbol {
                    name,
                    symbol_type: kind,
                    signature: Some(first_line(node, ctx.src)),
                    line_range: (node.start_position().row + 1)..(node.end_position().row + 2),
                    docstring: None,
                    exported,
                    parent: ctx.current_impl.clone(),
                    calls: Vec::new(),
                });
            }
        }
        _ => {}
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            visit(child, ctx, is_rust);
        }
    }
}

fn walk_function(node: Node, ctx: &mut Walk, is_rust: bool) {
    let name = field_text(node, "name", ctx.src)
        .or_else(|| field_text(node, "property", ctx.src))
        .unwrap_or_else(|| "anonymous".into());
    let exported = node.utf8_text(ctx.src).unwrap_or("").contains("pub ")
        || node.utf8_text(ctx.src).unwrap_or("").contains("export ");
    ctx.result.symbols.push(ParsedSymbol {
        name: name.clone(),
        symbol_type: NodeType::Function,
        signature: Some(first_line(node, ctx.src)),
        line_range: (node.start_position().row + 1)..(node.end_position().row + 2),
        docstring: None,
        exported,
        parent: ctx.current_impl.clone(),
        calls: Vec::new(),
    });
    let prev = ctx.current_fn.clone();
    ctx.current_fn = Some(name);
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            visit(child, ctx, is_rust);
        }
    }
    ctx.current_fn = prev;
}

fn record_call(node: Node, ctx: &mut Walk) {
    let Some(caller) = ctx.current_fn.as_deref() else {
        return;
    };
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let (name, receiver_hint) = call_target(func, ctx);
    if name.is_empty() || !is_callable_name(&name) || name == caller {
        return;
    }
    if ctx.result.relationships.iter().any(|rel| {
        rel.source_symbol == caller
            && rel.target_symbol == name
            && rel.relationship == EdgeType::Calls
    }) {
        return;
    }
    ctx.result.relationships.push(ParsedRelationship {
        source_symbol: caller.to_string(),
        target_symbol: name,
        relationship: EdgeType::Calls,
        target_file_hint: None,
        receiver_hint,
    });
}

fn call_target(func: Node, ctx: &Walk) -> (String, Option<String>) {
    match func.kind() {
        "identifier" => (text(func, ctx.src), None),
        "field_expression" => {
            let name = field_text(func, "field", ctx.src).unwrap_or_default();
            let recv = func.child_by_field_name("value");
            let hint = recv.and_then(|r| match r.kind() {
                "self" | "this" => ctx.current_impl.as_ref().map(|p| format!("impl:{p}")),
                "identifier" => {
                    let t = text(r, ctx.src);
                    if t.chars().next().is_some_and(|c| c.is_uppercase()) {
                        Some(format!("type:{t}"))
                    } else {
                        None
                    }
                }
                "field_expression" => {
                    let inner = r.child_by_field_name("value");
                    let field = field_text(r, "field", ctx.src);
                    match (inner.map(|n| n.kind()), field) {
                        (Some("self") | Some("this"), Some(field)) => {
                            Some(format!("field:{field}"))
                        }
                        _ => None,
                    }
                }
                _ => None,
            });
            (name, hint)
        }
        "scoped_identifier" => {
            let name = field_text(func, "name", ctx.src).unwrap_or_else(|| text(func, ctx.src));
            let path = func.child_by_field_name("path").map(|p| text(p, ctx.src));
            let hint = path.and_then(|p| {
                p.rsplit("::")
                    .next()
                    .filter(|last| last.chars().next().is_some_and(|c| c.is_uppercase()))
                    .map(|last| format!("type:{last}"))
            });
            (name, hint)
        }
        _ => (text(func, ctx.src), None),
    }
}

fn collect_ts_import(node: Node, ctx: &mut Walk) {
    let line = node.start_position().row + 1;
    let source = node
        .child_by_field_name("source")
        .map(|s| text(s, ctx.src).trim_matches(['"', '\'']).to_string())
        .unwrap_or_default();
    fn walk_names(n: Node, src: &[u8], names: &mut Vec<String>) {
        if n.kind() == "identifier" {
            names.push(text(n, src));
        }
        for i in 0..n.child_count() {
            if let Some(c) = n.child(i) {
                walk_names(c, src, names);
            }
        }
    }
    let mut names = Vec::new();
    walk_names(node, ctx.src, &mut names);
    names.retain(|n| n != "from" && n != "import");
    for imported in names {
        record_import(ctx.result, ctx.filename, imported, source.clone(), line);
    }
}

fn is_cfg_test_mod(node: Node, src: &[u8]) -> bool {
    if node.kind() != "mod_item" {
        return false;
    }
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if p.kind() == "attribute_item" || p.kind() == "inner_attribute_item" {
            if p.utf8_text(src)
                .unwrap_or("")
                .contains("test")
            {
                return true;
            }
        } else {
            break;
        }
        prev = p.prev_named_sibling();
    }
    false
}

fn type_name(node: Node, src: &[u8]) -> Option<String> {
    let ty = node.child_by_field_name("type")?;
    extract_ident(ty, src)
}

fn extract_ident(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" | "identifier" => Some(text(node, src)),
        "generic_type" => node
            .child_by_field_name("type")
            .and_then(|t| extract_ident(t, src)),
        _ => field_text(node, "name", src),
    }
}

fn field_text(node: Node, field: &str, src: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| text(n, src))
        .filter(|s| !s.is_empty())
}

fn text(node: Node, src: &[u8]) -> String {
    node.utf8_text(src).unwrap_or("").to_string()
}

fn first_line(node: Node, src: &[u8]) -> String {
    text(node, src)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn attach_calls(result: &mut AstAnalysisResult) {
    for rel in &result.relationships {
        if rel.relationship != EdgeType::Calls {
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
}
