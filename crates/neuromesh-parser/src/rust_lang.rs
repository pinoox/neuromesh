use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;

pub struct RustParser;

impl RustParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        // 1. Structs, Enums, Traits
        let item_regex =
            Regex::new(r#"(?m)^\s*(?:pub(?:\([^)]+\))?\s+)?(struct|enum|trait)\s+([A-Za-z0-9_]+)"#)
                .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = item_regex.captures(line) {
                let kind = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let name = cap
                    .get(2)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();

                let node_type = match kind {
                    "trait" => NodeType::Symbol,
                    _ => NodeType::Class,
                };

                result.symbols.push(ParsedSymbol {
                    name,
                    symbol_type: node_type,
                    signature: Some(line.trim().to_string()),
                    line_range: (line_idx + 1)..(line_idx + 2),
                    docstring: None,
                    exported: line.contains("pub "),
                });
            }
        }

        // 2. Functions
        let fn_regex =
            Regex::new(r#"(?m)^\s*(?:pub(?:\([^)]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)"#)
                .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = fn_regex.captures(line) {
                if let Some(fn_name) = cap.get(1) {
                    result.symbols.push(ParsedSymbol {
                        name: fn_name.as_str().to_string(),
                        symbol_type: NodeType::Function,
                        signature: Some(line.trim().to_string()),
                        line_range: (line_idx + 1)..(line_idx + 2),
                        docstring: None,
                        exported: line.contains("pub "),
                    });
                }
            }
        }

        // 3. Use statements
        let use_regex = Regex::new(r#"(?m)^\s*(?:pub\s+)?use\s+([^;]+);"#).unwrap();
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = use_regex.captures(line) {
                if let Some(use_path) = cap.get(1) {
                    let path_str = use_path.as_str().trim().to_string();
                    let imported = path_str.split("::").last().unwrap_or("").to_string();
                    result.imports.push(ParsedImport {
                        source_path: path_str.clone(),
                        imported_symbols: vec![imported.clone()],
                        is_default: false,
                        is_namespace: false,
                        line_number: line_idx + 1,
                    });
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: imported,
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(path_str),
                    });
                }
            }
        }

        result
    }
}
