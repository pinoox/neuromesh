use crate::types::{AstAnalysisResult, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct SqlParser;

impl SqlParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("schema");

        extract_tables(&mut result, filename, content);
        extract_views(&mut result, content);
        extract_routines(&mut result, content);
        extract_indexes(&mut result, content);
        result
    }
}

fn extract_tables(result: &mut AstAnalysisResult, filename: &str, content: &str) {
    static TABLE_RE: OnceLock<Regex> = OnceLock::new();
    let table_re = TABLE_RE.get_or_init(|| {
        Regex::new(
            r#"(?is)\b(?:create\s+table(?:\s+if\s+not\s+exists)?|alter\s+table|insert\s+into|update|from|join)\s+(?:only\s+)?[`"\[]?([A-Za-z_][\w.]*)"#,
        )
        .unwrap()
    });
    for cap in table_re.captures_iter(content) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = unquote_ident(raw);
        if name.is_empty() || is_sql_keyword(&name) {
            continue;
        }
        let short = name.rsplit('.').next().unwrap_or(&name);
        if result
            .symbols
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(short))
        {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.symbols.push(ParsedSymbol::new(
            short.to_string(),
            NodeType::DbModel,
            Some(
                cap.get(0)
                    .unwrap()
                    .as_str()
                    .split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            line..(line + 1),
            true,
        ));
        result.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: short.to_string(),
            relationship: EdgeType::Contains,
            target_file_hint: None,
            receiver_hint: None,
        });
    }
}

fn extract_views(result: &mut AstAnalysisResult, content: &str) {
    static VIEW_RE: OnceLock<Regex> = OnceLock::new();
    let view_re = VIEW_RE.get_or_init(|| {
        Regex::new(r#"(?is)\bcreate\s+(?:or\s+replace\s+)?view\s+[`"\[]?([A-Za-z_][\w.]*)"#)
            .unwrap()
    });
    for cap in view_re.captures_iter(content) {
        let name = unquote_ident(cap.get(1).map(|m| m.as_str()).unwrap_or(""));
        let short = name.rsplit('.').next().unwrap_or(&name);
        if short.is_empty()
            || result
                .symbols
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(short))
        {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.symbols.push(ParsedSymbol::new(
            short.to_string(),
            NodeType::DbModel,
            Some(format!("CREATE VIEW {short}")),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_routines(result: &mut AstAnalysisResult, content: &str) {
    static FN_RE: OnceLock<Regex> = OnceLock::new();
    let fn_re = FN_RE.get_or_init(|| {
        Regex::new(
            r#"(?is)\bcreate\s+(?:or\s+replace\s+)?(?:procedure|function)\s+[`"\[]?([A-Za-z_][\w.]*)"#,
        )
        .unwrap()
    });
    for cap in fn_re.captures_iter(content) {
        let name = unquote_ident(cap.get(1).map(|m| m.as_str()).unwrap_or(""));
        let short = name.rsplit('.').next().unwrap_or(&name);
        if short.is_empty()
            || result
                .symbols
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(short))
        {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.symbols.push(ParsedSymbol::new(
            short.to_string(),
            NodeType::Function,
            Some(format!("CREATE ROUTINE {short}")),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_indexes(result: &mut AstAnalysisResult, content: &str) {
    static INDEX_RE: OnceLock<Regex> = OnceLock::new();
    let index_re = INDEX_RE.get_or_init(|| {
        Regex::new(r#"(?is)\bcreate\s+(?:unique\s+)?index\s+[`"\[]?([A-Za-z_][\w.]*)"#).unwrap()
    });
    for cap in index_re.captures_iter(content) {
        let name = unquote_ident(cap.get(1).map(|m| m.as_str()).unwrap_or(""));
        let short = name.rsplit('.').next().unwrap_or(&name);
        if short.is_empty()
            || result
                .symbols
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(short))
        {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.symbols.push(ParsedSymbol::new(
            short.to_string(),
            NodeType::Symbol,
            Some(format!("CREATE INDEX {short}")),
            line..(line + 1),
            true,
        ));
    }
}

fn unquote_ident(raw: &str) -> String {
    raw.trim()
        .trim_matches(['`', '"', '[', ']', '\''])
        .to_string()
}

fn is_sql_keyword(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "select"
            | "where"
            | "set"
            | "values"
            | "into"
            | "table"
            | "view"
            | "index"
            | "on"
            | "as"
            | "and"
            | "or"
            | "not"
            | "null"
            | "true"
            | "false"
            | "dual"
            | "lateral"
            | "only"
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
    fn create_table_is_db_model() {
        let src = "CREATE TABLE IF NOT EXISTS `sms_messages` (\n  id INTEGER PRIMARY KEY,\n  body TEXT NOT NULL\n);\nCREATE INDEX sms_messages_body_idx ON sms_messages (body);\n";
        let ast = SqlParser::parse(Path::new("database/sql/sms_messages.sql"), src);
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "sms_messages" && s.symbol_type == NodeType::DbModel),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "sms_messages_body_idx"));
    }

    #[test]
    fn view_and_function_extract() {
        let src = "CREATE VIEW inbox_unread AS SELECT * FROM sms_messages;\nCREATE FUNCTION count_sms() RETURNS int AS $$ SELECT 1 $$;\n";
        let ast = SqlParser::parse(Path::new("schema.sql"), src);
        assert!(ast.symbols.iter().any(|s| s.name == "inbox_unread"));
        assert!(ast.symbols.iter().any(|s| s.name == "count_sms"));
    }
}
