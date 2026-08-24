use crate::types::{AstAnalysisResult, ParsedRelationship};
use neuromesh_core::EdgeType;
use regex::Regex;
use std::collections::HashMap;
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

/// Scan a whole function body so `throw $e` can see the matching `catch`.
pub fn extract_type_uses_from_body(caller: &str, body: &str, result: &mut AstAnalysisResult) {
    let mut catch_vars: HashMap<String, Vec<String>> = HashMap::new();
    for line in body.lines() {
        collect_catch_bindings(strip_line_comment(line), &mut catch_vars);
    }
    for line in body.lines() {
        extract_type_uses_from_line(caller, line, result);
        record_typed_rethrows(caller, strip_line_comment(line), &catch_vars, result);
    }
}

/// `throw new X`, `catch (X`, ternary `throw … new X`, and PHP `X $param` hints.
pub fn extract_type_uses_from_line(caller: &str, line: &str, result: &mut AstAnalysisResult) {
    let trimmed = strip_line_comment(line);

    for (types, _) in catch_clauses(trimmed) {
        for ty in types {
            record_type_use(caller, &ty, result);
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

    if trimmed.contains("throw") {
        static NEW_RE: OnceLock<Regex> = OnceLock::new();
        let new_re =
            NEW_RE.get_or_init(|| Regex::new(r"\bnew\s+\\?([A-Za-z_][A-Za-z0-9_\\]*)").unwrap());
        for cap in new_re.captures_iter(trimmed) {
            if let Some(raw) = cap.get(1) {
                record_type_use(caller, raw.as_str(), result);
            }
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

fn catch_clauses(line: &str) -> Vec<(Vec<String>, Option<String>)> {
    static CATCH_PAREN: OnceLock<Regex> = OnceLock::new();
    let catch_paren = CATCH_PAREN.get_or_init(|| Regex::new(r"\bcatch\s*\(\s*([^)]*)\)").unwrap());
    catch_paren
        .captures_iter(line)
        .filter_map(|cap| cap.get(1).map(|m| parse_catch_clause(m.as_str())))
        .collect()
}

fn parse_catch_clause(inner: &str) -> (Vec<String>, Option<String>) {
    let inner = inner.trim();
    if inner.is_empty() {
        return (Vec::new(), None);
    }
    if let Some((var, ty)) = inner.split_once(':') {
        let var = var.trim();
        if !var.is_empty()
            && var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !var.contains('$')
        {
            return (
                vec![type_basename(ty.trim()).to_string()],
                Some(var.to_string()),
            );
        }
    }

    let (types_part, php_var) = if let Some((types, var)) = inner.rsplit_once('$') {
        (
            types,
            Some(var.split_whitespace().next().unwrap_or(var).to_string()),
        )
    } else {
        (inner, None)
    };

    let mut types = Vec::new();
    let mut java_var = None;
    for piece in types_part.split('|') {
        let piece = piece.trim().trim_start_matches('\\');
        if piece.is_empty() {
            continue;
        }
        let toks: Vec<&str> = piece.split_whitespace().collect();
        if toks.len() >= 2 {
            let last = toks[toks.len() - 1];
            if last.chars().next().is_some_and(|c| c.is_lowercase()) {
                java_var = Some(last.to_string());
                types.push(type_basename(toks[0]).to_string());
                continue;
            }
        }
        types.push(type_basename(piece).to_string());
    }
    (types, php_var.or(java_var))
}

fn collect_catch_bindings(line: &str, catch_vars: &mut HashMap<String, Vec<String>>) {
    for (types, var) in catch_clauses(line) {
        if let Some(var) = var {
            catch_vars.entry(var).or_default().extend(types);
        }
    }
}

fn record_typed_rethrows(
    caller: &str,
    line: &str,
    catch_vars: &HashMap<String, Vec<String>>,
    result: &mut AstAnalysisResult,
) {
    static PHP_VAR: OnceLock<Regex> = OnceLock::new();
    let php_var =
        PHP_VAR.get_or_init(|| Regex::new(r"\bthrow\s+\$([A-Za-z_][A-Za-z0-9_]*)").unwrap());
    for cap in php_var.captures_iter(line) {
        if let Some(var) = cap.get(1) {
            if let Some(types) = catch_vars.get(var.as_str()) {
                for ty in types {
                    record_type_use(caller, ty, result);
                }
            }
        }
    }

    static BARE_VAR: OnceLock<Regex> = OnceLock::new();
    let bare_var =
        BARE_VAR.get_or_init(|| Regex::new(r"\bthrow\s+([a-z_][A-Za-z0-9_]*)\b").unwrap());
    for cap in bare_var.captures_iter(line) {
        if let Some(var) = cap.get(1) {
            let name = var.as_str();
            if name == "new" {
                continue;
            }
            if let Some(types) = catch_vars.get(name) {
                for ty in types {
                    record_type_use(caller, ty, result);
                }
            }
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
