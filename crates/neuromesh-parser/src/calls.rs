use crate::types::{AstAnalysisResult, ParsedRelationship};
use neuromesh_core::EdgeType;
use regex::Regex;
use std::sync::OnceLock;

const CALL_STOPWORDS: &[&str] = &[
    "if",
    "for",
    "while",
    "loop",
    "match",
    "return",
    "break",
    "continue",
    "else",
    "async",
    "await",
    "pub",
    "use",
    "mod",
    "crate",
    "super",
    "self",
    "let",
    "mut",
    "const",
    "static",
    "type",
    "struct",
    "enum",
    "trait",
    "impl",
    "fn",
    "where",
    "unsafe",
    "format",
    "vec",
    "println",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "assert",
    "todo",
    "unimplemented",
    "unreachable",
    "some",
    "none",
    "ok",
    "err",
    "true",
    "false",
    "box",
    "drop",
    "sizeof",
    "typeof",
    "new",
    "import",
    "from",
    "function",
    "class",
    "def",
    "print",
    "len",
    "range",
    "super",
    "this",
    "console",
    "require",
    "export",
    "switch",
    "case",
    "try",
    "catch",
    "throw",
    "yield",
    "when",
    "fun",
    "object",
    "val",
    "var",
];

/// Extract call-like identifiers from a line of source belonging to `caller`.
pub fn extract_calls_from_line(caller: &str, line: &str, result: &mut AstAnalysisResult) {
    extract_calls_from_line_ctx(caller, line, result, None);
}

pub fn extract_calls_from_line_ctx(
    caller: &str,
    line: &str,
    result: &mut AstAnalysisResult,
    impl_parent: Option<&str>,
) {
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
        if cap
            .get(0)
            .is_some_and(|m| trimmed[..m.start()].ends_with("::"))
        {
            continue;
        }
        let recv = cap.name("recv").map(|m| m.as_str());
        let match_start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let before = &trimmed[..match_start];
        let receiver_hint = match recv {
            Some("self") | Some("this") | Some("Super") => impl_parent.map(|p| format!("impl:{p}")),
            Some(recv) => recv
                .split("::")
                .last()
                .filter(|last| last.chars().next().is_some_and(|c| c.is_uppercase()))
                .map(|last| format!("type:{last}"))
                .or_else(|| {
                    if before.ends_with("self.") || before.ends_with("this.") {
                        Some(format!("field:{recv}"))
                    } else {
                        None
                    }
                }),
            None => impl_parent.map(|p| format!("impl:{p}")),
        };

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
            receiver_hint,
        });
    }

    static PATH_CALL_RE: OnceLock<Regex> = OnceLock::new();
    let path_re = PATH_CALL_RE.get_or_init(|| {
        Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*::)+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap()
    });
    for cap in path_re.captures_iter(trimmed) {
        if let Some(name) = cap.get(2) {
            let name = name.as_str();
            if !is_callable_name(name) || name == caller {
                continue;
            }
            if let Some(existing) = result.relationships.iter_mut().find(|rel| {
                rel.source_symbol == caller
                    && rel.target_symbol == name
                    && rel.relationship == EdgeType::Calls
            }) {
                if existing.receiver_hint.is_none() {
                    let type_name = cap
                        .get(1)
                        .map(|m| m.as_str().trim_end_matches(':').to_string())
                        .unwrap_or_default();
                    if !type_name.is_empty() {
                        existing.receiver_hint = Some(format!("type:{type_name}"));
                    }
                }
                continue;
            }
            let type_name = cap
                .get(1)
                .map(|m| m.as_str().trim_end_matches(':').to_string())
                .unwrap_or_default();
            result.relationships.push(ParsedRelationship {
                source_symbol: caller.to_string(),
                target_symbol: name.to_string(),
                relationship: EdgeType::Calls,
                target_file_hint: None,
                receiver_hint: if type_name.is_empty() {
                    None
                } else {
                    Some(format!("type:{type_name}"))
                },
            });
        }
    }
}

/// `throw new X`, `catch (X`, and PHP `X $param` type hints — inbound to the type.
pub fn extract_type_uses_from_line(caller: &str, line: &str, result: &mut AstAnalysisResult) {
    let trimmed = strip_line_comment(line);

    static CATCH_RE: OnceLock<Regex> = OnceLock::new();
    let catch_re = CATCH_RE
        .get_or_init(|| Regex::new(r"\bcatch\s*\(\s*\\?([A-Za-z_][A-Za-z0-9_\\]*)").unwrap());
    for cap in catch_re.captures_iter(trimmed) {
        if let Some(raw) = cap.get(1) {
            record_type_use(caller, raw.as_str(), result);
        }
    }

    static KT_CATCH_RE: OnceLock<Regex> = OnceLock::new();
    let kt_catch_re = KT_CATCH_RE.get_or_init(|| {
        Regex::new(r"\bcatch\s*\(\s*[A-Za-z_][A-Za-z0-9_]*\s*:\s*([A-Za-z_][A-Za-z0-9_.]*)")
            .unwrap()
    });
    for cap in kt_catch_re.captures_iter(trimmed) {
        if let Some(raw) = cap.get(1) {
            record_type_use(caller, raw.as_str(), result);
        }
    }

    static THROW_RE: OnceLock<Regex> = OnceLock::new();
    let throw_re = THROW_RE
        .get_or_init(|| Regex::new(r"\bthrow\s+(?:new\s+)?\\?([A-Za-z_][A-Za-z0-9_\\]*)").unwrap());
    for cap in throw_re.captures_iter(trimmed) {
        if let Some(raw) = cap.get(1) {
            let name = type_basename(raw.as_str());
            if name.eq_ignore_ascii_case("new") {
                continue;
            }
            record_type_use(caller, raw.as_str(), result);
        }
    }

    static HINT_RE: OnceLock<Regex> = OnceLock::new();
    let hint_re =
        HINT_RE.get_or_init(|| Regex::new(r"\\?([A-Z][A-Za-z0-9_\\]*)\s+\$[A-Za-z_]").unwrap());
    for cap in hint_re.captures_iter(trimmed) {
        if let Some(raw) = cap.get(1) {
            record_type_use(caller, raw.as_str(), result);
        }
    }
}

fn type_basename(name: &str) -> &str {
    name.rsplit(['\\', '/', '.']).next().unwrap_or(name)
}

fn record_type_use(caller: &str, raw: &str, result: &mut AstAnalysisResult) {
    let name = type_basename(raw);
    if name.len() < 2 || name == caller || !is_callable_name(name) {
        return;
    }
    if result.relationships.iter().any(|rel| {
        rel.source_symbol == caller
            && rel.target_symbol == name
            && rel.relationship == EdgeType::Calls
    }) {
        return;
    }
    result.relationships.push(ParsedRelationship {
        source_symbol: caller.to_string(),
        target_symbol: name.to_string(),
        relationship: EdgeType::Calls,
        target_file_hint: None,
        receiver_hint: Some("type".into()),
    });
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
