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

/// One span in a windowed skeleton: body kept (`is_exon`) or replaced by a fold marker.
type SpanEmit = (FunctionSpan, bool, f32);

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

fn is_c_like_path(file_path: &str) -> bool {
    file_path.ends_with(".ts")
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
        || file_path.ends_with(".rb")
}

fn is_preamble_line(line: &str) -> bool {
    let t = line.trim();
    t.is_empty()
        || t.starts_with("package ")
        || t.starts_with("import ")
        || t.starts_with("use ")
        || t.starts_with("pub use ")
        || t.starts_with("from ")
        || t.starts_with("#include")
        || t.starts_with("#!")
        || t.starts_with("#[")
        || t.starts_with("extern crate")
        || (t.starts_with("mod ") && t.ends_with(';'))
}

fn preamble_len(lines: &[&str]) -> usize {
    let mut last = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if is_preamble_line(line) {
            if !line.trim().is_empty() {
                last = i + 1;
            }
            continue;
        }
        break;
    }
    last
}

fn enclosing_header_line(lines: &[&str], owner: &str, before: usize) -> Option<usize> {
    let owner_l = owner.to_lowercase();
    let start = before.min(lines.len());
    let floor = start.saturating_sub(80);
    for i in (floor..start).rev() {
        let l = lines[i].to_lowercase();
        let is_type = l.contains("class ")
            || l.contains("struct ")
            || l.contains("interface ")
            || l.contains("enum ")
            || l.contains("impl ")
            || l.contains("object ")
            || l.contains("trait ");
        if is_type && l.contains(&owner_l) {
            return Some(i);
        }
    }
    None
}

fn header_indent(line: &str) -> String {
    line.chars().take_while(|c| c.is_whitespace()).collect()
}

fn detect_brace_spans(content: &str) -> Vec<FunctionSpan> {
    let lines: Vec<&str> = content.lines().collect();
    let fn_regex = Regex::new(
        r"^\s*(?:export\s+|pub\s+|async\s+|public\s+|private\s+|protected\s+|static\s+)*(?:fn|function|def)?\s*([a-zA-Z0-9_]+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)",
    )
    .unwrap();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(caps) = fn_regex.captures(line) {
            let sym_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
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
            if found_open {
                let header = line.trim_end();
                let signature = header
                    .strip_suffix('{')
                    .map(str::trim_end)
                    .unwrap_or(header);
                spans.push(FunctionSpan {
                    name: sym_name.to_string(),
                    start_line: body_start + 1,
                    end_line: body_end + 1,
                    signature: signature.to_string(),
                    owner: None,
                });
                i = body_end + 1;
                continue;
            }
        }
        i += 1;
    }
    spans
}

fn detect_python_spans(content: &str) -> Vec<FunctionSpan> {
    let lines: Vec<&str> = content.lines().collect();
    let py_fn_regex = Regex::new(r"^(\s*)(?:async\s+)?def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(caps) = py_fn_regex.captures(lines[i]) {
            let indent_len = caps.get(1).map(|m| m.as_str().len()).unwrap_or(0);
            let sym_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
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
            spans.push(FunctionSpan {
                name: sym_name.to_string(),
                start_line: i + 1,
                end_line: body_end + 1,
                signature: format!("def {}(...)", sym_name),
                owner: None,
            });
            i = body_end + 1;
            continue;
        }
        i += 1;
    }
    spans
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

        let detected = if file_path.ends_with(".py") {
            detect_python_spans(content)
        } else if is_c_like_path(file_path) {
            detect_brace_spans(content)
        } else {
            Vec::new()
        };
        if !detected.is_empty() {
            return Self::skeletonize_from_spans(
                file_path,
                content,
                policy,
                original_tokens,
                &detected,
                min_lines,
            );
        }

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

    fn skeletonize_from_spans(
        file_path: &str,
        content: &str,
        policy: &FoldPolicy,
        original_tokens: usize,
        spans: &[FunctionSpan],
        min_lines: usize,
    ) -> SkeletonResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut ordered: Vec<FunctionSpan> = spans.to_vec();
        ordered.sort_by_key(|s| (s.start_line, std::cmp::Reverse(s.end_line)));

        let scores: Vec<f32> = ordered
            .iter()
            .map(|span| {
                let body = span_body(&lines, span);
                policy.score(&span.name, span.owner.as_deref(), &span.signature, &body)
            })
            .collect();
        let exon_idx = policy.select_exons(&scores);
        let exon_spans: Vec<FunctionSpan> = ordered
            .iter()
            .enumerate()
            .filter(|(i, _)| exon_idx.contains(i))
            .map(|(_, s)| s.clone())
            .collect();

        let mut fold_plans: Vec<(usize, usize, FunctionSpan)> = Vec::new();
        let mut emit_spans: Vec<SpanEmit> = Vec::new();
        for (i, span) in ordered.iter().enumerate() {
            let is_exon = exon_idx.contains(&i);
            if !is_exon && exon_spans.iter().any(|exon| contains_span(span, exon)) {
                continue;
            }
            if is_exon {
                emit_spans.push((span.clone(), true, scores[i]));
                continue;
            }
            if let Some((interior_start, interior_end)) = interior_range(&lines, span, min_lines) {
                fold_plans.push((interior_start, interior_end, span.clone()));
                emit_spans.push((span.clone(), false, scores[i]));
            }
        }

        if !exon_spans.is_empty() {
            let kept_owners: HashSet<Option<String>> =
                exon_spans.iter().map(|s| s.owner.clone()).collect();
            emit_spans.retain(|(span, is_exon, _)| *is_exon || kept_owners.contains(&span.owner));
            fold_plans.retain(|(_, _, span)| kept_owners.contains(&span.owner));
        }

        if emit_spans.is_empty() {
            return SkeletonResult {
                skeleton_code: content.to_string(),
                original_tokens,
                skeleton_tokens: original_tokens,
                token_reduction_pct: 0.0,
                exons_count: exon_spans.len(),
                introns_folded: 0,
                folds: Vec::new(),
            };
        }

        let mut result_lines: Vec<String> = Vec::new();
        let mut folds: Vec<FoldedIntron> = Vec::new();
        for line in lines.iter().take(preamble_len(&lines)) {
            result_lines.push((*line).to_string());
        }

        let mut groups: Vec<(Option<String>, Vec<SpanEmit>)> = Vec::new();
        for item in emit_spans {
            let owner = item.0.owner.clone();
            if let Some(existing) = groups.iter_mut().find(|(o, _)| *o == owner) {
                existing.1.push(item);
            } else {
                groups.push((owner, vec![item]));
            }
        }
        groups.sort_by_key(|(_, items)| {
            items
                .iter()
                .map(|(s, _, _)| s.start_line)
                .min()
                .unwrap_or(0)
        });

        for (owner, mut items) in groups {
            items.sort_by_key(|(s, _, _)| s.start_line);
            let mut close_indent: Option<String> = None;
            if let Some(owner_name) = owner.as_deref() {
                let first_start = items
                    .first()
                    .map(|(s, _, _)| s.start_line.saturating_sub(1))
                    .unwrap_or(0);
                if let Some(header) = enclosing_header_line(&lines, owner_name, first_start) {
                    close_indent = Some(header_indent(lines[header]));
                    result_lines.push(lines[header].to_string());
                    if !lines[header].contains('{') {
                        if let Some(next) = lines.get(header + 1) {
                            if next.trim() == "{" {
                                result_lines.push((*next).to_string());
                            }
                        }
                    }
                }
            }
            for (span, is_exon, score) in items {
                let start = span.start_line.saturating_sub(1).min(lines.len());
                let end = span.end_line.min(lines.len()).saturating_sub(1).max(start);
                if is_exon {
                    for line in &lines[start..=end] {
                        result_lines.push((*line).to_string());
                    }
                    continue;
                }
                let Some((interior_start, interior_end, _)) = fold_plans
                    .iter()
                    .find(|(_, _, s)| s.start_line == span.start_line && s.name == span.name)
                else {
                    for line in &lines[start..=end] {
                        result_lines.push((*line).to_string());
                    }
                    continue;
                };
                let body_content = lines[*interior_start..=*interior_end].join("\n");
                let saved_tokens = TokenCounter::count_tokens(&body_content);
                let indent = lines[*interior_start]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>();
                let fold_id =
                    make_fold_id(file_path, &span.name, folds.len() + 1, *interior_start + 1);
                if start < *interior_start {
                    result_lines.push(lines[start].to_string());
                }
                result_lines.push(format!(
                    "{}/* [neuromesh:fold:{} | {} lines folded | {}] */",
                    indent,
                    fold_id,
                    interior_end.saturating_sub(*interior_start) + 1,
                    span.signature
                ));
                if end > *interior_end {
                    result_lines.push(lines[end].to_string());
                }
                folds.push(FoldedIntron {
                    fold_id,
                    symbol_name: span.name.clone(),
                    signature: span.signature.clone(),
                    original_body: body_content,
                    start_line: *interior_start + 1,
                    end_line: *interior_end + 1,
                    saved_tokens,
                    owner: span.owner.clone(),
                    task_score: score,
                });
            }
            if let Some(indent) = close_indent {
                result_lines.push(format!("{indent}}}"));
            }
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
            exons_count: exon_spans.len(),
            introns_folded: folds.len(),
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
        assert!(res.skeleton_tokens < res.original_tokens);
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

    #[test]
    fn windowed_skeleton_drops_unrelated_class_body() {
        let code = r#"package com.google.gson;
import java.io.IOException;

public class TypeAdapter<T> {
    public void unusedHelper() {
        int a = 1;
        int b = 2;
        int c = 3;
        int d = 4;
        int e = a + b + c + d;
    }
    private final class NullSafeTypeAdapter extends TypeAdapter<T> {
        public void write(JsonWriter out, T value) throws IOException {
            if (value != null) {
                out.nullValue();
            }
        }
        public T read(JsonReader in) throws IOException {
            int a = 1;
            int b = 2;
            int c = 3;
            return null;
        }
    }
}
"#;
        let mut active = HashSet::new();
        active.insert("write".into());
        let spans = vec![
            FunctionSpan {
                name: "unusedHelper".into(),
                start_line: 5,
                end_line: 11,
                signature: "public void unusedHelper()".into(),
                owner: Some("TypeAdapter".into()),
            },
            FunctionSpan {
                name: "write".into(),
                start_line: 13,
                end_line: 17,
                signature: "public void write(JsonWriter out, T value)".into(),
                owner: Some("NullSafeTypeAdapter".into()),
            },
            FunctionSpan {
                name: "read".into(),
                start_line: 18,
                end_line: 23,
                signature: "public T read(JsonReader in)".into(),
                owner: Some("NullSafeTypeAdapter".into()),
            },
        ];
        let res =
            CodeSkeletonizer::skeletonize_with_spans("TypeAdapter.java", code, &active, &spans);
        assert!(res.skeleton_code.contains("out.nullValue()"));
        assert!(res.skeleton_code.contains("import java.io.IOException"));
        assert!(
            !res.skeleton_code.contains("int e = a + b + c + d"),
            "unrelated class body must not ship: {}",
            res.skeleton_code
        );
        assert!(res.skeleton_tokens < res.original_tokens);
    }

    #[test]
    fn optional_budget_keeps_one_exon() {
        let code = r#"
export function keepMe() {
    const a = 1;
    const b = 2;
    return a + b;
}
export function otherOne() {
    const a = 1;
    const b = 2;
    const c = 3;
    return a + b + c;
}
export function otherTwo() {
    const a = 1;
    const b = 2;
    const c = 3;
    return a + b + c;
}
"#;
        let mut active = HashSet::new();
        active.insert("keepMe".into());
        let policy = FoldPolicy::from_symbols(&active).with_exon_budget(1);
        let res = CodeSkeletonizer::skeletonize_with_policy("x.ts", code, &policy, &[]);
        assert!(res.skeleton_code.contains("return a + b;"));
        assert!(res.introns_folded >= 2, "{}", res.skeleton_code);
        assert!(!res.folds.iter().any(|f| f.symbol_name == "keepMe"));
    }
}
