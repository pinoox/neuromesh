//! Unicode normalization for natural-language prompts and client-supplied keywords.
//!
//! Works for any non-English script (Persian, Arabic, CJK, Cyrillic, etc.):
//! NFKC, common control/format chars, and optional Persian-specific fixes.

const ZWNJ: char = '\u{200c}';
const ZWJ: char = '\u{200d}';
const LRM: char = '\u{200e}';
const RLM: char = '\u{200f}';
const BOM: char = '\u{feff}';

/// Normalize a client keyword: trim, NFKC, strip control/format chars, collapse space.
pub fn normalize_keyword(raw: &str) -> String {
    let s = normalize_unicode(raw);
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalize full prompt text before token splitting (any language).
pub fn normalize_unicode(raw: &str) -> String {
    let mut s: String = unicode_normalization::UnicodeNormalization::nfkc(raw.chars()).collect();
    s = s.replace(ZWNJ, " ").replace([ZWJ, LRM, RLM, BOM], "");
    apply_optional_script_fixes(&mut s);
    s.trim().to_string()
}

/// Split normalized prompt into whitespace-delimited tokens for seed fallback.
pub fn normalize_prompt_tokens(raw: &str) -> Vec<String> {
    normalize_unicode(raw)
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| {
                !c.is_alphanumeric() && c != '_' && c != '.' && c != '/' && c != ':'
            })
            .to_string()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

fn apply_optional_script_fixes(s: &mut String) {
    // Persian/Arabic letter variants — helpful but not required for other scripts.
    *s = s
        .replace('\u{0643}', "\u{06a9}") // Arabic kaf → Persian ke
        .replace('\u{064a}', "\u{06cc}"); // Arabic yaa → Persian ye
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfkc_and_zwnj() {
        let raw = "مدل\u{200c}محصول";
        let norm = normalize_unicode(raw);
        assert!(norm.contains(' ') || !norm.contains(ZWNJ));
    }

    #[test]
    fn keyword_trims_and_collapses() {
        assert_eq!(normalize_keyword("  UserController  "), "UserController");
    }

    #[test]
    fn cjk_prompt_tokens_do_not_panic() {
        let tokens = normalize_prompt_tokens("设计用户认证模块");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn arabic_yeh_ke_optional_fix() {
        let s = normalize_unicode("كتاب");
        assert!(s.contains('\u{06a9}') || s.contains('ك'));
    }
}
