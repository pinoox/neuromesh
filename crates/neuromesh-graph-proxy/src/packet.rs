use neuromesh_core::TaskSignature;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxySearchContext {
    pub raw_prompt: String,
    pub identifiers: Vec<String>,
    pub related_concepts: Vec<String>,
    pub client_keywords: Vec<String>,
    pub client_expansion: Vec<String>,
    pub path_hints: Vec<String>,
}

impl ProxySearchContext {
    pub fn from_prompt(task: impl Into<String>) -> Self {
        Self {
            raw_prompt: task.into(),
            ..Default::default()
        }
    }

    pub fn from_task_signature(sig: &TaskSignature) -> Self {
        Self {
            raw_prompt: sig.raw_prompt.clone(),
            identifiers: sig.identifiers.clone(),
            related_concepts: sig.related_concepts.clone(),
            client_keywords: sig.client_keywords.clone(),
            client_expansion: sig.client_expansion.clone(),
            path_hints: sig
                .client_path_hints
                .iter()
                .chain(sig.file_hints.iter())
                .cloned()
                .collect(),
        }
    }

    /// BM25 query for CBM — extracted terms only; never the raw NL sentence.
    pub fn cbm_query_string(&self) -> String {
        let mut parts = Vec::new();
        for term in self
            .identifiers
            .iter()
            .chain(self.client_keywords.iter())
            .chain(self.related_concepts.iter())
        {
            if proxy_query_noise(term) {
                continue;
            }
            let normalized = term.trim();
            if normalized.is_empty() {
                continue;
            }
            if !parts
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(normalized))
            {
                parts.push(normalized.to_string());
            }
        }
        if parts.is_empty() {
            return fallback_stripped_prompt(&self.raw_prompt);
        }
        parts.join(" ")
    }

    /// Semantic query payload for CBM when expansion/concepts are available.
    pub fn cbm_semantic_terms(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for term in self
            .client_expansion
            .iter()
            .chain(self.related_concepts.iter())
        {
            if proxy_query_noise(term) {
                continue;
            }
            if !out
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(term))
            {
                out.push(term.clone());
            }
        }
        out.truncate(8);
        out
    }

    /// Terms we expect to see reflected in proxy hits (for honest metadata).
    pub fn expected_terms(&self) -> Vec<String> {
        let mut out = Vec::new();
        for t in self
            .client_keywords
            .iter()
            .chain(self.identifiers.iter())
            .chain(self.client_expansion.iter())
            .chain(self.related_concepts.iter())
        {
            if proxy_query_noise(t) {
                continue;
            }
            let lower = t.to_lowercase();
            if lower.len() >= 3 && !out.iter().any(|x| x == &lower) {
                out.push(lower);
            }
        }
        out
    }
}

const PROXY_STOPWORDS: &[&str] = &[
    "how", "does", "what", "when", "where", "which", "about", "with", "from", "into", "that",
    "this", "the", "and", "for", "are", "was", "were", "been", "being", "have", "has", "had",
    "will", "would", "should", "could", "many", "much", "more", "most", "some", "such", "than",
    "then", "them", "they", "their", "there", "these", "those", "your", "after", "before",
    "system", "file", "files", "code", "user", "users", "app", "page", "view", "using", "use",
    "get", "gets", "make", "makes", "work", "works", "does", "explain", "describe", "tell",
];

fn proxy_query_noise(term: &str) -> bool {
    let lower = term.trim().to_lowercase();
    if lower.len() < 3 {
        return true;
    }
    if PROXY_STOPWORDS.contains(&lower.as_str()) {
        return true;
    }
    // Avoid leaking NeuroMesh's own NL helper name into CBM symbol search.
    lower.contains("how_does") || lower == "is_how_does_ident"
}

/// Strip question framing and keep substantive tokens for a last-resort CBM query.
fn fallback_stripped_prompt(raw: &str) -> String {
    let mut s = raw.to_lowercase();
    for prefix in [
        "how does the system ",
        "how does the ",
        "how do the ",
        "how does ",
        "how do ",
        "what is the ",
        "what is ",
        "explain how the ",
        "explain how ",
        "explain the ",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    s = s.trim_end_matches('?').trim().to_string();
    s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
                .to_string()
        })
        .filter(|w| w.len() >= 4 && !proxy_query_noise(w))
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxyRetrievalHints {
    pub matched_terms: Vec<String>,
    pub missed_terms: Vec<String>,
    pub critical_gaps: Vec<String>,
    pub suggested_keywords: Vec<String>,
    pub confidence: f32,
    pub sufficiency_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyContextPacket {
    pub task: String,
    pub provider: String,
    pub coverage: String,
    pub files: Vec<ProxyContextFile>,
    pub packet_tokens: usize,
    pub symbols_found: usize,
    #[serde(default)]
    pub retrieval: ProxyRetrievalHints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyContextFile {
    pub path: String,
    pub code: String,
    pub tokens: usize,
    pub why: String,
    pub qualified_name: Option<String>,
}

pub fn compute_retrieval_hints(
    ctx: &ProxySearchContext,
    files: &[ProxyContextFile],
) -> ProxyRetrievalHints {
    let expected = ctx.expected_terms();
    let expected_len = expected.len();
    let blob = files
        .iter()
        .map(|f| {
            format!(
                "{} {} {}",
                f.path,
                f.why,
                f.qualified_name.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    let mut matched = Vec::new();
    let mut missed = Vec::new();
    for term in &expected {
        if blob.contains(term) {
            matched.push(term.clone());
        } else {
            missed.push(term.clone());
        }
    }

    let denom = expected_len.max(1) as f32;
    let match_ratio = matched.len() as f32 / denom;
    let confidence = if files.is_empty() {
        0.15
    } else {
        (0.2 + match_ratio * 0.25).min(0.45)
    };
    let sufficiency_score = if files.is_empty() {
        0.0
    } else {
        (0.15 + match_ratio * 0.3).min(0.45)
    };

    ProxyRetrievalHints {
        matched_terms: matched,
        missed_terms: missed.clone(),
        critical_gaps: missed.clone(),
        suggested_keywords: missed,
        confidence,
        sufficiency_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbm_query_strips_how_does_framing() {
        let ctx = ProxySearchContext {
            raw_prompt: "How does the system estimate tokens?".into(),
            client_keywords: vec!["estimate".into(), "tokens".into()],
            ..Default::default()
        };
        let query = ctx.cbm_query_string();
        assert!(query.contains("estimate"));
        assert!(!query.to_lowercase().contains("how does"));
    }

    #[test]
    fn fallback_strips_question_words() {
        let q = fallback_stripped_prompt(
            "How does the system decide the maximum number of files to index automatically?",
        );
        assert!(q.contains("decide") || q.contains("maximum") || q.contains("index"));
        assert!(!q.contains("how"));
    }
}
