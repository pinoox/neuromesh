use crate::fold::{make_fold_id, FoldPolicy};
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
    #[serde(default)]
    pub owner: Option<String>,
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
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub task_score: f32,
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

fn contains_span(outer: &FunctionSpan, inner: &FunctionSpan) -> bool {
    inner.start_line > outer.start_line && inner.end_line <= outer.end_line
}

fn is_block_closer(line: &str) -> bool {
    matches!(line.trim(), "}" | "};" | "}," | ")" | ");" | "end" | "end;")
}

/// Fold the body, keep the signature (and a trailing `}` / `end`) as the file map.
fn interior_range(lines: &[&str], span: &FunctionSpan, min_lines: usize) -> Option<(usize, usize)> {
    let start = span.start_line.saturating_sub(1);
    let end = span.end_line.min(lines.len()).saturating_sub(1);
    if start >= lines.len() || end < start {
        return None;
    }
    let interior_start = start.saturating_add(1);
    let mut interior_end = end;
    if interior_end >= interior_start && is_block_closer(lines[interior_end]) {
        if interior_end == interior_start {
            return None;
        }
        interior_end -= 1;
    }
    if interior_start > interior_end {
        return None;
    }
    let interior_len = interior_end.saturating_sub(interior_start) + 1;
    if interior_len < min_lines {
        return None;
    }
    Some((interior_start, interior_end))
}

fn span_body(lines: &[&str], span: &FunctionSpan) -> String {
    let start = span.start_line.saturating_sub(1).min(lines.len());
    let end = span.end_line.min(lines.len()).saturating_sub(1);
    if start >= lines.len() || end < start {
        return String::new();
    }
    lines[start..=end].join("\n")
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
        Self::skeletonize_with_policy(
            file_path,
            content,
            &FoldPolicy::from_symbols(active_symbol_names),
            &[],
        )
    }

    /// Prefer parser/graph function spans when available (accurate bodies).
    pub fn skeletonize_with_spans(
        file_path: &str,
        content: &str,
        active_symbol_names: &HashSet<String>,
        spans: &[FunctionSpan],
    ) -> SkeletonResult {
        Self::skeletonize_with_policy(
            file_path,
            content,
            &FoldPolicy::from_symbols(active_symbol_names),
            spans,
        )
    }

    pub fn skeletonize_with_policy(
        file_path: &str,
        content: &str,
        policy: &FoldPolicy,
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
                file_path,
                content,
                policy,
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
            || file_path.ends_with(".php")
            || file_path.ends_with(".kt")
            || file_path.ends_with(".kts")
            || file_path.ends_with(".dart")
            || file_path.ends_with(".cs")
            || file_path.ends_with(".swift")
            || file_path.ends_with(".svelte")
            || file_path.ends_with(".rb");

        if is_python {
            Self::skeletonize_python(file_path, content, policy, original_tokens, min_lines)
        } else if is_c_like {
            Self::skeletonize_brace_language(file_path, content, policy, original_tokens, min_lines)
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
        file_path: &str,
        content: &str,
        policy: &FoldPolicy,
        original_tokens: usize,
        spans: &[FunctionSpan],
        min_lines: usize,
    ) -> SkeletonResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut exons_count = 0;
        let mut ordered: Vec<FunctionSpan> = spans.to_vec();
        ordered.sort_by_key(|s| (s.start_line, std::cmp::Reverse(s.end_line)));

        let exon_spans: Vec<FunctionSpan> = ordered
            .iter()
            .filter(|s| policy.keep_open(&s.name, s.owner.as_deref(), &span_body(&lines, s)))
            .cloned()
            .collect();

        let mut plans: Vec<(usize, usize, FunctionSpan)> = Vec::new();
        for span in ordered {
            let body = span_body(&lines, &span);
            if policy.keep_open(&span.name, span.owner.as_deref(), &body) {
                exons_count += 1;
                continue;
            }
            if exon_spans.iter().any(|exon| contains_span(&span, exon)) {
                exons_count += 1;
                continue;
            }
            let Some((interior_start, interior_end)) = interior_range(&lines, &span, min_lines)
            else {
                exons_count += 1;
                continue;
            };
            if plans
                .iter()
                .any(|(ps, pe, _)| *ps <= interior_start && interior_end <= *pe)
            {
                continue;
            }
            plans.push((interior_start, interior_end, span));
        }
        plans.sort_by_key(|(s, _, _)| *s);

        if plans.is_empty() {
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
        let mut plan_idx = 0usize;
        while i < lines.len() {
            if plan_idx < plans.len() && i == plans[plan_idx].0 {
                let (start, end, span) = &plans[plan_idx];
                let body_content = lines[*start..=*end].join("\n");
                let saved_tokens = TokenCounter::count_tokens(&body_content);
                let indent = lines[*start]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>();
                let fold_id = make_fold_id(file_path, &span.name, folds.len() + 1, *start + 1);
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
                    original_body: body_content.clone(),
                    start_line: *start + 1,
                    end_line: *end + 1,
                    saved_tokens,
                    owner: span.owner.clone(),
                    task_score: policy.score(
                        &span.name,
                        span.owner.as_deref(),
                        &span.signature,
                        &body_content,
                    ),
                });
                i = *end + 1;
                plan_idx += 1;
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
        file_path: &str,
        content: &str,
        policy: &FoldPolicy,
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

                let body_content = if found_open {
                    lines[body_start..=body_end].join("\n")
                } else {
                    String::new()
                };
                let is_active = policy.keep_open(sym_name, None, &body_content);

                let span = body_end - body_start + 1;
                if found_open && span >= min_lines && !is_active {
                    // Fold this intron
                    introns_folded += 1;
                    let fold_id = make_fold_id(file_path, sym_name, introns_folded, body_start + 1);
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
                        original_body: body_content.clone(),
                        start_line: body_start + 1,
                        end_line: body_end + 1,
                        saved_tokens,
                        owner: None,
                        task_score: policy.score(sym_name, None, clean_header, &body_content),
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
        file_path: &str,
        content: &str,
        policy: &FoldPolicy,
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

                let body_content = lines[i..=body_end].join("\n");
                let is_active = policy.keep_open(sym_name, None, &body_content);
                let span = body_end - i + 1;
                if span >= min_lines && !is_active {
                    introns_folded += 1;
                    let fold_id = make_fold_id(file_path, sym_name, introns_folded, i + 1);
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
                        original_body: body_content.clone(),
                        start_line: i + 1,
                        end_line: body_end + 1,
                        saved_tokens,
                        owner: None,
                        task_score: policy.score(
                            sym_name,
                            None,
                            &format!("def {}(...)", sym_name),
                            &body_content,
                        ),
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
                owner: None,
            },
            FunctionSpan {
                name: "drop_me".into(),
                start_line: 6,
                end_line: 11,
                signature: "fn drop_me()".into(),
                owner: None,
            },
        ];
        let res = CodeSkeletonizer::skeletonize_with_spans("x.rs", code, &active, &spans);
        assert!(res.skeleton_code.contains("fn keep()"));
        assert!(
            res.skeleton_code.contains("fn drop_me()"),
            "signature stays as the file map: {}",
            res.skeleton_code
        );
        assert!(res.skeleton_code.contains("neuromesh:fold:fold_drop_me"));
        assert!(!res.skeleton_code.contains("let z = 3"));
        assert_eq!(res.folds.len(), 1);
        assert!(
            res.folds[0].original_body.contains("let z = 3"),
            "folded interior must restore the body"
        );
        assert!(
            !res.folds[0].original_body.contains("fn drop_me()"),
            "signature is not part of the intron: {}",
            res.folds[0].original_body
        );
    }

    #[test]
    fn does_not_fold_parent_that_contains_an_exon() {
        let code = "fn outer() {\n    fn keep() {\n        let a = 1;\n        a\n    }\n    let noise = 1;\n    let more = 2;\n    noise + more\n}\n";
        let mut active = HashSet::new();
        active.insert("keep".into());
        let spans = vec![
            FunctionSpan {
                name: "outer".into(),
                start_line: 1,
                end_line: 9,
                signature: "fn outer()".into(),
                owner: None,
            },
            FunctionSpan {
                name: "keep".into(),
                start_line: 2,
                end_line: 5,
                signature: "fn keep()".into(),
                owner: None,
            },
        ];
        let res = CodeSkeletonizer::skeletonize_with_spans("x.rs", code, &active, &spans);
        assert!(
            !res.folds.iter().any(|f| f.symbol_name == "outer"),
            "folding outer would hide the keep exon: {:?}",
            res.skeleton_code
        );
        assert!(res.skeleton_code.contains("fn keep()"));
        assert!(res.skeleton_code.contains("let a = 1"));
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
