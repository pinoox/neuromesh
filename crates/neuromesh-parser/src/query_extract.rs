use crate::calls::{extract_type_uses_from_body, is_callable_name};
use crate::imports::{
    expand_rust_use, last_import_segment, normalize_module_hint, record_import, split_import_alias,
};
use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use std::cell::RefCell;
use std::path::Path;
use std::sync::OnceLock;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

pub const RUST_QUERIES: &str = include_str!("queries/rust.scm");
pub const TYPESCRIPT_QUERIES: &str = include_str!("queries/typescript.scm");
pub const PYTHON_QUERIES: &str = include_str!("queries/python.scm");
pub const GO_QUERIES: &str = include_str!("queries/go.scm");
pub const JAVA_QUERIES: &str = include_str!("queries/java.scm");
pub const KOTLIN_QUERIES: &str = include_str!("queries/kotlin.scm");
pub const PHP_QUERIES: &str = include_str!("queries/php.scm");
pub const CSHARP_QUERIES: &str = include_str!("queries/csharp.scm");
pub const DART_QUERIES: &str = include_str!("queries/dart.scm");
pub const SWIFT_QUERIES: &str = include_str!("queries/swift.scm");
pub const RUBY_QUERIES: &str = include_str!("queries/ruby.scm");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Grammar {
    Rust,
    TypeScript,
    Python,
    Go,
    Java,
    Kotlin,
    Php,
    CSharp,
    Dart,
    Swift,
    Ruby,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImportStyle {
    None,
    RustUse,
    TypeScript,
    Python,
    Go,
    Path,
    Ruby,
}

#[derive(Clone, Copy)]
pub enum ExportStyle {
    PubOrExport,
    NotUnderscore,
    InitialUpper,
    NotPrivate,
}

#[derive(Clone, Copy)]
pub struct QueryOptions {
    pub import: ImportStyle,
    pub export: ExportStyle,
    pub skip_cfg_test: bool,
    pub scan_type_uses: bool,
}

impl QueryOptions {
    pub fn rust() -> Self {
        Self {
            import: ImportStyle::RustUse,
            export: ExportStyle::PubOrExport,
            skip_cfg_test: true,
            scan_type_uses: false,
        }
    }

    pub fn typescript() -> Self {
        Self {
            import: ImportStyle::TypeScript,
            export: ExportStyle::PubOrExport,
            skip_cfg_test: false,
            scan_type_uses: false,
        }
    }

    pub fn python() -> Self {
        Self {
            import: ImportStyle::Python,
            export: ExportStyle::NotUnderscore,
            skip_cfg_test: false,
            scan_type_uses: false,
        }
    }

    pub fn go() -> Self {
        Self {
            import: ImportStyle::Go,
            export: ExportStyle::InitialUpper,
            skip_cfg_test: false,
            scan_type_uses: false,
        }
    }

    pub fn java() -> Self {
        Self {
            import: ImportStyle::Path,
            export: ExportStyle::NotPrivate,
            skip_cfg_test: false,
            scan_type_uses: true,
        }
    }

    pub fn kotlin() -> Self {
        Self {
            import: ImportStyle::Path,
            export: ExportStyle::NotPrivate,
            skip_cfg_test: false,
            scan_type_uses: true,
        }
    }

    pub fn php() -> Self {
        Self {
            import: ImportStyle::Path,
            export: ExportStyle::NotPrivate,
            skip_cfg_test: false,
            scan_type_uses: true,
        }
    }

    pub fn csharp() -> Self {
        Self {
            import: ImportStyle::Path,
            export: ExportStyle::NotPrivate,
            skip_cfg_test: false,
            scan_type_uses: true,
        }
    }

    pub fn dart() -> Self {
        Self {
            import: ImportStyle::Path,
            export: ExportStyle::NotUnderscore,
            skip_cfg_test: false,
            scan_type_uses: false,
        }
    }

    pub fn swift() -> Self {
        Self {
            import: ImportStyle::Path,
            export: ExportStyle::NotPrivate,
            skip_cfg_test: false,
            scan_type_uses: false,
        }
    }

    pub fn ruby() -> Self {
        Self {
            import: ImportStyle::Ruby,
            export: ExportStyle::NotUnderscore,
            skip_cfg_test: false,
            scan_type_uses: false,
        }
    }
}

thread_local! {
    static TS_PARSER: RefCell<(Parser, Option<Grammar>)> = RefCell::new((Parser::new(), None));
}

/// Parse with a grammar + query profile. Returns None if the grammar or query fails to load.
/// Reuses one tree-sitter `Parser` per thread so parallel ingest does not rebuild them.
pub fn parse(
    path: &Path,
    content: &str,
    grammar: Grammar,
    query_src: &'static str,
    options: QueryOptions,
) -> Option<AstAnalysisResult> {
    let language = grammar.language()?;
    let query = compiled_query(grammar, &language, query_src)?;
    TS_PARSER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let (parser, last) = &mut *slot;
        if *last != Some(grammar) {
            parser.set_language(&language).ok()?;
            *last = Some(grammar);
        }
        let tree = parser.parse(content, None)?;
        Some(extract(path, content, &tree, query, options))
    })
}

impl Grammar {
    pub(crate) fn language(self) -> Option<Language> {
        Some(match self {
            Grammar::Rust => tree_sitter_rust::LANGUAGE.into(),
            Grammar::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Grammar::Python => tree_sitter_python::LANGUAGE.into(),
            Grammar::Go => tree_sitter_go::LANGUAGE.into(),
            Grammar::Java => tree_sitter_java::LANGUAGE.into(),
            Grammar::Kotlin => tree_sitter_kotlin_sg::LANGUAGE.into(),
            Grammar::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Grammar::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Grammar::Dart => tree_sitter_dart_orchard::LANGUAGE.into(),
            Grammar::Swift => tree_sitter_swift::LANGUAGE.into(),
            Grammar::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        })
    }
}

fn compiled_query(
    grammar: Grammar,
    language: &Language,
    source: &'static str,
) -> Option<&'static Query> {
    fn load(
        slot: &'static OnceLock<Option<Query>>,
        language: &Language,
        source: &'static str,
    ) -> Option<&'static Query> {
        slot.get_or_init(|| match Query::new(language, source) {
            Ok(q) => Some(q),
            Err(err) => {
                tracing::warn!(error = %err, "tree-sitter query failed to compile");
                None
            }
        })
        .as_ref()
    }
    match grammar {
        Grammar::Rust => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::TypeScript => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Python => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Go => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Java => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Kotlin => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Php => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::CSharp => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Dart => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Swift => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
        Grammar::Ruby => {
            static Q: OnceLock<Option<Query>> = OnceLock::new();
            load(&Q, language, source)
        }
    }
}

struct FnHit<'a> {
    node: Node<'a>,
    end_byte: usize,
    name: String,
    parent_name: Option<String>,
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
        let mut function_parent = None;
        let mut impl_node = None;
        let mut impl_type = None;
        let mut type_node = None;
        let mut type_name = None;
        let mut type_kind = NodeType::Class;
        for cap in m.captures {
            match query.capture_names()[cap.index as usize] {
                "function" => function_node = Some(cap.node),
                "function.name" => function_name = Some(text(cap.node, src)),
                "function.parent" => function_parent = extract_ident(cap.node, src),
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
                end_byte: function_end_byte(node),
                name: function_name.unwrap_or_else(|| "anonymous".into()),
                parent_name: function_parent,
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
            exported: is_exported(ty.node, &ty.name, src, options.export),
            parent: None,
            calls: Vec::new(),
        });
    }

    for func in &functions {
        let parent = innermost_impl(&impls, func.node.start_byte())
            .and_then(|imp| imp.type_name.clone())
            .or_else(|| func.parent_name.clone())
            .or_else(|| innermost_type(&types, func.node.start_byte()).map(|ty| ty.name.clone()));
        result.symbols.push(ParsedSymbol {
            name: func.name.clone(),
            symbol_type: NodeType::Function,
            signature: Some(first_line(func.node, src)),
            line_range: line_range_bytes(func.node.start_byte(), func.end_byte, src),
            docstring: None,
            exported: is_exported(func.node, &func.name, src, options.export),
            parent,
            calls: Vec::new(),
        });
    }

    for node in import_nodes {
        match options.import {
            ImportStyle::None => {}
            ImportStyle::RustUse => {
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
            }
            ImportStyle::TypeScript => collect_ts_import(node, filename, src, &mut result),
            ImportStyle::Python => collect_python_import(node, filename, src, &mut result),
            ImportStyle::Go => collect_go_import(node, filename, src, &mut result),
            ImportStyle::Path => collect_path_import(node, filename, src, &mut result),
            ImportStyle::Ruby => {}
        }
    }
    if options.import == ImportStyle::Ruby {
        collect_ruby_requires(content, filename, &mut result);
    }

    for call in call_nodes {
        let Some(caller) = innermost_fn(&functions, call.start_byte()) else {
            continue;
        };
        let type_parent = innermost_type(&types, call.start_byte()).map(|ty| ty.name.clone());
        let parent = innermost_impl(&impls, call.start_byte())
            .and_then(|i| i.type_name.as_deref())
            .or(caller.parent_name.as_deref())
            .or(type_parent.as_deref());
        record_call(call, &caller.name, parent, src, &mut result);
    }

    if options.scan_type_uses {
        for func in &functions {
            extract_type_uses_from_body(&func.name, &text(func.node, src), &mut result);
        }
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
        .filter(|f| f.node.start_byte() <= byte && byte < f.end_byte)
        .min_by_key(|f| f.end_byte - f.node.start_byte())
}

/// Dart orchard (and similar) keep `function_signature` and `function_body` as
/// siblings. Extend the captured signature so calls in the body stay in-scope.
fn function_end_byte(node: Node) -> usize {
    let mut span = node;
    if node.kind() == "function_signature" {
        if let Some(parent) = node.parent() {
            if parent.kind() == "method_signature" {
                span = parent;
            }
        }
    }
    if let Some(next) = span.next_named_sibling() {
        if next.kind() == "function_body" {
            return next.end_byte();
        }
    }
    span.end_byte()
}

fn innermost_impl<'a>(impls: &'a [ImplHit<'a>], byte: usize) -> Option<&'a ImplHit<'a>> {
    impls
        .iter()
        .filter(|i| i.node.start_byte() <= byte && byte < i.node.end_byte())
        .min_by_key(|i| i.node.end_byte() - i.node.start_byte())
}

fn innermost_type<'a>(types: &'a [TypeHit<'a>], byte: usize) -> Option<&'a TypeHit<'a>> {
    types
        .iter()
        .filter(|t| {
            t.kind == NodeType::Class && t.node.start_byte() <= byte && byte < t.node.end_byte()
        })
        .min_by_key(|t| t.node.end_byte() - t.node.start_byte())
}

fn record_call(
    node: Node,
    caller: &str,
    impl_parent: Option<&str>,
    src: &[u8],
    result: &mut AstAnalysisResult,
) {
    let (name, receiver_hint) = call_name_and_hint(node, impl_parent, src);
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

fn call_name_and_hint(
    node: Node,
    impl_parent: Option<&str>,
    src: &[u8],
) -> (String, Option<String>) {
    if node.kind() == "object_creation_expression" || node.kind() == "constructor_invocation" {
        if let Some(ty) = node
            .child_by_field_name("type")
            .or_else(|| first_type_child(node))
        {
            let name = extract_ident(ty, src).unwrap_or_else(|| last_ident(ty, src));
            if !name.is_empty() {
                return (name.clone(), Some(format!("type:{name}")));
            }
        }
    }

    if node.kind() == "selector" {
        return dart_selector_call(node, impl_parent, src);
    }

    if let Some(method) = node.child_by_field_name("method") {
        if node.child_by_field_name("function").is_none() {
            let name = last_ident(method, src);
            let recv = node.child_by_field_name("receiver");
            return (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            );
        }
    }

    if let Some(name_node) = node.child_by_field_name("name") {
        if node.child_by_field_name("function").is_none() {
            let name = last_ident(name_node, src);
            let recv = node
                .child_by_field_name("object")
                .or_else(|| node.child_by_field_name("receiver"))
                .or_else(|| node.child_by_field_name("scope"));
            return (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            );
        }
    }

    if let Some(func) = node
        .child_by_field_name("function")
        .or_else(|| node.child_by_field_name("method"))
        .or_else(|| callee_child(node))
    {
        return call_target(func, impl_parent, src);
    }

    (String::new(), None)
}

fn dart_selector_call(
    node: Node,
    impl_parent: Option<&str>,
    src: &[u8],
) -> (String, Option<String>) {
    let method = (0..node.named_child_count())
        .filter_map(|i| node.named_child(i))
        .find(|c| {
            matches!(
                c.kind(),
                "unconditional_assignable_selector" | "conditional_assignable_selector"
            )
        });
    if let Some(method) = method {
        let name = last_ident(method, src);
        let hint = node
            .prev_named_sibling()
            .and_then(|r| hint_from_receiver(r, impl_parent, src));
        return (name, hint);
    }
    let name = node
        .prev_named_sibling()
        .map(|n| last_ident(n, src))
        .unwrap_or_default();
    (name, None)
}

fn callee_child(node: Node) -> Option<Node> {
    for i in 0..node.named_child_count() {
        let child = node.named_child(i)?;
        match child.kind() {
            "call_suffix" | "argument_list" | "arguments" | "value_arguments" => continue,
            _ => return Some(child),
        }
    }
    None
}

fn first_type_child(node: Node) -> Option<Node> {
    for i in 0..node.named_child_count() {
        let child = node.named_child(i)?;
        match child.kind() {
            "type_identifier" | "identifier" | "user_type" | "generic_type" | "qualified_name"
            | "name" => return Some(child),
            _ => {}
        }
    }
    None
}

fn call_target(func: Node, impl_parent: Option<&str>, src: &[u8]) -> (String, Option<String>) {
    match func.kind() {
        "identifier"
        | "field_identifier"
        | "simple_identifier"
        | "name"
        | "type_identifier"
        | "property_identifier" => (text(func, src), None),
        "attribute" => {
            let name = field_text(func, "attr", src)
                .or_else(|| field_text(func, "attribute", src))
                .unwrap_or_else(|| last_ident(func, src));
            let recv = func.child_by_field_name("object");
            (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            )
        }
        "selector_expression" => {
            let name = field_text(func, "field", src).unwrap_or_else(|| last_ident(func, src));
            let recv = func.child_by_field_name("operand");
            (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            )
        }
        "navigation_expression" => {
            let name = navigation_name(func, src);
            let recv = func.named_child(0);
            (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            )
        }
        "unconditional_assignable_selector" | "conditional_assignable_selector" => {
            let name = last_ident(func, src);
            let recv = func.named_child(0).filter(|n| last_ident(*n, src) != name);
            (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            )
        }
        "field_expression" | "member_expression" | "member_access_expression" => {
            let name = field_text(func, "field", src)
                .or_else(|| field_text(func, "property", src))
                .or_else(|| field_text(func, "name", src))
                .unwrap_or_default();
            let recv = func
                .child_by_field_name("value")
                .or_else(|| func.child_by_field_name("object"))
                .or_else(|| func.child_by_field_name("expression"));
            (
                name,
                recv.and_then(|r| hint_from_receiver(r, impl_parent, src)),
            )
        }
        "scoped_identifier" | "qualified_name" | "scoped_type_identifier" => {
            let name = field_text(func, "name", src).unwrap_or_else(|| last_ident(func, src));
            let path = func
                .child_by_field_name("path")
                .or_else(|| func.named_child(0));
            let hint = path.and_then(|p| hint_from_receiver(p, impl_parent, src));
            (name, hint)
        }
        _ => {
            if let Some(inner) = func.named_child(0) {
                if inner.kind() != func.kind() {
                    return call_target(inner, impl_parent, src);
                }
            }
            (last_ident(func, src), None)
        }
    }
}

fn navigation_name(node: Node, src: &[u8]) -> String {
    for i in (0..node.named_child_count()).rev() {
        if let Some(child) = node.named_child(i) {
            if child.kind() == "navigation_suffix" {
                return last_ident(child, src);
            }
        }
    }
    last_ident(node, src)
}

fn hint_from_receiver(recv: Node, impl_parent: Option<&str>, src: &[u8]) -> Option<String> {
    match recv.kind() {
        "self" | "this" | "this_expression" | "super_expression" => {
            impl_parent.map(|p| format!("impl:{p}"))
        }
        "identifier" | "simple_identifier" | "name" | "type_identifier" | "package_identifier"
        | "field_identifier" | "constant" => {
            let t = text(recv, src);
            if matches!(t.as_str(), "self" | "this" | "Super" | "super" | "Me") {
                impl_parent.map(|p| format!("impl:{p}"))
            } else if t.chars().next().is_some_and(|c| c.is_uppercase()) {
                Some(format!("type:{t}"))
            } else {
                None
            }
        }
        "field_expression" | "member_expression" => {
            let inner = recv
                .child_by_field_name("value")
                .or_else(|| recv.child_by_field_name("object"));
            let field =
                field_text(recv, "field", src).or_else(|| field_text(recv, "property", src));
            match (inner.map(|n| n.kind()), field) {
                (Some("self") | Some("this") | Some("this_expression"), Some(field)) => {
                    Some(format!("field:{field}"))
                }
                _ => inner.and_then(|n| hint_from_receiver(n, impl_parent, src)),
            }
        }
        "attribute" => recv
            .child_by_field_name("object")
            .and_then(|n| hint_from_receiver(n, impl_parent, src)),
        "selector_expression" => recv
            .child_by_field_name("operand")
            .and_then(|n| hint_from_receiver(n, impl_parent, src)),
        "navigation_expression" => recv
            .named_child(0)
            .and_then(|n| hint_from_receiver(n, impl_parent, src)),
        "pointer_type"
        | "parenthesized_expression"
        | "parenthesized_type"
        | "user_type"
        | "generic_type"
        | "nullable_type" => recv
            .child_by_field_name("type")
            .or_else(|| recv.named_child(0))
            .and_then(|n| hint_from_receiver(n, impl_parent, src)),
        _ => recv
            .named_child(0)
            .and_then(|n| hint_from_receiver(n, impl_parent, src)),
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

fn collect_python_import(node: Node, filename: &str, src: &[u8], result: &mut AstAnalysisResult) {
    let line = node.start_position().row + 1;
    let raw = text(node, src);
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("from ") {
        if let Some((module, names)) = rest.split_once(" import ") {
            let hint = normalize_module_hint(module.trim());
            for part in names.split(',') {
                let (imported, alias) = split_import_alias(part);
                if imported == "*" {
                    continue;
                }
                let name = alias.unwrap_or(imported);
                record_import(result, filename, name, hint.clone(), line);
            }
        }
        return;
    }
    if let Some(rest) = trimmed.strip_prefix("import ") {
        for part in rest.split(',') {
            let (path, alias) = split_import_alias(part);
            let imported = alias.unwrap_or_else(|| last_import_segment(&path));
            record_import(
                result,
                filename,
                imported,
                normalize_module_hint(&path),
                line,
            );
        }
    }
}

fn collect_go_import(node: Node, filename: &str, src: &[u8], result: &mut AstAnalysisResult) {
    let line = node.start_position().row + 1;
    let raw = text(node, src).trim().to_string();
    if raw.is_empty() {
        return;
    }
    let (alias, path) = if raw.starts_with('"') {
        (None, raw.trim_matches('"').to_string())
    } else if let Some((head, rest)) = raw.split_once(char::is_whitespace) {
        let path = rest.trim().trim_matches('"').to_string();
        if head == "_" || head == "." {
            (None, path)
        } else {
            (Some(head.to_string()), path)
        }
    } else {
        (None, raw.trim_matches('"').to_string())
    };
    let imported = alias.unwrap_or_else(|| last_import_segment(&path));
    if imported.is_empty() {
        return;
    }
    record_import(
        result,
        filename,
        imported,
        normalize_module_hint(&path),
        line,
    );
}

fn collect_path_import(node: Node, filename: &str, src: &[u8], result: &mut AstAnalysisResult) {
    let line = node.start_position().row + 1;
    let mut spec = text(node, src).trim().trim_end_matches(';').to_string();
    for prefix in [
        "import static",
        "require_relative",
        "import",
        "using",
        "use",
        "require",
    ] {
        if let Some(rest) = spec.strip_prefix(prefix) {
            spec = rest.trim().to_string();
            break;
        }
    }
    if spec.ends_with('*') {
        return;
    }
    let (path, alias) = split_import_alias(&spec);
    let imported = alias.unwrap_or_else(|| last_import_segment(&path));
    if imported.is_empty() {
        return;
    }
    record_import(
        result,
        filename,
        imported,
        normalize_module_hint(&path),
        line,
    );
}

fn collect_ruby_requires(content: &str, filename: &str, result: &mut AstAnalysisResult) {
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let rest = if let Some(rest) = trimmed.strip_prefix("require_relative ") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("require ") {
            rest
        } else {
            continue;
        };
        let spec = rest.trim().trim_end_matches(';');
        let imported = last_import_segment(spec);
        if imported.is_empty() {
            continue;
        }
        record_import(
            result,
            filename,
            imported,
            normalize_module_hint(spec),
            idx + 1,
        );
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
        "type_identifier" | "identifier" | "simple_identifier" | "name" | "field_identifier"
        | "package_identifier" => Some(text(node, src)),
        "generic_type" | "pointer_type" | "slice_type" | "array_type" | "channel_type"
        | "nullable_type" | "user_type" => node
            .child_by_field_name("type")
            .or_else(|| node.named_child(0))
            .and_then(|t| extract_ident(t, src)),
        "qualified_type" | "qualified_name" => {
            field_text(node, "name", src).or_else(|| Some(last_ident(node, src)))
        }
        _ => field_text(node, "name", src).or_else(|| {
            let ident = last_ident(node, src);
            if ident.is_empty() {
                None
            } else {
                Some(ident)
            }
        }),
    }
}

fn is_exported(node: Node, name: &str, src: &[u8], style: ExportStyle) -> bool {
    match style {
        ExportStyle::PubOrExport => {
            let t = first_line(node, src);
            t.contains("pub ") || t.contains("export ") || t.contains("public ")
        }
        ExportStyle::NotUnderscore => !name.starts_with('_'),
        ExportStyle::InitialUpper => name.chars().next().is_some_and(|c| c.is_uppercase()),
        ExportStyle::NotPrivate => {
            let t = first_line(node, src);
            !t.contains("private ") && !t.contains("protected ") && !t.contains("internal ")
        }
    }
}

fn last_ident(node: Node, src: &[u8]) -> String {
    match node.kind() {
        "identifier"
        | "field_identifier"
        | "simple_identifier"
        | "type_identifier"
        | "name"
        | "property_identifier"
        | "package_identifier"
        | "constant" => return text(node, src),
        _ => {}
    }
    let mut found = String::new();
    fn walk(n: Node, src: &[u8], found: &mut String) {
        match n.kind() {
            "identifier"
            | "field_identifier"
            | "simple_identifier"
            | "type_identifier"
            | "name"
            | "property_identifier"
            | "package_identifier"
            | "constant" => {
                *found = text(n, src);
            }
            _ => {
                for i in 0..n.named_child_count() {
                    if let Some(c) = n.named_child(i) {
                        walk(c, src, found);
                    }
                }
            }
        }
    }
    walk(node, src, &mut found);
    found
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

/// Inclusive start .. exclusive end from a byte span (tree-sitter `end_byte` is exclusive).
fn line_range_bytes(start_byte: usize, end_byte: usize, src: &[u8]) -> std::ops::Range<usize> {
    let start_line = byte_to_line(start_byte, src);
    let last_line = byte_to_line(
        end_byte.saturating_sub(1).min(src.len().saturating_sub(1)),
        src,
    )
    .max(start_line);
    start_line..(last_line + 1)
}

fn byte_to_line(byte: usize, src: &[u8]) -> usize {
    src.get(..byte.min(src.len()))
        .map(|head| head.iter().filter(|b| **b == b'\n').count() + 1)
        .unwrap_or(1)
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

    fn parse_lang(
        grammar: Grammar,
        queries: &'static str,
        options: QueryOptions,
        path: &str,
        code: &str,
    ) -> AstAnalysisResult {
        parse(&PathBuf::from(path), code, grammar, queries, options)
            .unwrap_or_else(|| panic!("query extract failed for {path}"))
    }

    #[test]
    fn python_class_method_import_and_typed_call() {
        let store = r#"
class SmsStore:
    def save(self, body):
        self.persist(body)
    def persist(self, body):
        return body
"#;
        let ast = parse_lang(
            Grammar::Python,
            PYTHON_QUERIES,
            QueryOptions::python(),
            "sms_store.py",
            store,
        );
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "SmsStore" && s.symbol_type == NodeType::Class));
        let save = ast.symbols.iter().find(|s| s.name == "save").expect("save");
        assert_eq!(save.parent.as_deref(), Some("SmsStore"));
        assert!(save.calls.iter().any(|c| c == "persist"));

        let recv = r#"
from sms_store import SmsStore
def on_receive(body):
    SmsStore.save(body)
"#;
        let ast = parse_lang(
            Grammar::Python,
            PYTHON_QUERIES,
            QueryOptions::python(),
            "receiver.py",
            recv,
        );
        assert!(ast
            .imports
            .iter()
            .any(|i| i.imported_symbols.contains(&"SmsStore".into())));
        assert!(ast.relationships.iter().any(|r| {
            r.source_symbol == "on_receive"
                && r.target_symbol == "save"
                && r.receiver_hint.as_deref() == Some("type:SmsStore")
        }));
    }

    #[test]
    fn kotlin_object_call_and_import() {
        let store = r#"
object SmsStore {
    fun save(body: String?) {
        persist(body)
    }
    private fun persist(body: String?) {}
}
"#;
        let ast = parse_lang(
            Grammar::Kotlin,
            KOTLIN_QUERIES,
            QueryOptions::kotlin(),
            "SmsStore.kt",
            store,
        );
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "SmsStore" && s.symbol_type == NodeType::Class));
        let save = ast.symbols.iter().find(|s| s.name == "save").expect("save");
        assert_eq!(save.parent.as_deref(), Some("SmsStore"));
        assert!(save.calls.iter().any(|c| c == "persist"));

        let recv = r#"
import com.example.app.SmsStore
class SmsReceiver {
    fun onReceive(intent: Intent) {
        SmsStore.save(intent.getStringExtra("sms"))
    }
}
"#;
        let ast = parse_lang(
            Grammar::Kotlin,
            KOTLIN_QUERIES,
            QueryOptions::kotlin(),
            "SmsReceiver.kt",
            recv,
        );
        assert!(ast
            .imports
            .iter()
            .any(|i| i.imported_symbols.contains(&"SmsStore".into())));
        assert!(ast.relationships.iter().any(|r| {
            r.source_symbol == "onReceive"
                && r.target_symbol == "save"
                && r.receiver_hint.as_deref() == Some("type:SmsStore")
        }));
    }

    #[test]
    fn go_method_receiver_is_parent() {
        let code = r#"
package sms
type Store struct{}
func (s *Store) Save(body string) { s.persist(body) }
func (s *Store) persist(body string) {}
"#;
        let ast = parse_lang(
            Grammar::Go,
            GO_QUERIES,
            QueryOptions::go(),
            "store.go",
            code,
        );
        let save = ast.symbols.iter().find(|s| s.name == "Save").expect("Save");
        assert_eq!(save.parent.as_deref(), Some("Store"));
        assert!(save.exported);
        let persist = ast
            .symbols
            .iter()
            .find(|s| s.name == "persist")
            .expect("persist");
        assert!(!persist.exported);
    }

    #[test]
    fn java_and_php_queries_compile() {
        let java = parse_lang(
            Grammar::Java,
            JAVA_QUERIES,
            QueryOptions::java(),
            "SmsStore.java",
            "class SmsStore { void save(String body) { persist(body); } void persist(String body) {} }",
        );
        assert!(java.symbols.iter().any(|s| s.name == "SmsStore"));
        assert!(java.symbols.iter().any(|s| s.name == "save"));

        let php = parse_lang(
            Grammar::Php,
            PHP_QUERIES,
            QueryOptions::php(),
            "Store.php",
            "<?php class Store { public function save($body) { $this->persist($body); } private function persist($body) {} }",
        );
        assert!(php.symbols.iter().any(|s| s.name == "Store"));
        let save = php.symbols.iter().find(|s| s.name == "save").expect("save");
        assert_eq!(save.parent.as_deref(), Some("Store"));
        assert!(save.calls.iter().any(|c| c == "persist"));
    }

    #[test]
    fn php_throw_rethrow_and_ternary_new_are_inbound() {
        let php = parse_lang(
            Grammar::Php,
            PHP_QUERIES,
            QueryOptions::php(),
            "Matcher.php",
            r#"<?php
class RedirectableUrlMatcher {
    public function match(string $pathinfo): array {
        try {
            return parent::match($pathinfo);
        } catch (ResourceNotFoundException $e) {
            if ($pathinfo === '/') {
                throw $e;
            }
            throw 0 < count($this->allow)
                ? new MethodNotAllowedException()
                : new ResourceNotFoundException('no routes');
        }
    }
}
"#,
        );
        for expected in ["ResourceNotFoundException", "MethodNotAllowedException"] {
            assert!(
                php.relationships
                    .iter()
                    .any(|r| { r.source_symbol == "match" && r.target_symbol == expected }),
                "php query extract missing {expected}: {:?}",
                php.relationships
                    .iter()
                    .map(|r| format!("{}→{}", r.source_symbol, r.target_symbol))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn wave2_queries_extract_typed_save() {
        for (grammar, queries, label) in [
            (Grammar::Dart, DART_QUERIES, "dart"),
            (Grammar::CSharp, CSHARP_QUERIES, "csharp"),
            (Grammar::Swift, SWIFT_QUERIES, "swift"),
            (Grammar::Ruby, RUBY_QUERIES, "ruby"),
        ] {
            let language = grammar
                .language()
                .unwrap_or_else(|| panic!("{label} language"));
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&language)
                .unwrap_or_else(|e| panic!("{label} set_language: {e}"));
            tree_sitter::Query::new(&language, queries)
                .unwrap_or_else(|e| panic!("{label} query: {e}"));
        }
        let dart = parse_lang(
            Grammar::Dart,
            DART_QUERIES,
            QueryOptions::dart(),
            "sms_store.dart",
            "class SmsStore {\n  void save(String body) { persist(body); }\n  void persist(String body) {}\n}\n",
        );
        let save = dart
            .symbols
            .iter()
            .find(|s| s.name == "save")
            .expect("save");
        assert_eq!(save.parent.as_deref(), Some("SmsStore"));
        assert!(save.calls.iter().any(|c| c == "persist"));

        let recv = parse_lang(
            Grammar::Dart,
            DART_QUERIES,
            QueryOptions::dart(),
            "receiver.dart",
            "import 'sms_store.dart';\nvoid onReceive(String body) { SmsStore.save(body); }\n",
        );
        assert!(recv
            .imports
            .iter()
            .any(|i| i.imported_symbols.iter().any(|n| n == "sms_store")));
        assert!(
            recv.relationships.iter().any(|r| {
                r.source_symbol == "onReceive"
                    && r.target_symbol == "save"
                    && r.receiver_hint.as_deref() == Some("type:SmsStore")
            }),
            "dart SmsStore.save hint missing: {:?}",
            recv.relationships
        );

        let cs = parse_lang(
            Grammar::CSharp,
            CSHARP_QUERIES,
            QueryOptions::csharp(),
            "SmsStore.cs",
            "class SmsStore { public static void Save(string body) { Persist(body); } static void Persist(string body) {} }",
        );
        assert!(cs.symbols.iter().any(|s| s.name == "SmsStore"));
        assert!(cs.symbols.iter().any(|s| s.name == "Save"));

        let swift = parse_lang(
            Grammar::Swift,
            SWIFT_QUERIES,
            QueryOptions::swift(),
            "SmsStore.swift",
            "class SmsStore {\n  func save(body: String?) { persist(body: body) }\n  private func persist(body: String?) {}\n}\n",
        );
        let save = swift
            .symbols
            .iter()
            .find(|s| s.name == "save")
            .expect("swift save");
        assert_eq!(save.parent.as_deref(), Some("SmsStore"));

        let ruby = parse_lang(
            Grammar::Ruby,
            RUBY_QUERIES,
            QueryOptions::ruby(),
            "receiver.rb",
            "require_relative 'sms_store'\nclass SmsReceiver\n  def on_receive(body)\n    SmsStore.save(body)\n  end\nend\n",
        );
        assert!(ruby
            .imports
            .iter()
            .any(|i| i.imported_symbols.contains(&"sms_store".into())));
        assert!(ruby.relationships.iter().any(|r| {
            r.source_symbol == "on_receive"
                && r.target_symbol == "save"
                && r.receiver_hint.as_deref() == Some("type:SmsStore")
        }));
    }

    #[test]
    fn dart_and_kotlin_spans_cover_the_function_body() {
        let dart = parse_lang(
            Grammar::Dart,
            DART_QUERIES,
            QueryOptions::dart(),
            "sms_store.dart",
            "void save(String body) {\n  persist(body);\n  keep(body);\n}\nvoid persist(String body) {}\n",
        );
        let save = dart
            .symbols
            .iter()
            .find(|s| s.name == "save")
            .expect("save");
        assert!(
            save.line_range.end - save.line_range.start >= 3,
            "dart save should include the body, got {:?}",
            save.line_range
        );
        assert!(save.calls.iter().any(|c| c == "persist"));

        let kt = parse_lang(
            Grammar::Kotlin,
            KOTLIN_QUERIES,
            QueryOptions::kotlin(),
            "SmsStore.kt",
            "fun save(body: String?) {\n  persist(body)\n  keep(body)\n}\nfun persist(body: String?) {}\n",
        );
        let save = kt.symbols.iter().find(|s| s.name == "save").expect("save");
        assert!(
            save.line_range.end - save.line_range.start >= 3,
            "kotlin save should include the body, got {:?}",
            save.line_range
        );
    }

    #[test]
    fn typescript_arrow_const_is_a_function() {
        let ast = parse_lang(
            Grammar::TypeScript,
            TYPESCRIPT_QUERIES,
            QueryOptions::typescript(),
            "store.ts",
            "export const saveSms = (body: string) => {\n  persist(body);\n  return body;\n};\nfunction persist(body: string) {}\n",
        );
        let save = ast
            .symbols
            .iter()
            .find(|s| s.name == "saveSms")
            .expect("saveSms");
        assert_eq!(save.symbol_type, NodeType::Function);
        assert!(
            save.line_range.end - save.line_range.start >= 3,
            "arrow body span {:?}",
            save.line_range
        );
        assert!(save.calls.iter().any(|c| c == "persist"));
    }
}
