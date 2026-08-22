use crate::types::{AstAnalysisResult, ParsedRelationship};
use neuromesh_core::EdgeType;
use regex::Regex;
use std::sync::OnceLock;

const CALL_STOPWORDS: &[&str] = &[
    "if", "for", "while", "loop", "match", "return", "break", "continue", "else", "async", "await",
    "pub", "use", "mod", "crate", "super", "self", "let", "mut", "const", "static", "type",
    "struct", "enum", "trait", "impl", "fn", "where", "unsafe", "format", "vec", "println",
    "eprintln", "write", "writeln", "panic", "assert", "todo", "unimplemented", "unreachable",
    "some", "none", "ok", "err", "true", "false", "box", "drop", "sizeof", "typeof", "new",
    "import", "from", "function", "class", "def", "print", "len", "range", "super", "this",
    "console", "require", "export", "switch", "case", "try", "catch", "throw", "yield",
];

/// Extract call-like identifiers from a line of source belonging to `caller`.
pub fn extract_calls_from_line(caller: &str, line: &str, result: &mut AstAnalysisResult) {
    static CALL_RE: OnceLock<Regex> = OnceLock::new();
    let call_re = CALL_RE.get_or_init(|| {
        Regex::new(r"(?:(?P<recv>[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)\.)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(")
            .unwrap()
    });

    let trimmed = strip_line_comment(line);
    for cap in call_re.captures_iter(trimmed) {
        let name = cap.name("name").map(|m| m.as_str()).unwrap_or("");
        if !is_callable_name(name) || name == caller {
            continue;
        }
        let qualified = if let Some(recv) = cap.name("recv") {
            let recv = recv.as_str();
            if recv == "self" || recv == "this" || recv == "Super" {
                name.to_string()
            } else if let Some(last) = recv.split("::").last() {
                if last.chars().next().is_some_and(|c| c.is_uppercase()) {
                    name.to_string()
                } else {
                    name.to_string()
                }
            } else {
                name.to_string()
            }
        } else {
            name.to_string()
        };

        if result
            .relationships
            .iter()
            .any(|rel| rel.source_symbol == caller && rel.target_symbol == qualified && rel.relationship == EdgeType::Calls)
        {
            continue;
        }

        result.relationships.push(ParsedRelationship {
            source_symbol: caller.to_string(),
            target_symbol: qualified,
            relationship: EdgeType::Calls,
            target_file_hint: None,
        });
    }

    static PATH_CALL_RE: OnceLock<Regex> = OnceLock::new();
    let path_re = PATH_CALL_RE
        .get_or_init(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*::)+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());
    for cap in path_re.captures_iter(trimmed) {
        if let Some(name) = cap.get(2) {
            let name = name.as_str();
            if !is_callable_name(name) || name == caller {
                continue;
            }
            if result.relationships.iter().any(|rel| {
                rel.source_symbol == caller
                    && rel.target_symbol == name
                    && rel.relationship == EdgeType::Calls
            }) {
                continue;
            }
            result.relationships.push(ParsedRelationship {
                source_symbol: caller.to_string(),
                target_symbol: name.to_string(),
                relationship: EdgeType::Calls,
                target_file_hint: None,
            });
        }
    }
}

pub fn is_callable_name(name: &str) -> bool {
    if name.len() < 2 {
        return false;
    }
    let lower = name.to_lowercase();
    !CALL_STOPWORDS.contains(&lower.as_str())
}

fn strip_line_comment(line: &str) -> &str {
    if let Some(idx) = line.find("//") {
        if !line[..idx].contains('"') {
            return &line[..idx];
        }
    }
    if let Some(idx) = line.find('#') {
        if !line[..idx].contains('"') && !line.contains("#!") {
            return &line[..idx];
        }
    }
    line
}

pub fn brace_delta(line: &str) -> i32 {
    let mut delta = 0i32;
    let mut in_string = false;
    let mut prev = '\0';
    for ch in line.chars() {
        if ch == '"' && prev != '\\' {
            in_string = !in_string;
        } else if !in_string {
            if ch == '{' {
                delta += 1;
            } else if ch == '}' {
                delta -= 1;
            }
        }
        prev = ch;
    }
    delta
}
