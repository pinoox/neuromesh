use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;

pub struct VueParser;

impl VueParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("AnonymousComponent");

        // 1. Register the component itself
        result.symbols.push(ParsedSymbol::new(
            filename,
            NodeType::Component,
            Some(format!("<{} />", filename)),
            1..content.lines().count() + 1,
            true,
        ));

        // 2. Extract imports from <script> blocks
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

        // 3. Extract Pinia stores and composables
        let store_regex = Regex::new(r"const\s+\w+\s*=\s*(use\w+Store)\s*\(").unwrap();
        let composable_regex = Regex::new(r"const\s+[^=]+=\s*(use[A-Z]\w+)\s*\(").unwrap();

        for cap in store_regex.captures_iter(content) {
            if let Some(store_hook) = cap.get(1) {
                result.relationships.push(ParsedRelationship {
                    source_symbol: filename.to_string(),
                    target_symbol: store_hook.as_str().to_string(),
                    relationship: EdgeType::DependsOn,
                    target_file_hint: None,
                });
            }
        }

        for cap in composable_regex.captures_iter(content) {
            if let Some(composable) = cap.get(1) {
                let name = composable.as_str();
                if !name.ends_with("Store") {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: name.to_string(),
                        relationship: EdgeType::Calls,
                        target_file_hint: None,
                    });
                }
            }
        }

        // 4. Extract Template Component Usages
        let tag_regex = Regex::new(r"<([A-Z][a-zA-Z0-9]+)[\s/>]").unwrap();
        for cap in tag_regex.captures_iter(content) {
            if let Some(tag) = cap.get(1) {
                let child_component = tag.as_str();
                if child_component != filename {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: child_component.to_string(),
                        relationship: EdgeType::Contains,
                        target_file_hint: Some(format!("{}.vue", child_component)),
                    });
                }
            }
        }

        // 5. Extract SCSS @use / @import / Design tokens from <style>
        let scss_import_regex = Regex::new(r#"(?m)@(use|import)\s+['"]([^'"]+)['"]"#).unwrap();
        for cap in scss_import_regex.captures_iter(content) {
            if let Some(source) = cap.get(2) {
                result.imports.push(ParsedImport {
                    source_path: source.as_str().to_string(),
                    imported_symbols: vec!["styles".into()],
                    is_default: false,
                    is_namespace: false,
                    line_number: 1,
                });
                result.relationships.push(ParsedRelationship {
                    source_symbol: filename.to_string(),
                    target_symbol: source.as_str().to_string(),
                    relationship: EdgeType::References,
                    target_file_hint: Some(source.as_str().to_string()),
                });
            }
        }

        let token_regex = Regex::new(r"(\$[-a-zA-Z0-9_]+|--[-a-zA-Z0-9_]+)").unwrap();
        for cap in token_regex.captures_iter(content) {
            if let Some(token) = cap.get(1) {
                let token_str = token.as_str().to_string();
                if !result.design_tokens.contains(&token_str) {
                    result.design_tokens.push(token_str);
                }
            }
        }

        result
    }
}
