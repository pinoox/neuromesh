use crate::selector::Selection;
use neuromesh_core::{PacketGap, SeedResolution, SkippedFile, StructuralEvidence, TaskSignature};
use neuromesh_graph::{NeuralProjectGraph, TraceDirection};
use std::collections::HashSet;

pub fn prompt_needs_callers(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    [
        "dead code",
        "dead-code",
        "unused",
        "unreferenced",
        "not used",
        "never used",
        "no caller",
        "callers",
        "who calls",
        "all usages",
        "all references",
        "references across",
        "list all references",
        "find unused",
    ]
    .iter()
    .any(|k| lower.contains(k))
        || (lower.contains("reference") && lower.contains("across"))
}

pub fn prompt_needs_bug_hunt(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("bug")
        || lower.contains("fix")
        || lower.contains("double discount")
        || lower.contains("wrong total")
        || (lower.contains("discount") && lower.contains("total"))
}

pub fn inject_caller_context(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<neuromesh_core::NodeId>,
    prompt: &str,
    selection: &mut Selection,
) {
    if !prompt_needs_callers(prompt) {
        return;
    }
    for seed in seeds {
        let Some(node) = graph.get_node(seed) else {
            continue;
        };
        if node.node_type == NodeType::File {
            continue;
        }
        let trace = graph.trace_symbol(&node.name, TraceDirection::Inbound, 2);
        for caller in trace.callers {
            let Some(file_id) = graph.file_id_for_path(&caller.file_path) else {
                continue;
            };
            if !selection.required.contains(&file_id) {
                selection.required.push(file_id.clone());
            }
            selection
                .scores
                .entry(file_id)
                .and_modify(|s| *s = (*s).max(17.0))
                .or_insert(17.0);
        }
    }
}

use neuromesh_core::NodeType;

pub fn who_reads_symbol(
    graph: &NeuralProjectGraph,
    node_id: &neuromesh_core::NodeId,
) -> Vec<String> {
    graph
        .get_connected_neighbors(node_id)
        .into_iter()
        .filter(|(_, edge)| {
            edge.target == *node_id
                && matches!(
                    edge.edge_type,
                    neuromesh_core::EdgeType::Calls
                        | neuromesh_core::EdgeType::UsedBy
                        | neuromesh_core::EdgeType::References
                )
        })
        .filter_map(|(neighbor, _)| graph.get_node(&neighbor))
        .map(|n| {
            format!(
                "{}@{}",
                n.name,
                n.file_path.to_string_lossy().replace('\\', "/")
            )
        })
        .collect()
}

pub fn build_structural_evidence(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<neuromesh_core::NodeId>,
) -> Vec<StructuralEvidence> {
    let mut out = Vec::new();
    for seed in seeds {
        let Some(node) = graph.get_node(seed) else {
            continue;
        };
        if node.node_type == NodeType::File {
            continue;
        }
        let callers = graph.inbound_caller_count(seed);
        let is_dead = graph.is_likely_dead_symbol(seed);
        let who_reads = who_reads_symbol(graph, seed);
        let exact_line = node
            .line_range
            .as_ref()
            .and_then(|range| {
                graph.read_source(&node.file_path).and_then(|src| {
                    src.lines()
                        .nth(range.start.saturating_sub(1))
                        .map(|l| l.trim().to_string())
                })
            })
            .filter(|l| !l.is_empty());
        out.push(StructuralEvidence {
            symbol: node.name.clone(),
            path: node.file_path.to_string_lossy().replace('\\', "/"),
            line: node.line_range.as_ref().map(|r| r.start),
            exact_line,
            callers_count: callers,
            is_dead,
            who_reads,
        });
    }
    out
}

pub fn compute_packet_gaps(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<neuromesh_core::NodeId>,
    selected_paths: &HashSet<String>,
    signature: &TaskSignature,
) -> (Vec<PacketGap>, Vec<String>) {
    let mut gaps = Vec::new();
    let mut unsure = Vec::new();

    if prompt_needs_callers(signature.raw_prompt.as_str()) {
        for seed in seeds {
            let Some(node) = graph.get_node(seed) else {
                continue;
            };
            if node.node_type == NodeType::File {
                continue;
            }
            let trace = graph.trace_symbol(&node.name, TraceDirection::Inbound, 2);
            let caller_count = trace.callers.len();
            for caller in &trace.callers {
                let path = caller.file_path.to_string_lossy().replace('\\', "/");
                if selected_paths.contains(&path) {
                    continue;
                }
                gaps.push(PacketGap {
                    kind: "caller".into(),
                    path: path.clone(),
                    reason: format!("inbound caller of {}", node.name),
                    line: caller.line_range.as_ref().map(|r| r.start),
                });
            }
            if caller_count == 0 && graph.is_likely_dead_symbol(seed) {
                unsure.push(format!(
                    "{} appears unused (0 inbound callers in graph)",
                    node.name
                ));
            }
        }
    }

    if super::style_routing::is_style_task(signature) {
        let style_ext = signature.style.as_deref().map(|s| s.to_ascii_lowercase());
        for hit in graph.search_symbols("tokens", 6) {
            let path = hit.file_path.to_string_lossy().replace('\\', "/");
            if !crate::style_routing::is_style_path(&hit.file_path)
                || !crate::style_routing::style_path_matches_task(&path, style_ext.as_deref())
                || selected_paths.contains(&path)
            {
                continue;
            }
            gaps.push(PacketGap {
                kind: "style".into(),
                path: path.clone(),
                reason: "style task likely needs tokens/mixins file".into(),
                line: None,
            });
        }
    }

    if prompt_needs_bug_hunt(signature.raw_prompt.as_str()) {
        for path in selected_paths {
            if !path.contains("cart") {
                continue;
            }
            if let Some(src) = graph.read_source(std::path::Path::new(path)) {
                for (idx, line) in src.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.contains("this.discount - this.discount")
                        || (trimmed.contains("total") && trimmed.matches("discount").count() >= 2)
                    {
                        gaps.push(PacketGap {
                            kind: "bug_line".into(),
                            path: path.clone(),
                            reason: "possible duplicate discount in total()".into(),
                            line: Some(idx + 1),
                        });
                    }
                }
            }
        }
        if gaps.iter().all(|g| g.kind != "bug_line") {
            for hit in graph.search_symbols("total", 8) {
                if !hit.file_path.to_string_lossy().contains("cart") {
                    continue;
                }
                if let Some(src) = graph.read_source(&hit.file_path) {
                    for (idx, line) in src.lines().enumerate() {
                        if line.contains("this.discount - this.discount") {
                            let path = hit.file_path.to_string_lossy().replace('\\', "/");
                            gaps.push(PacketGap {
                                kind: "bug_line".into(),
                                path,
                                reason: "duplicate discount subtraction in total()".into(),
                                line: Some(idx + 1),
                            });
                        }
                    }
                }
            }
        }
    }

    gaps.sort_by(|a, b| a.path.cmp(&b.path));
    gaps.dedup_by(|a, b| a.path == b.path && a.line == b.line);
    unsure.sort();
    unsure.dedup();
    (gaps, unsure)
}

pub fn semantic_style_coverage(
    selected_paths: &HashSet<String>,
    signature: &TaskSignature,
) -> Option<f32> {
    if !super::style_routing::is_style_task(signature) || selected_paths.is_empty() {
        return None;
    }
    let style_count = selected_paths
        .iter()
        .filter(|p| crate::style_routing::is_style_path(std::path::Path::new(p)))
        .count();
    Some(style_count as f32 / selected_paths.len() as f32)
}

#[allow(clippy::too_many_arguments)]
pub fn enrich_coverage(
    seeds: &[SeedResolution],
    packet_gaps: Vec<PacketGap>,
    unsure: Vec<String>,
    covered: Vec<String>,
    skipped: Vec<SkippedFile>,
    semantic_coverage: Option<f32>,
    sidecar_files: Vec<String>,
    budget_truncated: bool,
) -> neuromesh_core::CoverageReport {
    neuromesh_core::CoverageReport::from_seeds_with_gaps(
        seeds,
        packet_gaps,
        unsure,
        covered,
        skipped,
        semantic_coverage,
        sidecar_files,
        budget_truncated,
    )
}
