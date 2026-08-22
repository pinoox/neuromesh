use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub name: String,
    pub kind: String, // "interface" | "type" | "enum" | "struct" | "class"
    pub signature: String,
    pub is_exported: bool,
    pub line_number: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticTypeMap {
    pub types: HashMap<String, TypeDefinition>,
}

pub struct SemanticTypeExtractor;

impl SemanticTypeExtractor {
    pub fn extract(file_path: &str, content: &str) -> SemanticTypeMap {
        let mut map = SemanticTypeMap::default();

        if file_path.ends_with(".ts") || file_path.ends_with(".tsx") || file_path.ends_with(".vue")
        {
            Self::extract_typescript_types(content, &mut map);
        } else if file_path.ends_with(".rs") {
            Self::extract_rust_types(content, &mut map);
        } else if file_path.ends_with(".py") {
            Self::extract_python_types(content, &mut map);
        }

        map
    }

    fn extract_typescript_types(content: &str, map: &mut SemanticTypeMap) {
        let type_regex =
            Regex::new(r"(?m)^\s*(?:export\s+)?(interface|type|enum)\s+([A-Za-z0-9_$]+)").unwrap();
        for cap in type_regex.captures_iter(content) {
            let kind = cap[1].to_string();
            let name = cap[2].to_string();
            let trimmed = cap[0].trim();
            let is_exported = trimmed.starts_with("export");

            map.types.insert(
                name.clone(),
                TypeDefinition {
                    name,
                    kind,
                    signature: trimmed.to_string(),
                    is_exported,
                    line_number: 1,
                },
            );
        }
    }

    fn extract_rust_types(content: &str, map: &mut SemanticTypeMap) {
        let rust_regex =
            Regex::new(r"(?m)^\s*(?:pub\s+)?(struct|enum|trait|type)\s+([A-Za-z0-9_]+)").unwrap();
        for cap in rust_regex.captures_iter(content) {
            let kind = cap[1].to_string();
            let name = cap[2].to_string();
            let trimmed = cap[0].trim();
            let is_exported = trimmed.starts_with("pub");

            map.types.insert(
                name.clone(),
                TypeDefinition {
                    name,
                    kind,
                    signature: trimmed.to_string(),
                    is_exported,
                    line_number: 1,
                },
            );
        }
    }

    fn extract_python_types(content: &str, map: &mut SemanticTypeMap) {
        let py_regex = Regex::new(r"(?m)^\s*class\s+([A-Za-z0-9_]+)(?:\(([^)]+)\))?:").unwrap();
        for cap in py_regex.captures_iter(content) {
            let name = cap[1].to_string();
            let trimmed = cap[0].trim();
            map.types.insert(
                name.clone(),
                TypeDefinition {
                    name,
                    kind: "class".to_string(),
                    signature: trimmed.to_string(),
                    is_exported: true,
                    line_number: 1,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ts_types() {
        let code = r#"
            export interface CartItem {
                id: string;
                price: number;
            }
            export type CurrencyCode = "USD" | "EUR";
        "#;
        let map = SemanticTypeExtractor::extract("types.ts", code);
        assert!(map.types.contains_key("CartItem"));
        assert!(map.types.contains_key("CurrencyCode"));
        assert!(map.types["CartItem"].is_exported);
    }
}
