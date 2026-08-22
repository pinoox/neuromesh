use neuromesh_core::TokenCounter;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldedIntron {
    pub fold_id: String,
    pub symbol_name: String,
    pub signature: String,
    pub original_body: String,
    pub start_line: usize,
    pub end_line: usize,
    pub saved_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonResult {
    pub skeleton_code: String,
    pub original_tokens: usize,
    pub skeleton_tokens: usize,
    pub token_reduction_pct: f32,
    pub exons_count: usize,
    pub introns_folded: usize,
    pub folds: Vec<FoldedIntron>,
}

fn is_seed_exon(sym_name: &str, active_symbols: &HashSet<String>) -> bool {
    let lower = sym_name.to_lowercase();
    active_symbols.contains(sym_name)
        || active_symbols.contains(&lower)
        || active_symbols
            .iter()
            .any(|s| s.eq_ignore_ascii_case(sym_name) || s.rsplit("::").next() == Some(sym_name))
}

/// Seed functions stay open (exons). Sibling functions fold. Import lines are kept as-is.
pub struct CodeSkeletonizer;

impl CodeSkeletonizer {
    /// Skeletons code by preserving targeted active symbols (exons) and folding untargeted method bodies (introns)
    pub fn skeletonize(
        file_path: &str,
        content: &str,
        active_symbol_names: &HashSet<String>,
    ) -> SkeletonResult {
        let original_tokens = TokenCounter::count_tokens(content);

        // For very tiny snippets (< 8 lines or < 35 tokens), keep full content
        if content.lines().count() < 8 || original_tokens < 35 {
            return SkeletonResult {
                skeleton_code: content.to_string(),
                original_tokens,
                skeleton_tokens: original_tokens,
                token_reduction_pct: 0.0,
                exons_count: 1,
                introns_folded: 0,
                folds: Vec::new(),
            };
        }

        let is_python = file_path.ends_with(".py");
        let is_c_like = file_path.ends_with(".ts")
            || file_path.ends_with(".tsx")
            || file_path.ends_with(".js")
            || file_path.ends_with(".jsx")
            || file_path.ends_with(".vue")
            || file_path.ends_with(".rs")
            || file_path.ends_with(".go")
            || file_path.ends_with(".java")
            || file_path.ends_with(".cpp")
            || file_path.ends_with(".c")
            || file_path.ends_with(".php");

        if is_python {
            Self::skeletonize_python(content, active_symbol_names, original_tokens)
        } else if is_c_like {
            Self::skeletonize_brace_language(content, active_symbol_names, original_tokens)
        } else {
            // Default fallback
            SkeletonResult {
                skeleton_code: content.to_string(),
                original_tokens,
                skeleton_tokens: original_tokens,
                token_reduction_pct: 0.0,
                exons_count: 1,
                introns_folded: 0,
                folds: Vec::new(),
            }
        }
    }

    fn skeletonize_brace_language(
        content: &str,
        active_symbols: &HashSet<String>,
        original_tokens: usize,
    ) -> SkeletonResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut result_lines: Vec<String> = Vec::new();
        let mut folds: Vec<FoldedIntron> = Vec::new();
        let mut exons_count = 0;
        let mut introns_folded = 0;

        let fn_regex = Regex::new(r"^\s*(?:export\s+|pub\s+|async\s+|public\s+|private\s+|protected\s+|static\s+)*(?:fn|function|def)?\s*([a-zA-Z0-9_]+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)").unwrap();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            if let Some(caps) = fn_regex.captures(line) {
                let sym_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let is_active = is_seed_exon(sym_name, active_symbols);

                // Find brace body
                let mut brace_count = 0;
                let mut found_open = false;
                let body_start = i;
                let mut body_end = i;

                for (j, l) in lines.iter().enumerate().skip(i) {
                    for ch in l.chars() {
                        if ch == '{' {
                            brace_count += 1;
                            found_open = true;
                        } else if ch == '}' {
                            brace_count -= 1;
                        }
                    }

                    if found_open && brace_count == 0 {
                        body_end = j;
                        break;
                    }
                }

                let span = body_end - body_start + 1;
                if found_open && span > 3 && !is_active {
                    // Fold this intron
                    introns_folded += 1;
                    let fold_id = format!("fold_{}_{}", sym_name, introns_folded);
                    let body_content = lines[body_start..=body_end].join("\n");
                    let saved_tokens = TokenCounter::count_tokens(&body_content);

                    let indent = line
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    let header = line.trim_end();

                    // If header contains '{', strip trailing '{'
                    let clean_header = if let Some(stripped) = header.strip_suffix('{') {
                        stripped.trim_end()
                    } else {
                        header
                    };

                    result_lines.push(format!(
                        "{}/* [neuromesh:fold:{} | {} lines folded | {}] */",
                        indent, fold_id, span, clean_header
                    ));

                    folds.push(FoldedIntron {
                        fold_id,
                        symbol_name: sym_name.to_string(),
                        signature: clean_header.to_string(),
                        original_body: body_content,
                        start_line: body_start + 1,
                        end_line: body_end + 1,
                        saved_tokens,
                    });

                    i = body_end + 1;
                    continue;
                } else {
                    exons_count += 1;
                }
            }

            result_lines.push(line.to_string());
            i += 1;
        }

        let skeleton_code = result_lines.join("\n");
        let skeleton_tokens = TokenCounter::count_tokens(&skeleton_code);
        let saved = original_tokens.saturating_sub(skeleton_tokens);
        let token_reduction_pct = if original_tokens > 0 {
            (saved as f32 / original_tokens as f32) * 100.0
        } else {
            0.0
        };

        SkeletonResult {
            skeleton_code,
            original_tokens,
            skeleton_tokens,
            token_reduction_pct,
            exons_count,
            introns_folded,
            folds,
        }
    }

    fn skeletonize_python(
        content: &str,
        active_symbols: &HashSet<String>,
        original_tokens: usize,
    ) -> SkeletonResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut result_lines: Vec<String> = Vec::new();
        let mut folds: Vec<FoldedIntron> = Vec::new();
        let mut exons_count = 0;
        let mut introns_folded = 0;

        let py_fn_regex = Regex::new(r"^(\s*)(?:async\s+)?def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            if let Some(caps) = py_fn_regex.captures(line) {
                let indent_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let indent_len = indent_str.len();
                let sym_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                let is_active = is_seed_exon(sym_name, active_symbols);

                // Find end of indentation block
                let mut body_end = i;
                for (j, l) in lines.iter().enumerate().skip(i + 1) {
                    if l.trim().is_empty() {
                        continue;
                    }
                    let l_indent = l.chars().take_while(|c| c.is_whitespace()).count();
                    if l_indent <= indent_len {
                        body_end = j - 1;
                        break;
                    }
                    body_end = j;
                }

                let span = body_end - i + 1;
                if span > 3 && !is_active {
                    introns_folded += 1;
                    let fold_id = format!("fold_{}_{}", sym_name, introns_folded);
                    let body_content = lines[i..=body_end].join("\n");
                    let saved_tokens = TokenCounter::count_tokens(&body_content);

                    result_lines.push(format!(
                        "{}# [neuromesh:fold:{} | {} lines folded | def {}(...)]",
                        indent_str, fold_id, span, sym_name
                    ));
                    result_lines.push(format!("{}pass", indent_str.repeat(2).max("    ".into())));

                    folds.push(FoldedIntron {
                        fold_id,
                        symbol_name: sym_name.to_string(),
                        signature: format!("def {}(...)", sym_name),
                        original_body: body_content,
                        start_line: i + 1,
                        end_line: body_end + 1,
                        saved_tokens,
                    });

                    i = body_end + 1;
                    continue;
                } else {
                    exons_count += 1;
                }
            }

            result_lines.push(line.to_string());
            i += 1;
        }

        let skeleton_code = result_lines.join("\n");
        let skeleton_tokens = TokenCounter::count_tokens(&skeleton_code);
        let saved = original_tokens.saturating_sub(skeleton_tokens);
        let token_reduction_pct = if original_tokens > 0 {
            (saved as f32 / original_tokens as f32) * 100.0
        } else {
            0.0
        };

        SkeletonResult {
            skeleton_code,
            original_tokens,
            skeleton_tokens,
            token_reduction_pct,
            exons_count,
            introns_folded,
            folds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeletonize_typescript_folds_introns() {
        let code = r#"
import { ref } from 'vue';

export function activeTarget() {
    const x = 10;
    return x * 2;
}

export function untargetedHeavyHelper1() {
    const a = 1;
    const b = 2;
    const c = 3;
    const d = 4;
    return a + b + c + d;
}

export function untargetedHeavyHelper2() {
    console.log("doing heavy calculation 1");
    console.log("doing heavy calculation 2");
    console.log("doing heavy calculation 3");
    console.log("doing heavy calculation 4");
    return true;
}
"#;

        let mut active = HashSet::new();
        active.insert("activeTarget".to_string());

        let res = CodeSkeletonizer::skeletonize("test.ts", code, &active);

        assert!(res.introns_folded >= 2);
        assert!(res.skeleton_code.contains("activeTarget"));
        assert!(res.skeleton_code.contains("return x * 2;"));
        assert!(res
            .skeleton_code
            .contains("neuromesh:fold:fold_untargetedHeavyHelper"));
        assert!(res.token_reduction_pct > 30.0);
    }
}
