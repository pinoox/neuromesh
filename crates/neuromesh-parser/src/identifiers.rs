use regex::Regex;
use std::sync::OnceLock;

/// Tokens that are English / prompt noise, not code identifiers.
const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "how",
    "does",
    "what",
    "this",
    "that",
    "with",
    "from",
    "into",
    "using",
    "when",
    "where",
    "which",
    "about",
    "after",
    "before",
    "should",
    "would",
    "could",
    "please",
    "make",
    "update",
    "change",
    "add",
    "fix",
    "create",
    "implement",
    "explain",
    "modify",
    "refactor",
    "remove",
    "delete",
    "rewrite",
    "improve",
    "design",
    "define",
    "plan",
    "inspect",
    "review",
    "give",
    "architect",
    "deploy",
    "configure",
    "identify",
    "scaffold",
    "build",
    "show",
    "find",
    "get",
    "set",
    "run",
    "use",
    "need",
    "want",
    "task",
    "code",
    "file",
    "function",
    "class",
    "method",
    "type",
    "module",
    "project",
    "context",
    "minimal",
    "active",
    "exact",
    "real",
    "why",
    "any",
    "all",
    "our",
    "your",
    "their",
    "have",
    "has",
    "been",
    "were",
    "was",
    "are",
    "is",
    "not",
    "can",
    "will",
    "just",
    "also",
    "than",
    "then",
    "them",
    "they",
    "you",
    "its",
];

/// English filler in a topical cluster — not a seed unless it is the only handle.
const CLUSTER_STOPWORDS: &[&str] = &[
    "action",
    "actions",
    "flow",
    "work",
    "works",
    "check",
    "checks",
    "role",
    "roles",
    "each",
    "also",
    "plus",
    "both",
    "half",
    "part",
    "piece",
    "request",
    "requests",
    "response",
    "responses",
    "produce",
    "handle",
    "handles",
    "used",
    "using",
    "make",
    "made",
    "thing",
    "things",
    "case",
    "cases",
    "logic",
    "based",
    "named",
    "called",
    "related",
    "distinct",
    "second",
    "first",
    "other",
    "another",
    "every",
    "under",
    "over",
    "through",
    "during",
    "without",
    "within",
    "between",
    "among",
    "against",
    "toward",
    "across",
    "route",
    "routes",
    "including",
    "extract",
    "intent",
    "pipeline",
    "works",
    "server",
    "start",
    "process",
];

/// English planning/imperative verbs — not code identifiers even when PascalCase.
const IMPERATIVE_VERBS: &[&str] = &[
    "design",
    "define",
    "plan",
    "inspect",
    "review",
    "give",
    "create",
    "implement",
    "build",
    "architect",
    "deploy",
    "configure",
    "identify",
    "scaffold",
    "generate",
    "develop",
    "establish",
    "outline",
    "propose",
    "prepare",
    "analyze",
    "analyse",
    "evaluate",
    "assess",
    "document",
    "describe",
    "explain",
    "modify",
    "update",
    "change",
    "add",
    "fix",
    "make",
    "show",
    "find",
    "get",
    "set",
    "run",
    "use",
    "need",
    "want",
];

/// Extract code-like identifiers, file paths, and qualified paths from a prompt.
/// Operates on the original (not lowercased) text so PascalCase survives.
pub fn extract_prompt_anchors(prompt: &str) -> PromptAnchors {
    let mut identifiers = Vec::new();
    let mut file_hints = Vec::new();

    static FILE_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_FILE_RE: OnceLock<Regex> = OnceLock::new();
    static QUAL_RE: OnceLock<Regex> = OnceLock::new();
    static IDENT_RE: OnceLock<Regex> = OnceLock::new();
    static TICK_RE: OnceLock<Regex> = OnceLock::new();
    static HOW_DOES_RE: OnceLock<Regex> = OnceLock::new();
    static DOTTED_RE: OnceLock<Regex> = OnceLock::new();
    static CALL_RE: OnceLock<Regex> = OnceLock::new();
    static NS_DOTTED_RE: OnceLock<Regex> = OnceLock::new();
    static LC_MEMBER_RE: OnceLock<Regex> = OnceLock::new();
    static METHOD_ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    static URL_RE: OnceLock<Regex> = OnceLock::new();

    let file_re = FILE_RE.get_or_init(|| {
        Regex::new(r"(?x)(?:[A-Za-z0-9_.-]+[/\\])+[A-Za-z0-9_.-]+\.[A-Za-z0-9]+").unwrap()
    });
    let bare_file_re = BARE_FILE_RE.get_or_init(|| {
        Regex::new(
            r"\b[A-Za-z0-9_.-]+\.(?:rs|ts|tsx|js|jsx|mjs|cjs|py|vue|go|java|cs|kt|kts|dart|rb|php|astro|svelte|twig|cshtml|razor|swift|css|scss|sass|less|html|htm|svg|sql|json|jsonc)\b",
        )
        .unwrap()
    });
    let qual_re = QUAL_RE.get_or_init(|| {
        Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+\b").unwrap()
    });
    let ident_re = IDENT_RE.get_or_init(|| {
        Regex::new(
            r"\b(?:[a-z][a-z0-9]*(_[a-z0-9]+)+|[a-z]+[A-Z][A-Za-z0-9]*|[A-Z][a-zA-Z0-9]{2,})\b",
        )
        .unwrap()
    });
    let tick_re = TICK_RE.get_or_init(|| Regex::new(r"`([^`]+)`").unwrap());
    let how_does_re = HOW_DOES_RE.get_or_init(|| {
        Regex::new(
            r"(?i)\bhow\s+do(?:es)?\s+(?:(?:the|a|an|this|that)\s+)?([A-Za-z_][A-Za-z0-9_]*)\b",
        )
        .unwrap()
    });
    let dotted_re = DOTTED_RE
        .get_or_init(|| Regex::new(r"\b([A-Z][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\b").unwrap());
    let call_re = CALL_RE.get_or_init(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\(\)").unwrap());
    let ns_dotted_re =
        NS_DOTTED_RE.get_or_init(|| Regex::new(r"\b[a-z]\.([A-Za-z_][A-Za-z0-9_]{2,})\b").unwrap());
    let lc_member_re = LC_MEMBER_RE
        .get_or_init(|| Regex::new(r"\b([a-z][a-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\b").unwrap());
    let method_route_re = METHOD_ROUTE_RE.get_or_init(|| {
        Regex::new(r"(?i)\b(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS|ANY|MATCH)\s+(/[^\s`'<>,]+)")
            .unwrap()
    });
    let bare_route_re = BARE_ROUTE_RE.get_or_init(|| {
        Regex::new(
            r"(?:^|[\s`'(])(/[A-Za-z0-9._~:@!$&'*+,;=%-]+(?:/[A-Za-z0-9._~:@!$&'*+,;=%-]*)*)",
        )
        .unwrap()
    });
    let url_re = URL_RE.get_or_init(|| Regex::new(r"(?i)https?://[^\s`'<>,]+").unwrap());

    for cap in file_re.captures_iter(prompt) {
        let path = cap.get(0).unwrap().as_str().replace('\\', "/");
        push_unique(&mut file_hints, path);
    }
    for cap in bare_file_re.captures_iter(prompt) {
        push_unique(&mut file_hints, cap.get(0).unwrap().as_str().to_string());
    }

    for cap in qual_re.captures_iter(prompt) {
        let path = cap.get(0).unwrap().as_str();
        push_unique(&mut identifiers, path.to_string());
        if let Some(last) = path.split("::").last() {
            push_unique(&mut identifiers, last.to_string());
        }
    }

    for cap in tick_re.captures_iter(prompt) {
        let inner = cap.get(1).unwrap().as_str().trim();
        if is_route_query(inner) {
            push_route_identifiers(&mut identifiers, inner);
        } else if inner.contains('.') && inner.contains('/') || inner.contains('\\') {
            push_unique(&mut file_hints, inner.replace('\\', "/"));
        } else if is_code_ident(inner) || is_how_does_ident(inner) {
            push_unique(&mut identifiers, inner.to_string());
        }
    }

    for cap in how_does_re.captures_iter(prompt) {
        let ident = cap.get(1).unwrap().as_str();
        if is_how_does_ident(ident) {
            push_unique(&mut identifiers, ident.to_string());
        }
    }

    for cap in dotted_re.captures_iter(prompt) {
        let owner = cap.get(1).unwrap().as_str();
        let member = cap.get(2).unwrap().as_str();
        if is_code_ident(owner) {
            push_unique(&mut identifiers, owner.to_string());
        }
        if is_code_ident(member) || is_how_does_ident(member) {
            push_unique(&mut identifiers, member.to_string());
        }
    }

    for cap in call_re.captures_iter(prompt) {
        let ident = cap.get(1).unwrap().as_str();
        if is_seedable_call(ident) {
            push_unique(&mut identifiers, ident.to_string());
        }
    }

    for cap in ns_dotted_re.captures_iter(prompt) {
        let member = cap.get(1).unwrap().as_str();
        if is_code_ident(member) || is_how_does_ident(member) {
            push_unique(&mut identifiers, member.to_string());
        }
    }

    for cap in lc_member_re.captures_iter(prompt) {
        let owner = cap.get(1).unwrap().as_str();
        let member = cap.get(2).unwrap().as_str();
        let qualified = format!("{owner}.{member}");
        push_unique(&mut identifiers, qualified);
        if is_how_does_ident(member) || is_code_ident(member) {
            push_unique(&mut identifiers, member.to_string());
        }
    }

    for cap in ident_re.captures_iter(prompt) {
        let ident = cap.get(0).unwrap().as_str();
        if is_imperative_verb(ident) && is_sentence_initial_word(prompt, ident) {
            continue;
        }
        if is_code_ident(ident) {
            push_unique(&mut identifiers, ident.to_string());
        }
    }

    for cap in method_route_re.captures_iter(prompt) {
        let method = cap.get(1).unwrap().as_str().to_uppercase();
        let path = cap
            .get(2)
            .unwrap()
            .as_str()
            .trim_end_matches(['.', ',', ')', ']', ';', ':', '!', '?']);
        if is_http_path(path) {
            push_unique(&mut identifiers, format!("{method} {path}"));
            push_unique(&mut identifiers, path.to_string());
        }
    }
    for cap in bare_route_re.captures_iter(prompt) {
        let path = cap.get(1).unwrap().as_str();
        if is_http_path(path) {
            push_unique(&mut identifiers, path.to_string());
        }
    }
    for cap in url_re.captures_iter(prompt) {
        if let Some(path) = route_from_url(cap.get(0).unwrap().as_str()) {
            push_unique(&mut identifiers, path);
        }
    }

    PromptAnchors {
        identifiers,
        file_hints,
    }
}

/// Extract backtick-quoted, dotted, and snake_case code tokens embedded in NL prompts.
pub fn extract_embedded_code_tokens(prompt: &str) -> Vec<String> {
    let anchors = extract_prompt_anchors(prompt);
    let mut out = anchors.identifiers;
    static SNAKE_RE: OnceLock<Regex> = OnceLock::new();
    let snake_re =
        SNAKE_RE.get_or_init(|| Regex::new(r"\b[a-z][a-z0-9]*(?:_[a-z0-9]+)+\b").unwrap());
    for cap in snake_re.captures_iter(prompt) {
        let token = cap.get(0).unwrap().as_str();
        if token.len() >= 4 && !is_prompt_stopword(token) {
            push_unique(&mut out, token.to_string());
        }
    }
    out.truncate(12);
    out
}

/// Split a compound prompt so each topical clause can seed independently.
///
/// Delimiters are the phrases people actually use to glue two questions
/// together (`including`, `and how`, `as well as`). Bare `and` is left
/// alone — it is too common in "login and logout".
pub fn split_task_clusters(prompt: &str) -> Vec<String> {
    static SPLIT_RE: OnceLock<Regex> = OnceLock::new();
    let split_re = SPLIT_RE.get_or_init(|| {
        Regex::new(r"(?i)\s*(?:,\s*)?(?:\band how\b|\bincluding\b|\bas well as\b|\band also\b)\s*")
            .unwrap()
    });
    let parts: Vec<String> = split_re
        .split(prompt)
        .map(|s| {
            s.trim()
                .trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':')
                .trim()
                .to_string()
        })
        .filter(|s| s.len() >= 8)
        .collect();
    if parts.len() <= 1 {
        vec![prompt.to_string()]
    } else {
        parts
    }
}

/// Distinctive lowercase nouns from a cluster that has no code-like identifier.
/// Longest first, capped so English filler cannot flood seed resolution.
pub fn extract_cluster_nouns(cluster: &str) -> Vec<String> {
    let mut nouns = Vec::new();
    for token in cluster.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if !is_cluster_noun(token) {
            continue;
        }
        push_unique(&mut nouns, token.to_string());
    }
    nouns.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    nouns.truncate(4);
    nouns
}

#[derive(Debug, Clone, Default)]
pub struct PromptAnchors {
    pub identifiers: Vec<String>,
    pub file_hints: Vec<String>,
}

/// Morphological variants so "parsing" can seed `parse` and "validation" can
/// seed `validate` when the gerund/noun itself is not a symbol.
pub fn stem_search_queries(query: &str) -> Vec<String> {
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    if q.len() >= 7 && q.ends_with("ing") {
        let stem = &q[..q.len() - 3];
        push_unique(&mut out, stem.to_string());
        push_unique(&mut out, format!("{stem}e"));
    }
    if q.len() >= 9 && q.ends_with("ation") {
        let stem = &q[..q.len() - 5];
        push_unique(&mut out, stem.to_string());
        push_unique(&mut out, format!("{stem}e"));
        push_unique(&mut out, format!("{stem}ate"));
    } else if q.len() >= 8 && q.ends_with("tion") {
        let stem = &q[..q.len() - 4];
        push_unique(&mut out, stem.to_string());
        push_unique(&mut out, format!("{stem}e"));
    }
    if q == "infer" {
        push_unique(&mut out, "output".into());
        push_unique(&mut out, "input".into());
    }
    out.retain(|s| s.len() >= 4 && s != &q);
    out
}

pub fn tokenize_ident(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for chunk in name
        .split(['_', '-', '/', '\\', '.', ':'])
        .filter(|s| !s.is_empty())
    {
        let mut current = String::new();
        let chars: Vec<char> = chunk.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if ch.is_uppercase()
                && i > 0
                && (chars[i - 1].is_lowercase()
                    || (i + 1 < chars.len() && chars[i + 1].is_lowercase()))
                && !current.is_empty()
            {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            current.push(ch);
        }
        if !current.is_empty() {
            tokens.push(current.to_lowercase());
        }
    }
    tokens.retain(|t| t.len() > 1);
    tokens
}

const ROUTE_METHODS: &[&str] = &[
    "GET",
    "POST",
    "PUT",
    "PATCH",
    "DELETE",
    "HEAD",
    "OPTIONS",
    "ANY",
    "MATCH",
    "API-RESOURCE",
    "RESOURCE",
    "COLLECTION",
];

/// True when `query` is an HTTP route (`POST /sms`, `/sms`), not a file path.
pub fn is_route_query(query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return false;
    }
    if let Some(path) = strip_method_prefix(q) {
        return is_http_path(path);
    }
    is_http_path(q)
}

/// Path alias for an Api node named `POST /sms` → `Some("/sms")`.
/// Never the last path segment alone (`sms`).
pub fn api_path_alias(name: &str) -> Option<String> {
    let path = strip_method_prefix(name.trim())?;
    if is_http_path(path) {
        Some(path.to_string())
    } else {
        None
    }
}

fn strip_method_prefix(name: &str) -> Option<&str> {
    let (method, rest) = name.split_once(' ')?;
    if !ROUTE_METHODS
        .iter()
        .any(|wanted| method.eq_ignore_ascii_case(wanted))
    {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest)
}

fn is_http_path(path: &str) -> bool {
    if !path.starts_with('/') {
        return false;
    }
    if path.contains('\\') {
        return false;
    }
    !http_path_has_source_extension(path)
}

fn http_path_has_source_extension(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or(path);
    let Some((_, ext)) = last.rsplit_once('.') else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "py"
            | "vue"
            | "go"
            | "java"
            | "cs"
            | "kt"
            | "kts"
            | "dart"
            | "rb"
            | "php"
            | "astro"
            | "svelte"
            | "twig"
            | "cshtml"
            | "razor"
            | "swift"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "html"
            | "htm"
            | "svg"
            | "sql"
            | "json"
            | "jsonc"
            | "toml"
            | "md"
    )
}

fn route_from_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim_end_matches(['.', ',', ')', ']', ';', '!', '"', '\'']);
    let after_scheme = trimmed.split_once("://")?.1;
    let host_and_path = after_scheme
        .split_once('?')
        .map(|(head, _)| head)
        .unwrap_or(after_scheme);
    let host_and_path = host_and_path
        .split_once('#')
        .map(|(head, _)| head)
        .unwrap_or(host_and_path);
    let slash = host_and_path.find('/')?;
    let host = &host_and_path[..slash];
    let mut path = &host_and_path[slash..];
    if path.len() > 1 {
        path = path.trim_end_matches('/');
    }
    if path.is_empty() || path == "/" {
        return None;
    }
    if is_git_web_host(host) && path_segment_count(path) == 2 {
        return None;
    }
    if is_http_path(path) {
        Some(path.to_string())
    } else {
        None
    }
}

fn is_git_web_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(host.as_str());
    host == "github.com" || host == "gitlab.com" || host == "bitbucket.org"
}

fn path_segment_count(path: &str) -> usize {
    path.trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .count()
}

fn push_route_identifiers(identifiers: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim();
    if let Some(path) = strip_method_prefix(trimmed) {
        if is_http_path(path) {
            let method = trimmed.split_once(' ').unwrap().0.to_uppercase();
            push_unique(identifiers, format!("{method} {path}"));
            push_unique(identifiers, path.to_string());
        }
        return;
    }
    if is_http_path(trimmed) {
        push_unique(identifiers, trimmed.to_string());
    }
}

fn is_cluster_noun(value: &str) -> bool {
    if value.starts_with("__") || value.contains("__") {
        return false;
    }
    if value.len() < 5 || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    let lower = value.to_lowercase();
    if STOPWORDS.contains(&lower.as_str()) || CLUSTER_STOPWORDS.contains(&lower.as_str()) {
        return false;
    }
    true
}

fn is_seedable_call(value: &str) -> bool {
    if !is_how_does_ident(value) {
        return false;
    }
    !STOPWORDS.contains(&value.to_lowercase().as_str())
}

/// True for English planning/imperative verbs (any case).
pub fn is_imperative_verb(value: &str) -> bool {
    if value.len() < 3 {
        return false;
    }
    IMPERATIVE_VERBS.contains(&value.to_lowercase().as_str())
}

fn is_sentence_initial_word(prompt: &str, word: &str) -> bool {
    let trimmed = prompt.trim_start();
    trimmed
        .strip_prefix(word)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| !c.is_alphanumeric()))
}

/// English / prompt noise — not a fallback seed token.
pub fn is_prompt_stopword(value: &str) -> bool {
    let lower = value.trim().to_lowercase();
    if lower.len() < 3 {
        return true;
    }
    STOPWORDS.contains(&lower.as_str()) || CLUSTER_STOPWORDS.contains(&lower.as_str())
}

fn is_code_ident(value: &str) -> bool {
    if value.len() < 3 {
        return false;
    }
    if is_imperative_verb(value) {
        return false;
    }
    let lower = value.to_lowercase();
    if STOPWORDS.contains(&lower.as_str()) {
        return false;
    }
    value.contains('_')
        || value.chars().any(|c| c.is_uppercase())
        || value.contains("::")
        || value.contains('/')
}

/// Subject of "how does X use Y" — keep real method names even when they are
/// lowercase English (`store`, `create`) that `is_code_ident` would drop.
fn is_how_does_ident(value: &str) -> bool {
    if value.len() < 3 || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    !matches!(
        value.to_lowercase().as_str(),
        "the"
            | "and"
            | "for"
            | "how"
            | "does"
            | "what"
            | "this"
            | "that"
            | "with"
            | "from"
            | "into"
            | "using"
            | "when"
            | "where"
            | "which"
            | "about"
            | "user"
            | "users"
            | "app"
            | "page"
            | "view"
            | "file"
            | "code"
    )
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if value.is_empty() {
        return;
    }
    if !list.iter().any(|existing| existing == &value) {
        list.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_snake_and_path_from_real_prompt() {
        let anchors = extract_prompt_anchors(
            "How does neuromesh_get_context extract task intent from crates/neuromesh-mcp/src/tools.rs?",
        );
        assert!(
            anchors
                .identifiers
                .iter()
                .any(|id| id == "neuromesh_get_context"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            anchors.file_hints.iter().any(|p| p.contains("tools.rs")),
            "file_hints = {:?}",
            anchors.file_hints
        );
    }

    #[test]
    fn drops_imperative_verbs_and_keeps_file_path() {
        let anchors = extract_prompt_anchors("Modify theme/default/hello.twig to change the title");
        assert!(
            !anchors
                .identifiers
                .iter()
                .any(|id| id.eq_ignore_ascii_case("modify")),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            anchors
                .file_hints
                .iter()
                .any(|p| p.contains("theme/default/hello.twig")),
            "file_hints = {:?}",
            anchors.file_hints
        );
    }

    #[test]
    fn tokenizes_camel_and_snake() {
        let tokens = tokenize_ident("handle_tool_call");
        assert!(tokens.contains(&"handle".into()));
        assert!(tokens.contains(&"tool".into()));
        assert!(tokens.contains(&"call".into()));

        let camel = tokenize_ident("NeuralProjectGraph");
        assert!(camel.contains(&"neural".into()));
        assert!(camel.contains(&"project".into()));
        assert!(camel.contains(&"graph".into()));
    }

    #[test]
    fn how_does_keeps_lowercase_method_and_dotted_save() {
        let astro = extract_prompt_anchors("How does store use saveSms?");
        assert!(
            astro.identifiers.iter().any(|id| id == "store"),
            "identifiers = {:?}",
            astro.identifiers
        );
        assert!(astro.identifiers.iter().any(|id| id == "saveSms"));

        let rails = extract_prompt_anchors("How does create use SmsStore.save?");
        assert!(
            rails.identifiers.iter().any(|id| id == "create"),
            "identifiers = {:?}",
            rails.identifiers
        );
        assert!(rails.identifiers.iter().any(|id| id == "SmsStore"));
        assert!(rails.identifiers.iter().any(|id| id == "save"));

        let csharp = extract_prompt_anchors("How does OnReceive use SmsStore.Save?");
        assert!(csharp.identifiers.iter().any(|id| id == "OnReceive"));
        assert!(csharp.identifiers.iter().any(|id| id == "SmsStore"));
        assert!(csharp.identifiers.iter().any(|id| id == "Save"));
    }

    #[test]
    fn extracts_stylesheet_and_svg_file_hints() {
        let anchors = extract_prompt_anchors(
            "How does smsBadge use smsUnread in styles/sms.less and assets/sms-inbox.svg?",
        );
        assert!(anchors.file_hints.iter().any(|p| p.contains("sms.less")));
        assert!(anchors
            .file_hints
            .iter()
            .any(|p| p.contains("sms-inbox.svg")));
        assert!(anchors.identifiers.iter().any(|id| id == "smsBadge"));
        assert!(anchors.identifiers.iter().any(|id| id == "smsUnread"));
    }

    #[test]
    fn extracts_json_and_sql_file_hints() {
        let anchors = extract_prompt_anchors(
            "Where is smsFrom defined in config/sms.json and sms_messages in sms_messages.sql?",
        );
        assert!(anchors.file_hints.iter().any(|p| p.contains("sms.json")));
        assert!(anchors
            .file_hints
            .iter()
            .any(|p| p.contains("sms_messages.sql")));
    }

    #[test]
    fn splits_compound_auth_and_guard_clusters() {
        let prompt = "how does the user login and logout flow work, including the login action, getInfo action, and how the router permission guard checks roles before each route";
        let clusters = split_task_clusters(prompt);
        assert!(
            clusters.len() >= 2,
            "compound task must split, got {clusters:?}"
        );
        let last = clusters.last().unwrap().to_lowercase();
        assert!(
            last.contains("permission") && last.contains("guard"),
            "last cluster should be the guard clause: {clusters:?}"
        );
        let nouns = extract_cluster_nouns(clusters.last().unwrap());
        assert!(
            nouns.iter().any(|n| n.eq_ignore_ascii_case("permission")),
            "nouns = {nouns:?}"
        );
        assert!(
            nouns.iter().any(|n| n.eq_ignore_ascii_case("router")),
            "nouns = {nouns:?}"
        );
        let anchors = extract_prompt_anchors(prompt);
        assert!(
            anchors.identifiers.iter().any(|id| id == "getInfo"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            !anchors
                .identifiers
                .iter()
                .any(|id| id.eq_ignore_ascii_case("permission")),
            "cluster nouns stay out of identifier extraction: {:?}",
            anchors.identifiers
        );
        assert!(
            !anchors
                .identifiers
                .iter()
                .any(|id| id.eq_ignore_ascii_case("user")),
            "English 'the user' is not a how-does seed: {:?}",
            anchors.identifiers
        );
    }

    #[test]
    fn extracts_lowercase_member_access() {
        let anchors = extract_prompt_anchors(
            "how does app.handle process middleware and how does app.listen start the server, including init",
        );
        assert!(
            anchors.identifiers.iter().any(|id| id == "app.handle"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            anchors.identifiers.iter().any(|id| id == "app.listen"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            anchors.identifiers.iter().any(|id| id == "handle"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            anchors.identifiers.iter().any(|id| id == "listen"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            !anchors.identifiers.iter().any(|id| id == "does"),
            "identifiers = {:?}",
            anchors.identifiers
        );
    }

    #[test]
    fn middleware_prompt_extracts_next_and_nouns() {
        let anchors =
            extract_prompt_anchors("Explain the middleware pipeline and how next() works");
        assert!(
            anchors.identifiers.iter().any(|id| id == "next"),
            "identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            !anchors.identifiers.iter().any(|id| id == "does"),
            "identifiers = {:?}",
            anchors.identifiers
        );
    }

    #[test]
    fn extracts_call_and_single_letter_namespace() {
        let anchors = extract_prompt_anchors(
            "how does z.object schema validate an object and how does parse() report validation errors",
        );
        assert!(
            anchors.identifiers.iter().any(|id| id == "parse"),
            "parse() must be a seed, identifiers = {:?}",
            anchors.identifiers
        );
        assert!(
            anchors.identifiers.iter().any(|id| id == "object"),
            "z.object must seed object, identifiers = {:?}",
            anchors.identifiers
        );
        let infer = extract_prompt_anchors("how do ZodType generics flow through z.infer");
        assert!(
            infer.identifiers.iter().any(|id| id == "infer"),
            "z.infer must seed infer, identifiers = {:?}",
            infer.identifiers
        );
        assert!(
            infer.identifiers.iter().any(|id| id == "ZodType"),
            "ZodType must be a seed, identifiers = {:?}",
            infer.identifiers
        );
    }

    #[test]
    fn stems_gerunds_and_ations() {
        let parsing = stem_search_queries("parsing");
        assert!(
            parsing.iter().any(|s| s == "parse"),
            "parsing stems = {parsing:?}"
        );
        let validation = stem_search_queries("validation");
        assert!(
            validation.iter().any(|s| s == "validate"),
            "validation stems = {validation:?}"
        );
        let infer = stem_search_queries("infer");
        assert!(
            infer.iter().any(|s| s == "output"),
            "infer aliases = {infer:?}"
        );
    }

    #[test]
    fn design_imperative_not_an_identifier() {
        let anchors = extract_prompt_anchors(
            "Design the product catalog domain for products, categories, brands, and product status.",
        );
        assert!(
            !anchors.identifiers.iter().any(|id| id == "Design"),
            "identifiers = {:?}",
            anchors.identifiers
        );
    }

    #[test]
    fn extracts_http_routes_not_file_hints() {
        let method = extract_prompt_anchors("how does POST /sms persist the message?");
        assert!(
            method.identifiers.iter().any(|id| id == "POST /sms"),
            "identifiers = {:?}",
            method.identifiers
        );
        assert!(
            method.identifiers.iter().any(|id| id == "/sms"),
            "identifiers = {:?}",
            method.identifiers
        );
        assert!(
            !method.file_hints.iter().any(|h| h.contains("/sms")),
            "file_hints = {:?}",
            method.file_hints
        );

        let nested = extract_prompt_anchors("where is /api/v1/sms handled?");
        assert!(
            nested.identifiers.iter().any(|id| id == "/api/v1/sms"),
            "identifiers = {:?}",
            nested.identifiers
        );

        let url = extract_prompt_anchors("trace https://example.com/sms from the client");
        assert!(
            url.identifiers.iter().any(|id| id == "/sms"),
            "identifiers = {:?}",
            url.identifiers
        );

        let file = extract_prompt_anchors(
            "How does neuromesh_get_context extract task intent from crates/neuromesh-mcp/src/tools.rs?",
        );
        assert!(
            file.file_hints.iter().any(|p| p.contains("tools.rs")),
            "file_hints = {:?}",
            file.file_hints
        );
        assert!(
            !file
                .identifiers
                .iter()
                .any(|id| id.starts_with('/') && !id.contains('.')),
            "file path must not become a route identifier: {:?}",
            file.identifiers
        );

        let github = extract_prompt_anchors("see https://github.com/pinoox/pinoox for the app");
        assert!(
            !github
                .identifiers
                .iter()
                .any(|id| id == "/pinoox/pinoox" || id == "/pinoox"),
            "github repo URL is not a route seed: {:?}",
            github.identifiers
        );

        assert!(is_route_query("POST /sms"));
        assert!(is_route_query("/sms"));
        assert!(!is_route_query("crates/neuromesh-mcp/src/tools.rs"));
        assert_eq!(api_path_alias("POST /sms").as_deref(), Some("/sms"));
        assert_eq!(api_path_alias("store"), None);
    }
}
