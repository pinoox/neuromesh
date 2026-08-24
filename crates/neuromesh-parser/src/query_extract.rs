use crate::calls::is_callable_name;
use crate::imports::{expand_rust_use, record_import};
use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use std::path::Path;
use std::sync::OnceLock;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

pub const RUST_QUERIES: &str = include_str!("queries/rust.scm");
pub const TYPESCRIPT_QUERIES: &str = include_str!("queries/typescript.scm");

#[derive(Clone, Copy)]
pub enum Grammar {
    Rust,
    TypeScript,
}

#[derive(Clone, Copy)]
pub struct QueryOptions {
    pub rust_use: bool,
    pub skip_cfg_test: bool,
    pub ts_import: bool,
}

/// Parse with a grammar + query profile. Returns None if the grammar or query fails to load.
pub fn parse(
    path: &Path,
    content: &str,
    grammar: Grammar,
    query_src: &'static str,
    options: QueryOptions,
) -> Option<AstAnalysisResult> {
    let language = grammar.language();
    let query = compiled_query(grammar, &language, query_src)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(content, None)?;
    Some(extract(path, content, &tree, query, options))
}

impl Grammar {
    fn language(self) -> Language {
        match self {
            Grammar::Rust => tree_sitter_rust::LANGUAGE.into(),
            Grammar::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }
}

fn compiled_query(
    grammar: Grammar,
    language: &Language,
    source: &'static str,
) -> Option<&'static Query> {
    match grammar {
        Grammar::Rust => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            Q.get_or_init(|| Query::new(language, source).ok()).as_ref()
        }
        Grammar::TypeScript => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            Q.get_or_init(|| Query::new(language, source).ok()).as_ref()
        }
    }
}

struct FnHit<'a> {
    node: Node<'a>,
    name: String,
}

struct ImplHit<'a> {
    node: Node<'a>,
    type_name: Option<String>,
}

struct TypeHit<'a> {
    node: Node<'a>,
    name: String,
    kind: NodeType,
}

fn extract(
    path: &Path,
    content: &str,
    tree: &Tree,
    query: &Query,
    options: QueryOptions,
) -> AstAnalysisResult {
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let src = content.as_bytes();
    let mut result = AstAnalysisResult::default();

    let mut functions: Vec<FnHit<'_>> = Vec::new();
    let mut impls: Vec<ImplHit<'_>> = Vec::new();
    let mut types: Vec<TypeHit<'_>> = Vec::new();
    let mut import_nodes: Vec<Node<'_>> = Vec::new();
    let mut call_nodes: Vec<Node<'_>> = Vec::new();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), src);
    while let Some(m) = matches.next() {
        let mut function_node = None;
        let mut function_name = None;
        let mut impl_node = None;
        let mut impl_type = None;
        let mut type_node = None;
        let mut type_name = None;
        let mut type_kind = NodeType::Class;
        for cap in m.captures {
            match query.capture_names()[cap.index as usize] {
                "function" => function_node = Some(cap.node),
                "function.name" => function_name = Some(text(cap.node, src)),
                "impl" => impl_node = Some(cap.node),
                "impl.type" => impl_type = extract_ident(cap.node, src),
                "class" => {
                    type_node = Some(cap.node);
                    type_kind = NodeType::Class;
                }
                "class.name" => type_name = Some(text(cap.node, src)),
                "symbol" => {
                    type_node = Some(cap.node);
                    type_kind = NodeType::Symbol;
                }
                "symbol.name" => type_name = Some(text(cap.node, src)),
                "import" => import_nodes.push(cap.node),
                "call" => call_nodes.push(cap.node),
                _ => {}
            }
        }
        if let Some(node) = function_node {
            functions.push(FnHit {
                node,
                name: function_name.unwrap_or_else(|| "anonymous".into()),
            });
        }
        if let Some(node) = impl_node {
            impls.push(ImplHit {
                node,
                type_name: impl_type,
            });
        }
        if let (Some(node), Some(name)) = (type_node, type_name) {
            types.push(TypeHit {
                node,
                name,
                kind: type_kind,
            });
        }
    }

    if options.skip_cfg_test {
        functions.retain(|f| !in_cfg_test_mod(f.node, src));
        types.retain(|t| !in_cfg_test_mod(t.node, src));
        import_nodes.retain(|n| !in_cfg_test_mod(*n, src));
        call_nodes.retain(|n| !in_cfg_test_mod(*n, src));
        impls.retain(|i| !in_cfg_test_mod(i.node, src));
    }

    for ty in &types {
        result.symbols.push(ParsedSymbol {
            name: ty.name.clone(),
            symbol_type: ty.kind,
            signature: Some(first_line(ty.node, src)),
            line_range: line_range(ty.node),
            docstring: None,
            exported: is_exported(ty.node, src),
            parent: None,
            calls: Vec::new(),
        });
    }

    for func in &functions {
        let parent =
            innermost_impl(&impls, func.node.start_byte()).and_then(|imp| imp.type_name.clone());
        result.symbols.push(ParsedSymbol {
            name: func.name.clone(),
            symbol_type: NodeType::Function,
            signature: Some(first_line(func.node, src)),
            line_range: line_range(func.node),
            docstring: None,
            exported: is_exported(func.node, src),
            parent,
            calls: Vec::new(),
        });
    }

    for node in import_nodes {
        if options.rust_use {
            let spec = text(node, src)
                .trim()
                .trim_start_matches("pub ")
                .trim_start_matches("use ")
                .to_string();
            for (imported, full) in expand_rust_use(&spec) {
                record_import(
                    &mut result,
                    filename,
                    imported,
                    full,
                    node.start_position().row + 1,
                );
            }
        } else if options.ts_import {
            collect_ts_import(node, filename, src, &mut result);
        }
    }

    for call in call_nodes {
        let Some(caller) = innermost_fn(&functions, call.start_byte()) else {
            continue;
        };
        let impl_parent = innermost_impl(&impls, call.start_byte());
        record_call(
            call,
            &caller.name,
            impl_parent.and_then(|i| i.type_name.as_deref()),
            src,
            &mut result,
        );
    }

    result.exports = result
        .symbols
        .iter()
        .filter(|s| s.exported)
        .map(|s| s.name.clone())
        .collect();
    attach_calls(&mut result);
    result
}

fn innermost_fn<'a>(functions: &'a [FnHit<'a>], byte: usize) -> Option<&'a FnHit<'a>> {
    functions
        .iter()
        .filter(|f| f.node.start_byte() <= byte && byte < f.node.end_byte())
        .min_by_key(|f| f.node.end_byte() - f.node.start_byte())
}

fn innermost_impl<'a>(impls: &'a [ImplHit<'a>], byte: usize) -> Option<&'a ImplHit<'a>> {
    impls
        .iter()
        .filter(|i| i.node.start_byte() <= byte && byte < i.node.end_byte())
        .min_by_key(|i| i.node.end_byte() - i.node.start_byte())
}

fn record_call(
    node: Node,
    caller: &str,
    impl_parent: Option<&str>,
    src: &[u8],
    result: &mut AstAnalysisResult,
) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let (name, receiver_hint) = call_target(func, impl_parent, src);
    if name.is_empty() || !is_callable_name(&name) || name == caller {
        return;
    }
    if result.relationships.iter().any(|rel| {
        rel.source_symbol == caller
            && rel.target_symbol == name
            && rel.relationship == EdgeType::Calls
    }) {
        return;
    }
    result.relationships.push(crate::types::ParsedRelationship {
        source_symbol: caller.to_string(),
        target_symbol: name,
        relationship: EdgeType::Calls,
        target_file_hint: None,
        receiver_hint,
    });
}

fn call_target(func: Node, impl_parent: Option<&str>, src: &[u8]) -> (String, Option<String>) {
    match func.kind() {
        "identifier" => (text(func, src), None),
        "field_expression" | "member_expression" => {
            let name = field_text(func, "field", src)
                .or_else(|| field_text(func, "property", src))
                .unwrap_or_default();
            let recv = func
                .child_by_field_name("value")
                .or_else(|| func.child_by_field_name("object"));
            let hint = recv.and_then(|r| match r.kind() {
                "self" | "this" => impl_parent.map(|p| format!("impl:{p}")),
                "identifier" => {
                    let t = text(r, src);
                    if t.chars().next().is_some_and(|c| c.is_uppercase()) {
                        Some(format!("type:{t}"))
                    } else {
                        None
                    }
                }
                "field_expression" | "member_expression" => {
                    let inner = r
                        .child_by_field_name("value")
                        .or_else(|| r.child_by_field_name("object"));
                    let field =
                        field_text(r, "field", src).or_else(|| field_text(r, "property", src));
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
            let name = field_text(func, "name", src).unwrap_or_else(|| text(func, src));
            let path = func.child_by_field_name("path").map(|p| text(p, src));
            let hint = path.and_then(|p| {
                p.rsplit("::")
                    .next()
                    .filter(|last| last.chars().next().is_some_and(|c| c.is_uppercase()))
                    .map(|last| format!("type:{last}"))
            });
            (name, hint)
        }
        _ => (text(func, src), None),
    }
}

fn collect_ts_import(node: Node, filename: &str, src: &[u8], result: &mut AstAnalysisResult) {
    let line = node.start_position().row + 1;
    let source = node
        .child_by_field_name("source")
        .map(|s| text(s, src).trim_matches(['"', '\'']).to_string())
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
    walk_names(node, src, &mut names);
    names.retain(|n| n != "from" && n != "import");
    for imported in names {
        record_import(result, filename, imported, source.clone(), line);
    }
}

fn in_cfg_test_mod(mut node: Node, src: &[u8]) -> bool {
    loop {
        if is_cfg_test_mod(node, src) {
            return true;
        }
        match node.parent() {
            Some(parent) => node = parent,
            None => return false,
        }
    }
}

fn is_cfg_test_mod(node: Node, src: &[u8]) -> bool {
    if node.kind() != "mod_item" {
        return false;
    }
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if p.kind() == "attribute_item" || p.kind() == "inner_attribute_item" {
            if p.utf8_text(src).unwrap_or("").contains("test") {
                return true;
            }
        } else {
            break;
        }
        prev = p.prev_named_sibling();
    }
    false
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

fn is_exported(node: Node, src: &[u8]) -> bool {
    let t = node.utf8_text(src).unwrap_or("");
    t.contains("pub ") || t.contains("export ")
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

fn line_range(node: Node) -> std::ops::Range<usize> {
    (node.start_position().row + 1)..(node.end_position().row + 2)
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
