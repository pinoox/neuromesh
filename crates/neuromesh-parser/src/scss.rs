use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;

pub struct ScssParser;

impl ScssParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("styles");

        // 1. Extract SCSS variables: $color-primary: #3b82f6;
        let var_regex = Regex::new(r#"(?m)^\s*(\$[-a-zA-Z0-9_]+)\s*:\s*([^;]+);"#).unwrap();
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = var_regex.captures(line) {
                if let Some(var_name) = cap.get(1) {
                    let name = var_name.as_str().to_string();
                    result.symbols.push(ParsedSymbol {
                        name: name.clone(),
                        symbol_type: NodeType::StyleToken,
                        signature: Some(line.trim().to_string()),
                        line_range: (line_idx + 1)..(line_idx + 2),
                        docstring: None,
                        exported: true,
                    });
                    result.design_tokens.push(name);
                }
            }
        }

        // 2. Extract SCSS Mixins: @mixin responsive($breakpoint) { ... }
        let mixin_regex =
            Regex::new(r#"(?m)^\s*@mixin\s+([-a-zA-Z0-9_]+)(?:\(([^)]*)\))?"#).unwrap();
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = mixin_regex.captures(line) {
                if let Some(mixin_name) = cap.get(1) {
                    result.symbols.push(ParsedSymbol {
                        name: format!("@mixin {}", mixin_name.as_str()),
                        symbol_type: NodeType::Function,
                        signature: Some(line.trim().to_string()),
                        line_range: (line_idx + 1)..(line_idx + 2),
                        docstring: None,
                        exported: true,
                    });
                }
            }
        }

        // 3. Extract @use and @import
        let import_regex = Regex::new(r#"(?m)@(use|import)\s+['"]([^'"]+)['"]"#).unwrap();
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = import_regex.captures(line) {
                if let Some(source) = cap.get(2) {
                    let source_path = source.as_str().to_string();
                    result.imports.push(ParsedImport {
                        source_path: source_path.clone(),
                        imported_symbols: vec!["styles".into()],
                        is_default: false,
                        is_namespace: false,
                        line_number: line_idx + 1,
                    });
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: source_path.clone(),
                        relationship: EdgeType::References,
                        target_file_hint: Some(source_path),
                    });
                }
            }
        }

        result
    }
}
