use crate::selector::Selection;
use neuromesh_core::{PacketGap, SeedResolution, StructuralEvidence, TaskSignature};
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
        out.push(StructuralEvidence {
            symbol: node.name.clone(),
            path: node.file_path.to_string_lossy().replace('\\', "/"),
            line: node.line_range.as_ref().map(|r| r.start),
            callers_count: callers,
            is_dead,
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
                || !style_path_matches_task(&path, style_ext.as_deref())
                || selected_paths.contains(&path)
            {
                continue;
            }
            gaps.push(PacketGap {
                kind: "style".into(),
                path: path.clone(),
                reason: "style task likely needs tokens/mixins file".into(),
            });
        }
    }

    gaps.sort_by(|a, b| a.path.cmp(&b.path));
    gaps.dedup_by(|a, b| a.path == b.path);
    unsure.sort();
    unsure.dedup();
    (gaps, unsure)
}

pub fn enrich_coverage(
    seeds: &[SeedResolution],
    packet_gaps: Vec<PacketGap>,
    unsure: Vec<String>,
) -> neuromesh_core::CoverageReport {
    neuromesh_core::CoverageReport::from_seeds_with_gaps(seeds, packet_gaps, unsure)
}

fn style_path_matches_task(path: &str, style: Option<&str>) -> bool {
    let p = path.to_ascii_lowercase();
    match style {
        Some("scss") | Some("sass") => p.ends_with(".scss") || p.ends_with(".sass"),
        Some("less") => p.ends_with(".less"),
        Some("css") => p.ends_with(".css"),
        _ => true,
    }
}
