use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::NodeType;
use serde_json::Value;
use std::path::Path;

const MAX_SYMBOLS: usize = 64;
const SKIP_OBJECT_KEYS: &[&str] = &[
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
    "require",
    "require-dev",
    "packages",
    "lockfileVersion",
    "workspaces",
];

pub struct JsonParser;

impl JsonParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        match serde_json::from_str::<Value>(content) {
            Ok(Value::Object(map)) => {
                walk_object(&mut result, file_path, &map, 0, content);
            }
            Ok(_) => {}
            Err(_) => extract_quoted_keys(&mut result, content),
        }
        result
    }
}

fn walk_object(
    result: &mut AstAnalysisResult,
    path: &Path,
    map: &serde_json::Map<String, Value>,
    depth: usize,
    content: &str,
) {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    for (key, value) in map {
        if result.symbols.len() >= MAX_SYMBOLS {
            return;
        }
        if !keep_key(key) {
            continue;
        }
        let line = key_line(content, key);
        let signature = match value {
            Value::String(s) if s.len() < 80 => Some(format!("{key}: {s}")),
            Value::Number(n) => Some(format!("{key}: {n}")),
            Value::Bool(b) => Some(format!("{key}: {b}")),
            Value::Object(_) => Some(format!("{key} {{}}")),
            Value::Array(_) => Some(format!("{key} []")),
            _ => Some(key.clone()),
        };
        if !result.symbols.iter().any(|s| s.name == *key) {
            result.symbols.push(ParsedSymbol::new(
                key.clone(),
                NodeType::Config,
                signature,
                line..(line + 1),
                true,
            ));
        }
        if depth >= 2 {
            continue;
        }
        match value {
            Value::Object(child)
                if !SKIP_OBJECT_KEYS.iter().any(|k| k.eq_ignore_ascii_case(key)) =>
            {
                walk_object(result, path, child, depth + 1, content);
            }
            Value::Object(child)
                if filename == "package.json" && key.eq_ignore_ascii_case("scripts") =>
            {
                for (script, _) in child {
                    if result.symbols.len() >= MAX_SYMBOLS {
                        return;
                    }
                    if !keep_key(script) || result.symbols.iter().any(|s| s.name == *script) {
                        continue;
                    }
                    let script_line = key_line(content, script);
                    result.symbols.push(ParsedSymbol::new(
                        script.clone(),
                        NodeType::Config,
                        Some(format!("npm script {script}")),
                        script_line..(script_line + 1),
                        true,
                    ));
                }
            }
            _ => {}
        }
    }
}

fn keep_key(key: &str) -> bool {
    let trimmed = key.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return false;
    }
    if trimmed.starts_with('_') || trimmed.starts_with('$') && trimmed != "$schema" {
        return false;
    }
    !matches!(
        trimmed,
        "version"
            | "license"
            | "private"
            | "type"
            | "main"
            | "module"
            | "exports"
            | "files"
            | "keywords"
            | "author"
            | "description"
            | "homepage"
            | "bugs"
            | "repository"
            | "engines"
            | "os"
            | "cpu"
    )
}

fn key_line(content: &str, key: &str) -> usize {
    let needle = format!("\"{key}\"");
    content
        .find(&needle)
        .map(|byte| content[..byte].bytes().filter(|b| *b == b'\n').count() + 1)
        .unwrap_or(1)
}

fn extract_quoted_keys(result: &mut AstAnalysisResult, content: &str) {
    for (idx, line) in content.lines().enumerate() {
        if result.symbols.len() >= MAX_SYMBOLS {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('"') {
            continue;
        }
        let Some(end) = trimmed[1..].find('"') else {
            continue;
        };
        let key = &trimmed[1..=end];
        if !keep_key(key) || result.symbols.iter().any(|s| s.name == key) {
            continue;
        }
        if !trimmed[end + 2..].contains(':') {
            continue;
        }
        result.symbols.push(ParsedSymbol::new(
            key,
            NodeType::Config,
            Some(trimmed.chars().take(80).collect()),
            (idx + 1)..(idx + 2),
            true,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn package_scripts_and_skips_deps() {
        let src = r#"{
  "name": "mini-store",
  "scripts": { "build": "tsc" },
  "dependencies": { "react": "19.0.0", "left-pad": "1" }
}"#;
        let ast = JsonParser::parse(Path::new("package.json"), src);
        assert!(ast.symbols.iter().any(|s| s.name == "scripts"));
        assert!(ast.symbols.iter().any(|s| s.name == "build"));
        assert!(
            !ast.symbols.iter().any(|s| s.name == "left-pad"),
            "dependency names must not flood the graph"
        );
    }

    #[test]
    fn config_object_keys() {
        let src = r##"{ "smsFrom": "neuromesh", "nested": { "smsUnread": "#ef4444" } }"##;
        let ast = JsonParser::parse(Path::new("config/sms.json"), src);
        assert!(ast.symbols.iter().any(|s| s.name == "smsFrom"));
        assert!(ast.symbols.iter().any(|s| s.name == "smsUnread"));
    }
}
