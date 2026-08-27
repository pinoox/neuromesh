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

        // 1. Extract interfaces, types, enums
        let type_regex =
            Regex::new(r#"(?m)^\s*(?:export\s+)?(?:interface|type|enum)\s+([A-Za-z0-9_]+)"#)
                .unwrap();

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
            r#"(?m)^\s*(?:export\s+(?:default\s+)?)?(?:async\s+)?(?:function\s+([A-Za-z0-9_]+)|const\s+([A-Za-z0-9_]+)\s*(?::[^=]{1,80})?\s*=\s*(?:async\s*)?\([^)]*\)\s*=>|class\s+([A-Za-z0-9_]+))"#,
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

        // 4. Extract ES Module Imports (named, namespace, side-effect, CSS/JSON)
        let import_regex = Regex::new(
            r#"(?m)^\s*import\s+(?:(?:type\s+)?(?:(\w+)|(?:\{\s*([^}]+)\s*\})|(?:\*\s+as\s+(\w+)))\s+from\s+)?['"]([^'"]+)['"];?"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(caps) = import_regex.captures(line) {
                let default_import = caps.get(1).map(|m| m.as_str().trim());
                let named_imports = caps.get(2).map(|m| m.as_str());
                let namespace_import = caps.get(3).map(|m| m.as_str().trim());
                let source_path = caps.get(4).map(|m| m.as_str()).unwrap_or("");

                let mut imported_symbols = Vec::new();
                if let Some(def) = default_import {
                    if !def.is_empty() {
                        imported_symbols.push(def.to_string());
                    }
                }
                if let Some(ns) = namespace_import {
                    if !ns.is_empty() {
                        imported_symbols.push(ns.to_string());
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
                if imported_symbols.is_empty() {
                    imported_symbols.push(asset_import_label(source_path));
                }

                result.imports.push(ParsedImport {
                    source_path: source_path.to_string(),
                    imported_symbols: imported_symbols.clone(),
                    is_default: default_import.is_some(),
                    is_namespace: namespace_import.is_some(),
                    line_number: line_idx + 1,
                });

                for sym in imported_symbols {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: sym,
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(source_path.to_string()),
                        receiver_hint: None,
                    });
                }
            }
        }

        let reexport_regex =
            Regex::new(r#"(?m)^\s*export\s*\{([^}]+)\}(?:\s*from\s*['"]([^'"]+)['"])?"#).unwrap();
        for cap in reexport_regex.captures_iter(content) {
            if let Some(names) = cap.get(1) {
                for part in names.as_str().split(',') {
                    let export_name = part
                        .split_whitespace()
                        .last()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if export_name.is_empty() {
                        continue;
                    }
                    if !result.exports.contains(&export_name) {
                        result.exports.push(export_name.clone());
                    }
                    if let Some(from) = cap.get(2) {
                        result.relationships.push(ParsedRelationship {
                            source_symbol: filename.to_string(),
                            target_symbol: export_name,
                            relationship: EdgeType::Imports,
                            target_file_hint: Some(from.as_str().to_string()),
                            receiver_hint: None,
                        });
                    }
                }
            }
        }

        collect_cjs_requires(&mut result, filename, content);

        for sym in &result.symbols {
            if sym.exported && !result.exports.contains(&sym.name) {
                result.exports.push(sym.name.clone());
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

fn asset_import_label(source: &str) -> String {
    Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module")
        .to_string()
}

fn collect_cjs_requires(result: &mut AstAnalysisResult, filename: &str, content: &str) {
    let require_re = Regex::new(r#"(?:require|import)\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
    let destructure_re = Regex::new(
        r#"(?:const|let|var)\s+(?:\{([^}]+)\}|([A-Za-z_][A-Za-z0-9_]*))\s*=\s*require\(\s*['"]([^'"]+)['"]"#,
    )
    .unwrap();
    let exports_re =
        Regex::new(r#"(?:module\.exports|exports)\.([A-Za-z_][A-Za-z0-9_]*)\s*="#).unwrap();

    for cap in destructure_re.captures_iter(content) {
        let source_path = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        let mut imported_symbols = Vec::new();
        if let Some(named) = cap.get(1) {
            for part in named.as_str().split(',') {
                let clean = part.split_whitespace().next().unwrap_or("").trim();
                if !clean.is_empty() {
                    imported_symbols.push(clean.to_string());
                }
            }
        }
        if let Some(binding) = cap.get(2) {
            imported_symbols.push(binding.as_str().to_string());
        }
        let line = cap
            .get(0)
            .map(|m| content[..m.start()].bytes().filter(|b| *b == b'\n').count() + 1)
            .unwrap_or(1);
        result.imports.push(ParsedImport {
            source_path: source_path.to_string(),
            imported_symbols: imported_symbols.clone(),
            is_default: cap.get(2).is_some(),
            is_namespace: false,
            line_number: line,
        });
        for sym in imported_symbols {
            result.relationships.push(ParsedRelationship {
                source_symbol: filename.to_string(),
                target_symbol: sym,
                relationship: EdgeType::Imports,
                target_file_hint: Some(source_path.to_string()),
                receiver_hint: None,
            });
        }
    }

    for cap in require_re.captures_iter(content) {
        let source_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if result.imports.iter().any(|i| i.source_path == source_path) {
            continue;
        }
        let label = asset_import_label(source_path);
        let line = cap
            .get(0)
            .map(|m| content[..m.start()].bytes().filter(|b| *b == b'\n').count() + 1)
            .unwrap_or(1);
        result.imports.push(ParsedImport {
            source_path: source_path.to_string(),
            imported_symbols: vec![label.clone()],
            is_default: true,
            is_namespace: false,
            line_number: line,
        });
        result.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: label,
            relationship: EdgeType::Imports,
            target_file_hint: Some(source_path.to_string()),
            receiver_hint: None,
        });
    }

    for cap in exports_re.captures_iter(content) {
        let name = cap.get(1).unwrap().as_str();
        if !result.exports.contains(&name.to_string()) {
            result.exports.push(name.to_string());
        }
    }

    if let Some(cap) = Regex::new(r"module\.exports\s*=\s*\{([^}]+)\}")
        .ok()
        .and_then(|re| re.captures(content))
    {
        for part in cap.get(1).unwrap().as_str().split(',') {
            let name = part.split(':').next().unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !result.exports.contains(&name.to_string())
            {
                result.exports.push(name.to_string());
            }
        }
    }
}
