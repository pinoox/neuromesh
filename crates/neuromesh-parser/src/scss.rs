use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum StylesheetKind {
    Css,
    Scss,
    Less,
}

pub struct ScssParser;

impl ScssParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        parse_stylesheet(file_path, content, StylesheetKind::Scss)
    }

    pub fn parse_css(file_path: &Path, content: &str) -> AstAnalysisResult {
        parse_stylesheet(file_path, content, StylesheetKind::Css)
    }

    pub fn parse_less(file_path: &Path, content: &str) -> AstAnalysisResult {
        parse_stylesheet(file_path, content, StylesheetKind::Less)
    }
}

fn parse_stylesheet(file_path: &Path, content: &str, kind: StylesheetKind) -> AstAnalysisResult {
    let mut result = AstAnalysisResult::default();
    let filename = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("styles");

    extract_imports(&mut result, filename, content);
    extract_custom_properties(&mut result, content);
    extract_class_and_id_selectors(&mut result, content, kind);

    match kind {
        StylesheetKind::Scss => {
            extract_scss_variables(&mut result, content);
            extract_scss_mixins(&mut result, content);
        }
        StylesheetKind::Less => extract_less_variables(&mut result, content),
        StylesheetKind::Css => {}
    }

    result
}

fn extract_imports(result: &mut AstAnalysisResult, filename: &str, content: &str) {
    static IMPORT_RE: OnceLock<Regex> = OnceLock::new();
    let import_re = IMPORT_RE.get_or_init(|| {
        Regex::new(r#"(?m)@(use|import|forward)\s+(?:url\()?['"]([^'"]+)['"]"#).unwrap()
    });
    for cap in import_re.captures_iter(content) {
        let Some(source) = cap.get(2) else {
            continue;
        };
        let source_path = source.as_str().to_string();
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.imports.push(ParsedImport {
            source_path: source_path.clone(),
            imported_symbols: vec!["styles".into()],
            is_default: false,
            is_namespace: false,
            line_number: line,
        });
        result.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: source_path.clone(),
            relationship: EdgeType::Imports,
            target_file_hint: Some(source_path),
            receiver_hint: None,
        });
    }
}

fn extract_custom_properties(result: &mut AstAnalysisResult, content: &str) {
    static VAR_RE: OnceLock<Regex> = OnceLock::new();
    let var_re = VAR_RE.get_or_init(|| Regex::new(r"(?m)(--[A-Za-z_][-A-Za-z0-9_]*)\s*:").unwrap());
    for cap in var_re.captures_iter(content) {
        let raw = cap.get(1).unwrap().as_str();
        let name = raw.trim_start_matches('-').to_string();
        if name.is_empty() || result.symbols.iter().any(|s| s.name == name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.design_tokens.push(raw.to_string());
        result.symbols.push(ParsedSymbol::new(
            name,
            NodeType::StyleToken,
            Some(raw.to_string()),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_class_and_id_selectors(
    result: &mut AstAnalysisResult,
    content: &str,
    kind: StylesheetKind,
) {
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    static ID_RE: OnceLock<Regex> = OnceLock::new();
    static LESS_MIXIN_RE: OnceLock<Regex> = OnceLock::new();
    let class_re =
        CLASS_RE.get_or_init(|| Regex::new(r"(?m)^\s*\.([A-Za-z_][-A-Za-z0-9_]*)\s*\{").unwrap());
    let id_re =
        ID_RE.get_or_init(|| Regex::new(r"(?m)^\s*#([A-Za-z_][-A-Za-z0-9_]*)\s*\{").unwrap());
    let less_mixin_re = LESS_MIXIN_RE
        .get_or_init(|| Regex::new(r"(?m)^\s*\.([A-Za-z_][-A-Za-z0-9_]*)\s*\([^)]*\)").unwrap());

    if matches!(kind, StylesheetKind::Less) {
        for cap in less_mixin_re.captures_iter(content) {
            let name = cap.get(1).unwrap().as_str();
            if result.symbols.iter().any(|s| s.name == name) {
                continue;
            }
            let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
            result.symbols.push(ParsedSymbol::new(
                name.to_string(),
                NodeType::Function,
                Some(format!(".{name}()")),
                line..(line + 1),
                true,
            ));
        }
    }

    for cap in class_re.captures_iter(content) {
        let name = cap.get(1).unwrap().as_str();
        if result.symbols.iter().any(|s| s.name == name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.design_tokens.push(format!(".{name}"));
        result.symbols.push(ParsedSymbol::new(
            name.to_string(),
            NodeType::StyleToken,
            Some(format!(".{name} {{")),
            line..(line + 1),
            true,
        ));
    }

    for cap in id_re.captures_iter(content) {
        let name = cap.get(1).unwrap().as_str();
        if result.symbols.iter().any(|s| s.name == name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.design_tokens.push(format!("#{name}"));
        result.symbols.push(ParsedSymbol::new(
            name.to_string(),
            NodeType::StyleToken,
            Some(format!("#{name} {{")),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_scss_variables(result: &mut AstAnalysisResult, content: &str) {
    static VAR_RE: OnceLock<Regex> = OnceLock::new();
    let var_re = VAR_RE.get_or_init(|| Regex::new(r"(?m)^\s*(\$[-a-zA-Z0-9_]+)\s*:").unwrap());
    for cap in var_re.captures_iter(content) {
        let raw = cap.get(1).unwrap().as_str();
        let name = raw.trim_start_matches('$').to_string();
        if name.is_empty() || result.symbols.iter().any(|s| s.name == name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.design_tokens.push(raw.to_string());
        result.symbols.push(ParsedSymbol::new(
            name,
            NodeType::StyleToken,
            Some(raw.to_string()),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_scss_mixins(result: &mut AstAnalysisResult, content: &str) {
    static MIXIN_RE: OnceLock<Regex> = OnceLock::new();
    let mixin_re =
        MIXIN_RE.get_or_init(|| Regex::new(r"(?m)^\s*@mixin\s+([-a-zA-Z0-9_]+)").unwrap());
    for cap in mixin_re.captures_iter(content) {
        let mixin_name = cap.get(1).unwrap().as_str();
        if result.symbols.iter().any(|s| s.name == mixin_name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.symbols.push(ParsedSymbol::new(
            mixin_name.to_string(),
            NodeType::Function,
            Some(format!("@mixin {mixin_name}")),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_less_variables(result: &mut AstAnalysisResult, content: &str) {
    static VAR_RE: OnceLock<Regex> = OnceLock::new();
    let var_re =
        VAR_RE.get_or_init(|| Regex::new(r"(?m)^\s*(@[A-Za-z_][-A-Za-z0-9_]*)\s*:").unwrap());
    for cap in var_re.captures_iter(content) {
        let raw = cap.get(1).unwrap().as_str();
        if is_css_at_rule(raw) {
            continue;
        }
        let name = raw.trim_start_matches('@');
        if name.is_empty() || result.symbols.iter().any(|s| s.name == name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.design_tokens.push(raw.to_string());
        result.symbols.push(ParsedSymbol::new(
            name.to_string(),
            NodeType::StyleToken,
            Some(cap.get(0).unwrap().as_str().trim().to_string()),
            line..(line + 1),
            true,
        ));
    }
}

fn is_css_at_rule(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "@import"
            | "@use"
            | "@forward"
            | "@media"
            | "@supports"
            | "@charset"
            | "@namespace"
            | "@keyframes"
            | "@font-face"
            | "@page"
            | "@layer"
            | "@container"
            | "@property"
            | "@scope"
            | "@plugin"
            | "@document"
            | "@viewport"
            | "@counter-style"
            | "@starting-style"
    )
}

fn line_of(content: &str, byte: usize) -> usize {
    content
        .get(..byte)
        .map(|head| head.bytes().filter(|b| *b == b'\n').count() + 1)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn css_extracts_class_id_and_custom_property() {
        let src = ":root { --smsUnread: #ef4444; }\n.smsBadge { color: var(--smsUnread); }\n#smsInbox { fill: currentColor; }\n@import url(\"tokens.css\");\n";
        let ast = ScssParser::parse_css(Path::new("styles/sms.css"), src);
        assert!(ast.symbols.iter().any(|s| s.name == "smsUnread"));
        assert!(ast.symbols.iter().any(|s| s.name == "smsBadge"));
        assert!(ast.symbols.iter().any(|s| s.name == "smsInbox"));
        assert!(ast.imports.iter().any(|i| i.source_path == "tokens.css"));
    }

    #[test]
    fn less_extracts_variable_and_parametric_mixin() {
        let src = "@import \"tokens.less\";\n@smsUnread: #ef4444;\n.smsBadge(@color) {\n  color: @smsUnread;\n}\n@media screen {\n  .hidden { display: none; }\n}\n";
        let ast = ScssParser::parse_less(Path::new("styles/sms.less"), src);
        assert!(ast.symbols.iter().any(|s| s.name == "smsUnread"));
        assert!(ast.symbols.iter().any(|s| s.name == "smsBadge"));
        assert!(
            ast.symbols.iter().any(|s| s.name == "hidden"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(!ast.symbols.iter().any(|s| s.name == "@media"));
        assert!(ast.imports.iter().any(|i| i.source_path == "tokens.less"));
    }

    #[test]
    fn scss_still_extracts_dollar_vars_and_mixins() {
        let src =
            "$sms-unread: #ef4444;\n@mixin smsBadge { color: $sms-unread; }\n@use 'tokens';\n";
        let ast = ScssParser::parse(Path::new("styles/_sms.scss"), src);
        assert!(ast.symbols.iter().any(|s| s.name == "sms-unread"));
        assert!(ast.symbols.iter().any(|s| s.name == "smsBadge"));
        assert!(ast.imports.iter().any(|i| i.source_path == "tokens"));
    }
}
