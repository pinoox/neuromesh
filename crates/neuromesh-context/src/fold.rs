//! Task-conditioned fold policy: which bodies stay exons, unique fold ids,
//! and how `expand_fold` looks those ids back up.

use neuromesh_core::TaskSignature;
use std::collections::HashSet;

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
    "every",
    "being",
    "been",
    "were",
    "was",
    "are",
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
    "now",
    "but",
    "our",
    "your",
    "have",
    "has",
    "added",
    "like",
    "exactly",
    "could",
    "cause",
    "treated",
    "during",
    "started",
    "registered",
    "custom",
    "class",
    "objects",
    "fields",
    "entirely",
    "output",
    "example",
];

/// Names the skeletonizer should keep open, plus prompt tokens for scoring.
#[derive(Debug, Clone)]
pub struct FoldPolicy {
    pub active_symbols: HashSet<String>,
    pub ident_tokens: HashSet<String>,
    pub prompt_tokens: HashSet<String>,
    pub verb_exons: HashSet<String>,
    pub exon_budget: usize,
    /// Resolved seed / callee names. Rank above prompt identifiers so K
    /// never folds the method the packet is for.
    pub priority_symbols: HashSet<String>,
}

pub const SEED_EXON_BUDGET: usize = 4;
pub const OPTIONAL_EXON_BUDGET: usize = 1;
/// Weak lexical hits below this stay folded even when K has room.
const EXON_SCORE_FLOOR: f32 = 25.0;

impl Default for FoldPolicy {
    fn default() -> Self {
        Self {
            active_symbols: HashSet::new(),
            ident_tokens: HashSet::new(),
            prompt_tokens: HashSet::new(),
            verb_exons: HashSet::new(),
            exon_budget: SEED_EXON_BUDGET,
            priority_symbols: HashSet::new(),
        }
    }
}

impl FoldPolicy {
    pub fn from_symbols(symbols: &HashSet<String>) -> Self {
        let mut active_symbols = HashSet::new();
        let mut ident_tokens = HashSet::new();
        for s in symbols {
            let lower = s.to_lowercase();
            active_symbols.insert(lower.clone());
            active_symbols.insert(s.clone());
            for tok in tokenize_name(s) {
                ident_tokens.insert(tok);
            }
        }
        Self {
            active_symbols: active_symbols.clone(),
            ident_tokens,
            prompt_tokens: HashSet::new(),
            verb_exons: HashSet::new(),
            exon_budget: SEED_EXON_BUDGET,
            priority_symbols: active_symbols,
        }
    }

    pub fn from_task(symbols: &HashSet<String>, signature: &TaskSignature) -> Self {
        let mut policy = Self::from_symbols(symbols);
        policy.priority_symbols.clear();
        for ident in &signature.identifiers {
            policy.active_symbols.insert(ident.to_lowercase());
            for tok in tokenize_name(ident) {
                policy.ident_tokens.insert(tok);
            }
        }
        if !signature.entity.is_empty() {
            policy
                .active_symbols
                .insert(signature.entity.to_lowercase());
            for tok in tokenize_name(&signature.entity) {
                policy.ident_tokens.insert(tok);
            }
        }
        for concept in &signature.related_concepts {
            for tok in tokenize_name(concept) {
                policy.ident_tokens.insert(tok);
            }
        }
        policy.prompt_tokens = prompt_focus_tokens(&signature.raw_prompt, &policy.ident_tokens);
        policy.verb_exons = infer_verb_exons(&signature.raw_prompt);
        policy
    }

    pub fn with_exon_budget(mut self, budget: usize) -> Self {
        self.exon_budget = budget.max(1);
        self
    }

    pub fn with_priority_symbols(mut self, names: HashSet<String>) -> Self {
        for name in names {
            self.priority_symbols.insert(name.to_lowercase());
            self.priority_symbols.insert(name);
        }
        self
    }

    /// Pick at most `exon_budget` bodies, highest score first.
    /// Exact seeds (100) rank above compound/verb hits, so reducing K
    /// never folds the top-scored method to make room for a weaker one.
    pub fn select_exons(&self, scores: &[f32]) -> HashSet<usize> {
        let budget = self.exon_budget.max(1);
        let mut order: Vec<(f32, usize)> = scores
            .iter()
            .copied()
            .enumerate()
            .map(|(idx, score)| (score, idx))
            .collect();
        order.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        let mut picked = HashSet::new();
        for (score, idx) in &order {
            if picked.len() >= budget {
                break;
            }
            if *score >= 100.0 || *score >= EXON_SCORE_FLOOR {
                picked.insert(*idx);
                continue;
            }
            if picked.is_empty() && *score > 0.0 {
                picked.insert(*idx);
            }
        }
        picked
    }

    pub fn keep_open(&self, name: &str, owner: Option<&str>, body: &str) -> bool {
        if is_seed_exon(name, &self.active_symbols) {
            return true;
        }
        if owner.is_some_and(|owner| self.compound_type_match(owner)) {
            return true;
        }
        self.method_matches_task(name, owner, body)
    }

    pub fn score(&self, name: &str, owner: Option<&str>, signature: &str, body: &str) -> f32 {
        if is_seed_exon(name, &self.priority_symbols) {
            return 200.0;
        }
        if is_seed_exon(name, &self.active_symbols) {
            return 100.0;
        }
        let mut score = 0.0;
        let name_hits = self.ident_hits(&tokenize_name(name));
        let owner_tokens = owner.map(tokenize_name).unwrap_or_default();
        let owner_hits = self.focus_hits(&owner_tokens);
        let body_hits = self.focus_hits(&tokenize_name(body));
        let sig_hits = self.ident_hits(&tokenize_name(signature));
        let verb = self.verb_exons.contains(&name.to_lowercase());

        if owner.is_some_and(|o| self.compound_type_match(o)) {
            score += 40.0;
        }
        if verb && owner_hits >= 2 {
            score += 25.0;
        } else if verb && body_hits >= 2 {
            score += 20.0;
        } else if verb {
            score += 8.0;
        }
        score += (name_hits as f32) * 8.0;
        score += (owner_hits as f32) * 6.0;
        score += (sig_hits as f32) * 3.0;
        score += (body_hits as f32) * 2.0;
        score
    }

    fn method_matches_task(&self, name: &str, owner: Option<&str>, body: &str) -> bool {
        let name_l = name.to_lowercase();
        let verb = self.verb_exons.contains(&name_l);
        let owner_hits = owner
            .map(|o| self.focus_hits(&tokenize_name(o)))
            .unwrap_or(0);
        let name_hits = self.ident_hits(&tokenize_name(name));
        if verb && owner_hits >= 2 {
            return true;
        }
        if verb && name_hits >= 1 {
            return true;
        }
        if name_hits >= 2 {
            return true;
        }
        if verb {
            let body_hits = self.focus_hits(&tokenize_name(body));
            if body_hits >= 2 {
                return true;
            }
        }
        false
    }

    fn compound_type_match(&self, owner: &str) -> bool {
        let tokens = tokenize_name(owner);
        if tokens.len() < 3 {
            return false;
        }
        let hits = self.focus_hits(&tokens);
        hits >= 3 && hits * 2 >= tokens.len()
    }

    fn ident_hits(&self, tokens: &[String]) -> usize {
        token_hits(tokens, &self.ident_tokens)
    }

    fn focus_hits(&self, tokens: &[String]) -> usize {
        tokens
            .iter()
            .filter(|t| {
                t.len() > 2
                    && (self.ident_tokens.contains(t.as_str())
                        || self.prompt_tokens.contains(t.as_str()))
            })
            .count()
    }
}

pub fn is_seed_exon(sym_name: &str, active_symbols: &HashSet<String>) -> bool {
    let lower = sym_name.to_lowercase();
    active_symbols.contains(sym_name)
        || active_symbols.contains(&lower)
        || active_symbols
            .iter()
            .any(|s| s.eq_ignore_ascii_case(sym_name) || s.rsplit("::").next() == Some(sym_name))
}

pub fn make_fold_id(file_path: &str, symbol: &str, ordinal: usize, start_line: usize) -> String {
    let name = sanitize_ident(symbol);
    let tag = path_tag(file_path, start_line);
    format!("fold_{name}_{ordinal}_{tag}")
}

/// Pull a `fold_*` handle out of a raw tool argument (bare id, marker line, or quotes).
pub fn normalize_fold_query(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(rest) = trimmed.split("neuromesh:fold:").nth(1) {
        let id = rest
            .split(|c: char| c.is_whitespace() || c == '|' || c == ']' || c == ',')
            .next()
            .unwrap_or("")
            .trim();
        if id.starts_with("fold_") {
            return id.to_string();
        }
    }
    trimmed.to_string()
}

pub fn tokenize_name(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for chunk in name
        .split([
            '_', '-', '/', '\\', '.', ':', '(', ')', '<', '>', ',', ';', '"', '\'', '`',
        ])
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
                push_token(&mut tokens, &current);
                current.clear();
            }
            current.push(ch);
        }
        if !current.is_empty() {
            push_token(&mut tokens, &current);
        }
    }
    tokens.retain(|t| t.len() > 1);
    tokens
}

fn push_token(tokens: &mut Vec<String>, raw: &str) {
    let tok = raw.to_lowercase();
    if tok.len() > 1 {
        tokens.push(tok);
    }
}

fn token_hits(tokens: &[String], focus: &HashSet<String>) -> usize {
    tokens
        .iter()
        .filter(|t| t.len() > 2 && focus.contains(t.as_str()))
        .count()
}

fn prompt_focus_tokens(prompt: &str, ident_tokens: &HashSet<String>) -> HashSet<String> {
    let mut out = ident_tokens.clone();
    for raw in prompt.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        let w = raw.to_lowercase();
        if w.len() >= 5 && !STOPWORDS.contains(&w.as_str()) {
            out.insert(w.clone());
        }
        if matches!(
            w.as_str(),
            "null" | "json" | "xml" | "http" | "sql" | "write" | "read"
        ) {
            out.insert(w);
        }
    }
    out
}

fn infer_verb_exons(prompt: &str) -> HashSet<String> {
    let p = prompt.to_lowercase();
    let mut verbs = HashSet::new();
    if p.contains("serializ")
        || p.contains("tojson")
        || p.contains("to_json")
        || p.contains("json output")
        || word_present(&p, "write")
    {
        for name in ["write", "serialize", "tojson", "to_json", "encode", "dump"] {
            verbs.insert(name.to_string());
        }
    }
    if p.contains("deserializ")
        || p.contains("fromjson")
        || p.contains("from_json")
        || p.contains("parse json")
        || word_present(&p, "read")
    {
        for name in [
            "read",
            "deserialize",
            "fromjson",
            "from_json",
            "decode",
            "parse",
        ] {
            verbs.insert(name.to_string());
        }
    }
    if p.contains("nullsafe") || p.contains("null-safe") || p.contains("null safe") {
        verbs.insert("nullsafe".into());
        verbs.insert("null_safe".into());
    }
    verbs
}

fn word_present(haystack: &str, word: &str) -> bool {
    haystack
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|w| w == word)
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
        if out.len() >= 40 {
            break;
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "sym".into()
    } else {
        trimmed.to_string()
    }
}

fn path_tag(file_path: &str, start_line: usize) -> String {
    let mut h: u32 = 5381;
    for b in file_path.replace('\\', "/").bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    h = h.wrapping_mul(33).wrapping_add(start_line as u32);
    format!("{:05x}", h & 0x000f_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::{TaskIntent, TaskRisk};

    fn gson_bug_signature() -> TaskSignature {
        TaskSignature {
            id: "gson-nullsafe".into(),
            intent: TaskIntent::Fix,
            domain: "general".into(),
            technology: "Fullstack".into(),
            style: None,
            entity: "TypeAdapter".into(),
            goal: "bug fix".into(),
            risk: TaskRisk::Low,
            related_concepts: vec!["TypeAdapter".into(), "nullSafe".into()],
            identifiers: vec![
                "TypeAdapter".into(),
                "Point".into(),
                "registerTypeAdapter".into(),
                "PointAdapter".into(),
                "nullSafe".into(),
            ],
            file_hints: Vec::new(),
            client_keywords: Vec::new(),
            confidence: 0.94,
            raw_prompt: "I registered a custom TypeAdapter for my Point class using builder.registerTypeAdapter(Point.class, new PointAdapter().nullSafe()) exactly like the Gson javadoc example, but now every non-null Point field in my objects is being serialized as if it were null and dropped entirely from the JSON output. This started after I added .nullSafe(). Where does nullSafe() wrapping live and what could cause non-null values to be treated as null during serialization?".into(),
        }
    }

    #[test]
    fn null_safe_write_stays_an_exon() {
        let mut symbols = HashSet::new();
        symbols.insert("typeadapter".into());
        symbols.insert("nullsafe".into());
        let policy = FoldPolicy::from_task(&symbols, &gson_bug_signature());
        let body = "if (value != null) {\n    out.nullValue();\n} else {\n    TypeAdapter.this.write(out, value);\n}";
        assert!(
            policy.keep_open("write", Some("NullSafeTypeAdapter"), body),
            "NullSafeTypeAdapter.write must stay open for a nullSafe serialization bug"
        );
        assert!(
            policy.score(
                "write",
                Some("NullSafeTypeAdapter"),
                "public void write(JsonWriter out, T value)",
                body
            ) > 30.0
        );
    }

    #[test]
    fn unrelated_helper_still_folds() {
        let mut symbols = HashSet::new();
        symbols.insert("typeadapter".into());
        let policy = FoldPolicy::from_task(&symbols, &gson_bug_signature());
        let helper_body = "return new JsonArray(elements);";
        assert!(!policy.keep_open("deepCopy", Some("JsonArray"), helper_body));
    }

    #[test]
    fn fold_ids_differ_across_files() {
        let a = make_fold_id("gson/TypeAdapter.java", "write", 1, 300);
        let b = make_fold_id("gson/JsonWriter.java", "write", 1, 80);
        assert_ne!(a, b);
        assert!(a.starts_with("fold_write_1_"));
        assert!(b.starts_with("fold_write_1_"));
    }

    #[test]
    fn normalize_accepts_marker_and_bare_id() {
        assert_eq!(
            normalize_fold_query("fold_write_4_abc12"),
            "fold_write_4_abc12"
        );
        assert_eq!(
            normalize_fold_query(
                "/* [neuromesh:fold:fold_write_4_abc12 | 6 lines folded | @Override] */"
            ),
            "fold_write_4_abc12"
        );
    }

    #[test]
    fn serialize_prompt_implies_write_verb() {
        let verbs = infer_verb_exons("values are serialized as null");
        assert!(verbs.contains("write"));
        assert!(!verbs.contains("read"));
    }

    #[test]
    fn exon_budget_keeps_top_scores_only() {
        let policy = FoldPolicy::default().with_exon_budget(1);
        let picked = policy.select_exons(&[8.0, 40.0, 12.0]);
        assert_eq!(picked, HashSet::from([1]));
        let with_seed = FoldPolicy::default().with_exon_budget(1);
        let picked = with_seed.select_exons(&[40.0, 100.0, 90.0]);
        assert_eq!(picked, HashSet::from([1]), "K=1 still keeps the exact seed");
        let picked = FoldPolicy::default()
            .with_exon_budget(2)
            .select_exons(&[40.0, 100.0, 90.0]);
        assert!(picked.contains(&1) && picked.contains(&2));
        let only_seeds = FoldPolicy::default()
            .with_exon_budget(1)
            .select_exons(&[100.0, 100.0, 100.0]);
        assert_eq!(only_seeds, HashSet::from([0]), "K caps even exact seeds");
    }
}
