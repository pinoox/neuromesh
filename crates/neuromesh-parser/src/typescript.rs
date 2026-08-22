use crate::calls::{brace_delta, extract_calls_from_line};
use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;

pub struct TypeScriptParser;

impl TypeScriptParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        // 1. Extract interfaces & types
        let type_regex =
            Regex::new(r#"(?m)^\s*(?:export\s+)?(?:interface|type)\s+([A-Za-z0-9_]+)"#).unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = type_regex.captures(line) {
                if let Some(type_name) = cap.get(1) {
                    result.symbols.push(ParsedSymbol::new(
                        type_name.as_str(),
                        NodeType::Symbol,
                        Some(line.trim().to_string()),
                        (line_idx + 1)..(line_idx + 2),
                        line.contains("export"),
                    ));
                }
            }
        }

        // 2. Extract functions, composables & classes
        let fn_regex = Regex::new(
            r#"(?m)^\s*(?:export\s+)?(?:async\s+)?(?:function\s+([A-Za-z0-9_]+)|const\s+([A-Za-z0-9_]+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>|class\s+([A-Za-z0-9_]+))"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = fn_regex.captures(line) {
                let name = cap
                    .get(1)
                    .or_else(|| cap.get(2))
                    .or_else(|| cap.get(3))
                    .map(|m| m.as_str().to_string());

                if let Some(symbol_name) = name {
                    let node_type = if line.contains("class ") {
                        NodeType::Class
                    } else {
                        NodeType::Function
                    };

                    result.symbols.push(ParsedSymbol::new(
                        symbol_name,
                        node_type,
                        Some(line.trim().to_string()),
                        (line_idx + 1)..(line_idx + 2),
                        line.contains("export"),
                    ));
                }
            }
        }

        // 3. Extract Pinia Store definitions: export const useCartStore = defineStore('cart', ...)
        let pinia_regex = Regex::new(
            r#"export\s+const\s+(use[A-Za-z0-9_]+Store)\s*=\s*defineStore\s*\(\s*['"]([^'"]+)['"]"#,
        )
        .unwrap();

        for cap in pinia_regex.captures_iter(content) {
            if let Some(store_name) = cap.get(1) {
                result.symbols.push(ParsedSymbol::new(
                    store_name.as_str(),
                    NodeType::Component,
                    Some(format!(
                        "defineStore('{}')",
                        cap.get(2).map(|m| m.as_str()).unwrap_or("")
                    )),
                    1..content.lines().count() + 1,
                    true,
                ));
            }
        }

        // 4. Extract ES Module Imports
        let import_regex = Regex::new(
            r#"(?m)^\s*import\s+(?:(?:(?:type\s+)?(\w+)|(?:\{\s*([^}]+)\s*\}))\s+from\s+)?['"]([^'"]+)['"];?"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(caps) = import_regex.captures(line) {
                let default_import = caps.get(1).map(|m| m.as_str().trim());
                let named_imports = caps.get(2).map(|m| m.as_str());
                let source_path = caps.get(3).map(|m| m.as_str()).unwrap_or("");

                let mut imported_symbols = Vec::new();
                if let Some(def) = default_import {
                    if !def.is_empty() {
                        imported_symbols.push(def.to_string());
                    }
                }
                if let Some(named) = named_imports {
                    for sym in named.split(',') {
                        let clean = sym.split_whitespace().next_back().unwrap_or("").trim();
                        if !clean.is_empty() {
                            imported_symbols.push(clean.to_string());
                        }
                    }
                }

                result.imports.push(ParsedImport {
                    source_path: source_path.to_string(),
                    imported_symbols: imported_symbols.clone(),
                    is_default: default_import.is_some(),
                    is_namespace: false,
                    line_number: line_idx + 1,
                });

                for sym in imported_symbols {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: sym,
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(source_path.to_string()),
                    });
                }
            }
        }

        let mut current_fn: Option<String> = None;
        let mut depth = 0i32;
        let mut fn_start = 0i32;
        for line in content.lines() {
            if let Some(cap) = fn_regex.captures(line) {
                current_fn = cap
                    .get(1)
                    .or_else(|| cap.get(2))
                    .or_else(|| cap.get(3))
                    .map(|m| m.as_str().to_string());
                fn_start = depth;
            } else if let Some(caller) = current_fn.as_deref() {
                extract_calls_from_line(caller, line, &mut result);
            }
            depth += brace_delta(line);
            if current_fn.is_some() && depth <= fn_start && line.contains('}') {
                current_fn = None;
            }
        }

        result
    }
}
