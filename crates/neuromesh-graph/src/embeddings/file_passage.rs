//! Composite file-level passages for hierarchical tier-0 indexing.
//!
//! Preserves full module docstrings and complete function signatures (no mid-line
//! truncation). A line budget caps how many symbols are included, not signature width.

use crate::NeuralProjectGraph;
use neuromesh_core::{ContextNode, EdgeType, NodeType};
use neuromesh_embed::{format_document_for_model, EmbeddingModelId};
use std::path::Path;

/// Max symbol lines in the composite body (signatures + type names).
pub const MAX_SYMBOL_LINES: usize = 16;
/// Max chars for module/file docstring (kept intact up to this limit).
pub const MAX_DOC_CHARS: usize = 480;
/// Max chars per signature line (whole-line truncate only at whitespace when possible).
pub const MAX_SIGNATURE_LINE_CHARS: usize = 220;

#[derive(Debug, Clone)]
struct SymbolLine {
    score: f32,
    order: usize,
    text: String,
}

/// Build a MiniLM document passage for a file node.
pub fn file_passage(
    graph: &NeuralProjectGraph,
    file_node: &ContextNode,
    model: EmbeddingModelId,
) -> Option<String> {
    if file_node.node_type != NodeType::File {
        return None;
    }
    let path = file_node.file_path.to_string_lossy().replace('\\', "/");
    let stem = file_node
        .file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let title = if stem.is_empty() {
        path.clone()
    } else {
        format!("{path} [{stem}]")
    };

    let doc = file_node
        .doc_summary
        .as_deref()
        .or(file_node.signature.as_deref())
        .unwrap_or("")
        .trim();
    let doc_part = truncate_doc_preserving_words(doc, MAX_DOC_CHARS);

    let mut lines = select_symbol_lines(graph, &file_node.file_path, &stem);
    lines.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.order.cmp(&b.order))
    });
    lines.truncate(MAX_SYMBOL_LINES);

    let mut body = String::new();
    if !stem.is_empty() {
        body.push_str(&stem);
    }
    if !doc_part.is_empty() {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&doc_part);
    }
    for entry in &lines {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&entry.text);
    }

    Some(format_document_for_model(
        model, &title, "file", &body, None,
    ))
}

fn truncate_doc_preserving_words(doc: &str, max_chars: usize) -> String {
    if doc.len() <= max_chars {
        return doc.to_string();
    }
    let mut end = max_chars;
    while end > 0 && !doc.is_char_boundary(end) {
        end -= 1;
    }
    if let Some(sp) = doc[..end].rfind(' ') {
        doc[..sp].trim().to_string()
    } else {
        doc[..end].trim().to_string()
    }
}

fn format_signature_line(node: &ContextNode) -> String {
    let kind = match node.node_type {
        NodeType::Function => "fn",
        NodeType::Class => "class",
        NodeType::Component => "component",
        NodeType::Api => "api",
        NodeType::Config => "config",
        NodeType::Test => "test",
        NodeType::DbModel => "model",
        _ => "sym",
    };
    let sig = node.signature.as_deref().unwrap_or("").trim();
    let line = if sig.is_empty() {
        format!("{kind} {}", node.name)
    } else {
        format!("{kind} {} {sig}", node.name)
    };
    truncate_signature_line(&line)
}

fn truncate_signature_line(line: &str) -> String {
    if line.len() <= MAX_SIGNATURE_LINE_CHARS {
        return line.to_string();
    }
    let mut end = MAX_SIGNATURE_LINE_CHARS;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    if let Some(sp) = line[..end].rfind(' ') {
        format!("{}…", line[..sp].trim_end())
    } else {
        format!("{}…", line[..end].trim_end())
    }
}

fn symbol_score(graph: &NeuralProjectGraph, node: &ContextNode, file_stem: &str) -> f32 {
    let mut score = match node.node_type {
        NodeType::Function | NodeType::Class | NodeType::Component => 10.0,
        NodeType::Api => 9.0,
        NodeType::DbModel | NodeType::Config => 7.0,
        NodeType::Test => 2.0,
        _ => 4.0,
    };
    if !file_stem.is_empty() {
        let name_l = node.name.to_lowercase();
        let stem_l = file_stem.to_lowercase();
        if name_l == stem_l {
            score += 8.0;
        } else if stem_l.contains(&name_l) || name_l.contains(&stem_l) {
            score += 4.0;
        }
    }
    if node
        .doc_summary
        .as_deref()
        .is_some_and(|d| !d.trim().is_empty())
    {
        score += 2.0;
    }
    if node
        .signature
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        score += 1.5;
    }
    let mut callers = 0u32;
    for (_, edge) in graph.get_connected_neighbors(&node.id) {
        if edge.edge_type == EdgeType::Calls && edge.target == node.id {
            callers += 1;
        }
    }
    score += (callers.min(8) as f32) * 0.5;
    score
}

fn is_embeddable_symbol(node: &ContextNode) -> bool {
    matches!(
        node.node_type,
        NodeType::Function
            | NodeType::Class
            | NodeType::Component
            | NodeType::Api
            | NodeType::DbModel
            | NodeType::Config
            | NodeType::Symbol
            | NodeType::Test
    )
}

fn select_symbol_lines(
    graph: &NeuralProjectGraph,
    file_path: &Path,
    file_stem: &str,
) -> Vec<SymbolLine> {
    graph
        .nodes_in_file(file_path)
        .into_iter()
        .enumerate()
        .filter(|(_, n)| is_embeddable_symbol(n))
        .map(|(order, node)| SymbolLine {
            score: symbol_score(graph, &node, file_stem),
            order,
            text: format_signature_line(&node),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_not_truncated_when_short() {
        let doc = "Module-level auth middleware and JWT helpers.";
        assert_eq!(truncate_doc_preserving_words(doc, MAX_DOC_CHARS), doc);
    }

    #[test]
    fn signature_truncates_at_word_boundary() {
        let long = "a".repeat(MAX_SIGNATURE_LINE_CHARS + 40);
        let line = truncate_signature_line(&format!("fn foo {long}"));
        assert!(line.len() <= MAX_SIGNATURE_LINE_CHARS + 4);
        assert!(line.ends_with('…'));
    }
}
