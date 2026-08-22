use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;

pub struct PythonParser;

impl PythonParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        // 1. Classes & Functions
        let def_regex = Regex::new(r#"(?m)^\s*(class|def)\s+([A-Za-z0-9_]+)"#).unwrap();
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = def_regex.captures(line) {
                let kind = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let name = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();

                let node_type = if kind == "class" {
                    NodeType::Class
                } else {
                    NodeType::Function
                };

                result.symbols.push(ParsedSymbol {
                    name,
                    symbol_type: node_type,
                    signature: Some(line.trim().to_string()),
                    line_range: (line_idx + 1)..(line_idx + 2),
                    docstring: None,
                    exported: !line.trim_start().starts_with('_'),
                });
            }
        }

        // 2. Imports: import foo / from foo import bar
        let import_regex = Regex::new(
            r#"(?m)^\s*(?:from\s+([A-Za-z0-9_.]+)\s+import\s+([^#\n]+)|import\s+([A-Za-z0-9_.]+))"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = import_regex.captures(line) {
                if let Some(from_mod) = cap.get(1) {
                    let source = from_mod.as_str().to_string();
                    let imported = cap
                        .get(2)
                        .map(|m| {
                            m.as_str()
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    result.imports.push(ParsedImport {
                        source_path: source.clone(),
                        imported_symbols: imported.clone(),
                        is_default: false,
                        is_namespace: false,
                        line_number: line_idx + 1,
                    });

                    for sym in imported {
                        result.relationships.push(ParsedRelationship {
                            source_symbol: filename.to_string(),
                            target_symbol: sym,
                            relationship: EdgeType::Imports,
                            target_file_hint: Some(source.clone()),
                        });
                    }
                } else if let Some(direct_mod) = cap.get(3) {
                    let source = direct_mod.as_str().to_string();
                    result.imports.push(ParsedImport {
                        source_path: source.clone(),
                        imported_symbols: vec![source.clone()],
                        is_default: true,
                        is_namespace: true,
                        line_number: line_idx + 1,
                    });
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: source.clone(),
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(source),
                    });
                }
            }
        }

        result
    }
}
