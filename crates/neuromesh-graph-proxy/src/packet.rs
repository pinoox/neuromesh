use neuromesh_core::TaskSignature;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxySearchContext {
    pub raw_prompt: String,
    pub identifiers: Vec<String>,
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

    /// Terms we expect to see reflected in proxy hits (for honest metadata).
    pub fn expected_terms(&self) -> Vec<String> {
        let mut out = Vec::new();
        for t in self
            .client_keywords
            .iter()
            .chain(self.identifiers.iter())
            .chain(self.client_expansion.iter())
        {
            let lower = t.to_lowercase();
            if lower.len() >= 3 && !out.iter().any(|x| x == &lower) {
                out.push(lower);
            }
        }
        out
    }
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
