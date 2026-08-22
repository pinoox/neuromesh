use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;

pub struct GenericParser;

impl GenericParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        // General C-style / Java / C# / PHP / Go classes and functions
        let class_regex = Regex::new(
            r#"(?m)^\s*(?:public|private|protected|static|final|abstract|\s)*\s*(class|interface|struct|package)\s+([A-Za-z0-9_]+)"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = class_regex.captures(line) {
                if let Some(name) = cap.get(2) {
                    result.symbols.push(ParsedSymbol {
                        name: name.as_str().to_string(),
                        symbol_type: NodeType::Class,
                        signature: Some(line.trim().to_string()),
                        line_range: (line_idx + 1)..(line_idx + 2),
                        docstring: None,
                        exported: true,
                    });
                }
            }
        }

        // Generic import / include / require
        let generic_import_regex = Regex::new(
            r#"(?m)^\s*(?:import|include|require|using)\s+['"<]?([^'">;\s]+)['">]?"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(cap) = generic_import_regex.captures(line) {
                if let Some(src) = cap.get(1) {
                    let source = src.as_str().to_string();
                    let imported = source.split('/').last().unwrap_or(&source).to_string();
                    result.imports.push(ParsedImport {
                        source_path: source.clone(),
                        imported_symbols: vec![imported.clone()],
                        is_default: false,
                        is_namespace: false,
                        line_number: line_idx + 1,
                    });
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: imported,
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(source),
                    });
                }
            }
        }

        result
    }
}
