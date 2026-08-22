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

    let file_re = FILE_RE.get_or_init(|| {
        Regex::new(r"(?x)(?:[A-Za-z0-9_.-]+[/\\])+[A-Za-z0-9_.-]+\.[A-Za-z0-9]+").unwrap()
    });
    let bare_file_re = BARE_FILE_RE.get_or_init(|| {
        Regex::new(r"\b[A-Za-z0-9_.-]+\.(?:rs|ts|tsx|js|jsx|py|vue|go|java|cs)\b").unwrap()
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
        if inner.contains('.') && inner.contains('/') || inner.contains('\\') {
            push_unique(&mut file_hints, inner.replace('\\', "/"));
        } else if is_code_ident(inner) {
            push_unique(&mut identifiers, inner.to_string());
        }
    }

    for cap in ident_re.captures_iter(prompt) {
        let ident = cap.get(0).unwrap().as_str();
        if is_code_ident(ident) {
            push_unique(&mut identifiers, ident.to_string());
        }
    }

    PromptAnchors {
        identifiers,
        file_hints,
    }
}

#[derive(Debug, Clone, Default)]
pub struct PromptAnchors {
    pub identifiers: Vec<String>,
    pub file_hints: Vec<String>,
}

pub fn tokenize_ident(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for chunk in name
        .split(|c: char| c == '_' || c == '-' || c == '/' || c == '\\' || c == '.' || c == ':')
        .filter(|s| !s.is_empty())
    {
        let mut current = String::new();
        let chars: Vec<char> = chunk.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if ch.is_uppercase()
                && i > 0
                && (chars[i - 1].is_lowercase()
                    || (i + 1 < chars.len() && chars[i + 1].is_lowercase()))
            {
                if !current.is_empty() {
                    tokens.push(current.to_lowercase());
                    current.clear();
                }
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

fn is_code_ident(value: &str) -> bool {
    if value.len() < 3 {
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
}
