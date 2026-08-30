//! Server-side keyword/expansion inference for MCP assisted-by-default behavior.

use crate::retrieval::alias::{alias_code_seeds_for_prompt, canonical_concepts, expand_aliases};
use crate::retrieval::concept_expand::{expand_concept_to_code_seeds, identifier_variants};
use crate::retrieval::query_intent::{assisted_signals, classify_intent};
use neuromesh_core::TaskSignature;
use neuromesh_parser::extract_embedded_code_tokens;
use neuromesh_task::TaskSignatureExtractor;

const MAX_INFERRED_KEYWORDS: usize = 8;
const MAX_INFERRED_EXPANSION: usize = 8;

fn push_unique_ci(out: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    if !out.iter().any(|x| x.eq_ignore_ascii_case(trimmed)) {
        out.push(trimmed.to_string());
    }
}

fn merge_keywords(out: &mut Vec<String>, terms: impl IntoIterator<Item = impl AsRef<str>>) {
    for term in terms {
        push_unique_ci(out, term.as_ref());
        if out.len() >= MAX_INFERRED_KEYWORDS {
            out.truncate(MAX_INFERRED_KEYWORDS);
            return;
        }
    }
}

fn merge_expansion(out: &mut Vec<String>, terms: impl IntoIterator<Item = impl AsRef<str>>) {
    for term in terms {
        push_unique_ci(out, term.as_ref());
        if out.len() >= MAX_INFERRED_EXPANSION {
            out.truncate(MAX_INFERRED_EXPANSION);
            return;
        }
    }
}

/// Keep symbol-like embedded tokens; drop capitalized NL words (e.g. German "Wie").
fn is_code_like_token(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains('.') || t.contains('(') || t.contains('_') || t.contains('/') {
        return true;
    }
    if t.chars().all(|c| c.is_ascii_alphanumeric()) && t.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    if t.len() >= 2
        && t.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && t.chars().any(|c| c.is_ascii_uppercase())
    {
        return true;
    }
    matches!(t, "next" | "use" | "Router" | "app" | "stat")
}

fn filtered_embedded_tokens(prompt: &str) -> Vec<String> {
    extract_embedded_code_tokens(prompt)
        .into_iter()
        .filter(|t| is_code_like_token(t))
        .collect()
}

/// Infer English code keywords and related expansion from a natural-language prompt.
///
/// Pipeline: intent pack → alias code seeds → embedded symbols → alias concepts.
pub fn infer_assisted_seed_signals(prompt: &str) -> (Vec<String>, Vec<String>) {
    let mut keywords: Vec<String> = Vec::new();
    let mut expansion: Vec<String> = Vec::new();

    let signature = TaskSignatureExtractor::extract(prompt);
    let intent = classify_intent(&signature);

    // 1. Intent assisted packs (hard-gated: General / ExplainSymbol → empty)
    let (intent_kw, intent_exp) = assisted_signals(intent);
    merge_keywords(&mut keywords, intent_kw);
    merge_expansion(&mut expansion, intent_exp);

    // 2. Alias code seeds for every matched concept
    merge_keywords(&mut keywords, alias_code_seeds_for_prompt(prompt));

    // 3. Embedded code tokens (symbol-like only)
    merge_keywords(&mut keywords, filtered_embedded_tokens(prompt));

    // 4. Alias expansion concepts + ASCII terms + code identifier variants
    for term in expand_aliases(prompt) {
        let is_concept = canonical_concepts()
            .iter()
            .any(|c| term.eq_ignore_ascii_case(c));
        if is_concept {
            merge_expansion(&mut expansion, std::iter::once(term.as_str()));
            for seed in expand_concept_to_code_seeds(&term) {
                merge_keywords(&mut keywords, std::iter::once(seed.as_str()));
            }
        } else if term.is_ascii() && term.len() >= 3 {
            merge_keywords(&mut keywords, std::iter::once(term.as_str()));
            for variant in identifier_variants(&term) {
                merge_expansion(&mut expansion, std::iter::once(variant.as_str()));
            }
        }
    }

    keywords.truncate(MAX_INFERRED_KEYWORDS);
    expansion.truncate(MAX_INFERRED_EXPANSION);
    (keywords, expansion)
}

/// Fill missing `client_keywords` / `client_expansion` from server inference (FILL-ONLY-MISSING).
///
/// When the client supplied one side, only the empty side is populated.
pub fn apply_auto_extract_keywords(
    signature: &mut TaskSignature,
    prompt: &str,
    enabled: bool,
) -> bool {
    if !enabled {
        return false;
    }
    let had_keywords = !signature.client_keywords.is_empty();
    let had_expansion = !signature.client_expansion.is_empty();
    let (kw, ex) = infer_assisted_seed_signals(prompt);
    if signature.client_keywords.is_empty() {
        for k in kw {
            push_unique_ci(&mut signature.client_keywords, &k);
        }
    }
    if signature.client_expansion.is_empty() {
        for e in ex {
            push_unique_ci(&mut signature.client_expansion, &e);
        }
    }
    (!had_keywords && !signature.client_keywords.is_empty())
        || (!had_expansion && !signature.client_expansion.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::query_intent::QueryIntent;

    #[test]
    fn infer_middleware_has_two_gold_hits() {
        let (kw, exp) =
            infer_assisted_seed_signals("Explain the middleware pipeline and how next() works.");
        let gold = ["app.use", "next", "middleware"];
        let hits = gold
            .iter()
            .filter(|g| kw.iter().any(|k| k.eq_ignore_ascii_case(g)))
            .count();
        assert!(hits >= 2, "keywords={kw:?} expansion={exp:?}");
        assert!(!exp.is_empty());
    }

    #[test]
    fn infer_render_from_fa() {
        let (kw, exp) = infer_assisted_seed_signals(
            "تابع res.render() چطور با موتورهای قالب و ویوها کار می‌کند؟",
        );
        let gold = ["res.render", "view", "render", "engine"];
        let hits = gold
            .iter()
            .filter(|g| kw.iter().any(|k| k.eq_ignore_ascii_case(g)))
            .count();
        assert!(hits >= 2, "keywords={kw:?}");
        assert!(!exp.is_empty());
    }

    #[test]
    fn apply_respects_client_keywords_only_fills_expansion_side() {
        let prompt = "Explain the middleware pipeline and how next() works.".to_string();
        let mut sig = TaskSignatureExtractor::extract(&prompt);
        sig.client_keywords = vec!["app.use".into()];
        let before_kw = sig.client_keywords.clone();
        apply_auto_extract_keywords(&mut sig, &prompt, true);
        assert_eq!(sig.client_keywords, before_kw);
        assert!(!sig.client_expansion.is_empty());
    }

    #[test]
    fn apply_disabled_populates_nothing() {
        let prompt = "Explain middleware pipeline".to_string();
        let mut sig = TaskSignatureExtractor::extract(&prompt);
        apply_auto_extract_keywords(&mut sig, &prompt, false);
        assert!(sig.client_keywords.is_empty());
        assert!(sig.client_expansion.is_empty());
    }

    #[test]
    fn assisted_signals_general_is_empty() {
        let (kw, exp) = assisted_signals(QueryIntent::General);
        assert!(kw.is_empty());
        assert!(exp.is_empty());
    }

    #[test]
    fn infer_jwt_from_fa_prompt() {
        let (kw, exp) = infer_assisted_seed_signals("کد مربوط به اعتبارسنجی توکن jwt کجاست؟");
        let gold = ["validateToken", "verifyJwt", "JwtPayload", "jwt", "auth"];
        let hits = gold
            .iter()
            .filter(|g| {
                kw.iter().any(|k| k.eq_ignore_ascii_case(g))
                    || exp.iter().any(|e| e.eq_ignore_ascii_case(g))
            })
            .count();
        assert!(hits >= 2, "keywords={kw:?} expansion={exp:?}");
    }
}
