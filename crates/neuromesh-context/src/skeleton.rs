use crate::genetic_optimizer::ContextChromosome;
use neuromesh_core::TokenCounter;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSpan {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
}

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
pub fn fold_intron_min_lines() -> usize {
    ContextChromosome::default().fold_threshold_lines.max(2)
}
pub struct CodeSkeletonizer;

impl CodeSkeletonizer {
    /// Skeletons code by preserving targeted active symbols (exons) and folding untargeted method bodies (introns)
    pub fn skeletonize(
        file_path: &str,
        content: &str,
        active_symbol_names: &HashSet<String>,
    ) -> SkeletonResult {
        Self::skeletonize_with_spans(file_path, content, active_symbol_names, &[])
    }

    /// Prefer parser/graph function spans when available (accurate bodies).
    pub fn skeletonize_with_spans(
        file_path: &str,
        content: &str,
        active_symbol_names: &HashSet<String>,
        spans: &[FunctionSpan],
    ) -> SkeletonResult {
        let original_tokens = TokenCounter::count_tokens(content);
        let line_count = content.lines().count();
        let min_lines = fold_intron_min_lines();

        let tiny = line_count < 4 || original_tokens < 20;
        if tiny && spans.len() < 2 {
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

        if !spans.is_empty() {
            return Self::skeletonize_from_spans(
                content,
                active_symbol_names,
                original_tokens,
                spans,
                min_lines,
            );
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
            Self::skeletonize_python(content, active_symbol_names, original_tokens, min_lines)
        } else if is_c_like {
            Self::skeletonize_brace_language(
                content,
                active_symbol_names,
                original_tokens,
                min_lines,
            )
        } else {
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

    fn skeletonize_from_spans(
        content: &str,
        active_symbols: &HashSet<String>,
        original_tokens: usize,
        spans: &[FunctionSpan],
        min_lines: usize,
    ) -> SkeletonResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut fold_ranges: Vec<(usize, usize, FunctionSpan)> = Vec::new();
        let mut exons_count = 0;
        let mut ordered: Vec<FunctionSpan> = spans.to_vec();
        ordered.sort_by_key(|s| s.start_line);

        for span in ordered {
            let start = span.start_line.saturating_sub(1);
            let end = span.end_line.min(lines.len()).saturating_sub(1);
            if start >= lines.len() || end < start {
                continue;
            }
            let span_len = end.saturating_sub(start) + 1;
            if is_seed_exon(&span.name, active_symbols) {
                exons_count += 1;
                continue;
            }
            if span_len < min_lines {
                exons_count += 1;
                continue;
            }
            fold_ranges.push((start, end, span));
        }

        if fold_ranges.is_empty() {
            return SkeletonResult {
                skeleton_code: content.to_string(),
                original_tokens,
                skeleton_tokens: original_tokens,
                token_reduction_pct: 0.0,
                exons_count,
                introns_folded: 0,
                folds: Vec::new(),
            };
        }

        let mut result_lines: Vec<String> = Vec::new();
        let mut folds: Vec<FoldedIntron> = Vec::new();
        let mut i = 0usize;
        let mut fold_idx = 0usize;
        while i < lines.len() {
            if fold_idx < fold_ranges.len() && i == fold_ranges[fold_idx].0 {
                let (start, end, span) = &fold_ranges[fold_idx];
                let body_content = lines[*start..=*end].join("\n");
                let saved_tokens = TokenCounter::count_tokens(&body_content);
                let indent = lines[*start]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>();
                let fold_id = format!("fold_{}_{}", span.name, folds.len() + 1);
                result_lines.push(format!(
                    "{}/* [neuromesh:fold:{} | {} lines folded | {}] */",
                    indent,
                    fold_id,
                    end.saturating_sub(*start) + 1,
                    span.signature
                ));
                folds.push(FoldedIntron {
                    fold_id,
                    symbol_name: span.name.clone(),
                    signature: span.signature.clone(),
                    original_body: body_content,
                    start_line: span.start_line,
                    end_line: span.end_line,
                    saved_tokens,
                });
                i = *end + 1;
                fold_idx += 1;
                continue;
            }
            result_lines.push(lines[i].to_string());
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
            introns_folded: folds.len(),
            folds,
        }
    }

    fn skeletonize_brace_language(
        content: &str,
        active_symbols: &HashSet<String>,
        original_tokens: usize,
        min_lines: usize,
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
                if found_open && span >= min_lines && !is_active {
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
        min_lines: usize,
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
                if span >= min_lines && !is_active {
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

    #[test]
    fn skeletonize_from_spans_folds_non_exons() {
        let code = "fn keep() {\n    let a = 1;\n    let b = 2;\n    a + b\n}\nfn drop_me() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n    x + y + z\n}\n";
        let mut active = HashSet::new();
        active.insert("keep".into());
        let spans = vec![
            FunctionSpan {
                name: "keep".into(),
                start_line: 1,
                end_line: 5,
                signature: "fn keep()".into(),
            },
            FunctionSpan {
                name: "drop_me".into(),
                start_line: 6,
                end_line: 11,
                signature: "fn drop_me()".into(),
            },
        ];
        let res = CodeSkeletonizer::skeletonize_with_spans("x.rs", code, &active, &spans);
        assert!(res.skeleton_code.contains("fn keep()"));
        assert!(res.skeleton_code.contains("neuromesh:fold:fold_drop_me"));
        assert!(!res.skeleton_code.contains("let z = 3"));
        assert_eq!(res.folds.len(), 1);
        assert_eq!(res.folds[0].original_body.lines().count(), 6);
    }

    #[test]
    fn small_multi_function_file_still_skeletonizes() {
        let code = "fn keep() {\n    1\n}\nfn drop_me() {\n    let x = 1;\n    x\n}\n";
        let mut active = HashSet::new();
        active.insert("keep".into());
        let res = CodeSkeletonizer::skeletonize("tiny.rs", code, &active);
        assert!(
            res.introns_folded >= 1,
            "multi-fn files must fold even when short: {:?}",
            res.skeleton_code
        );
        assert!(res.skeleton_code.contains("keep"));
        assert!(res.skeleton_code.contains("neuromesh:fold"));
    }
}
