use neuromesh_core::{
    hmvc_app_prefix, EdgeConfidence, EdgeType, NodeId, NodeType, OptimizationMode,
};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Extra tokens allowed *on top of* seed files. Seeds always ship.
pub fn fill_budget(mode: OptimizationMode) -> usize {
    match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 5_000,
        OptimizationMode::MaxQuality => 16_000,
    }
}

/// Hard cap on the whole packet (seeds + fill) after skeletonization.
/// Seeds are never dropped; optionals go first, then seed exon budget shrinks.
pub fn packet_cap(mode: OptimizationMode) -> usize {
    match mode {
        OptimizationMode::MaxSavings => 6_000,
        OptimizationMode::Balanced => 12_000,
        OptimizationMode::MaxQuality => 24_000,
    }
}

pub fn token_budget(mode: OptimizationMode) -> usize {
    fill_budget(mode)
}

pub fn budget_mode_name(mode: OptimizationMode) -> &'static str {
    match mode {
        OptimizationMode::MaxSavings => "max_savings",
        OptimizationMode::Balanced => "balanced",
        OptimizationMode::MaxQuality => "max_quality",
    }
}

/// Function names the seed actually calls. Those stay exons so the packet
/// does not fold the body that answers the question.
pub fn seed_callee_exon_names(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for seed in seeds {
        let Some(seed_node) = graph.get_node(seed) else {
            continue;
        };
        if seed_node.node_type == NodeType::File {
            continue;
        }
        for (neighbor, edge) in graph.get_connected_neighbors(seed) {
            if edge.edge_type != EdgeType::Calls || edge.source != *seed {
                continue;
            }
            if edge.confidence == EdgeConfidence::Unresolved {
                continue;
            }
            let Some(node) = graph.get_node(&neighbor) else {
                continue;
            };
            if is_common_call(&node.name) {
                continue;
            }
            names.insert(node.name.to_lowercase());
        }
    }
    names
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub node_ids: Vec<NodeId>,
    pub required: Vec<NodeId>,
    pub optional: Vec<NodeId>,
    pub scores: HashMap<NodeId, f32>,
    pub budget_used: usize,
    pub budget_cap: usize,
    pub optional_cap: usize,
    pub method: &'static str,
}

/// Seed files/symbols are required. Connectors are ranked for the activator to
/// fill under a real post-skeleton token budget. Docs and fixtures stay out of
/// the optional list unless they themselves are seeds.
pub fn select(
    graph: &NeuralProjectGraph,
    neighborhood: &HashSet<NodeId>,
    seeds: &HashSet<NodeId>,
    seed_energies: &HashMap<NodeId, f32>,
    focus_terms: &HashSet<String>,
    mode: OptimizationMode,
) -> Selection {
    let fill_cap = fill_budget(mode);
    if seeds.is_empty() {
        return Selection {
            node_ids: Vec::new(),
            required: Vec::new(),
            optional: Vec::new(),
            scores: HashMap::new(),
            budget_used: 0,
            budget_cap: fill_cap,
            optional_cap: 0,
            method: "no_seed",
        };
    }
    let mut required: HashSet<NodeId> = HashSet::new();
    let mut scores: HashMap<NodeId, f32> = HashMap::new();

    for seed in seeds {
        if let Some(node) = graph.get_node(seed) {
            required.insert(node.id.clone());
            scores.insert(
                node.id.clone(),
                10.0 * seed_energies.get(seed).copied().unwrap_or(1.0),
            );
            if node.node_type != NodeType::File {
                if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                    required.insert(file_id.clone());
                    scores.entry(file_id).or_insert(8.5);
                }
            }
        }
    }

    const MAX_REQUIRED_CALLEE_FILES: usize = 3;
    let mut callee_candidates: Vec<(NodeId, String, bool, bool, String)> = Vec::new();
    for seed in seeds {
        let Some(seed_node) = graph.get_node(seed) else {
            continue;
        };
        if seed_node.node_type == NodeType::File {
            continue;
        }
        for (neighbor, edge) in graph.get_connected_neighbors(seed) {
            if edge.edge_type != EdgeType::Calls || edge.source != *seed {
                continue;
            }
            if edge.confidence == EdgeConfidence::Unresolved {
                continue;
            }
            let Some(node) = graph.get_node(&neighbor) else {
                continue;
            };
            if is_common_call(&node.name) {
                continue;
            }
            let Some(file_id) = graph.file_id_for_path(&node.file_path) else {
                continue;
            };
            if required.contains(&file_id) {
                continue;
            }
            if hmvc_apps_conflict(&seed_node.file_path, &node.file_path) {
                continue;
            }
            let stem_focus = focus_terms.iter().any(|t| file_stem_eq(&node.file_path, t));
            let focus = stem_focus || focus_terms.contains(&node.name.to_lowercase());
            callee_candidates.push((
                file_id,
                node.file_path.to_string_lossy().replace('\\', "/"),
                focus,
                stem_focus,
                node.name.to_lowercase(),
            ));
        }
    }
    callee_candidates.sort_by(|a, b| match (a.3, b.3) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.1.cmp(&b.1),
        },
    });
    let mut seen_callee_files: HashSet<NodeId> = HashSet::new();
    let mut added_callees = 0usize;
    for (file_id, _, _, stem_focus, name) in callee_candidates {
        if !seen_callee_files.insert(file_id.clone()) {
            continue;
        }
        if added_callees >= MAX_REQUIRED_CALLEE_FILES {
            break;
        }
        if !stem_focus
            && focus_terms
                .iter()
                .any(|t| name == *t && required_owns_term_stem(graph, &required, t))
        {
            continue;
        }
        required.insert(file_id.clone());
        scores
            .entry(file_id)
            .or_insert(if stem_focus { 18.0 } else { 16.0 });
        added_callees += 1;
    }

    let hop_limit = match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 1,
        OptimizationMode::MaxQuality => 2,
    };
    let max_extra_files: usize = match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 5,
        OptimizationMode::MaxQuality => 8,
    };
    let max_extra_files = max_extra_files.saturating_sub(added_callees);

    let mut file_scores: HashMap<NodeId, f32> = HashMap::new();
    let mut callee_files: HashSet<NodeId> = HashSet::new();
    let learning_boost = |id: &NodeId| -> f32 {
        graph.get_node(id).map_or(0.0, |n| {
            let access = (n.access_count as f32).ln_1p() * 2.0;
            let relevance = (n.base_relevance - 1.0).max(0.0) * 4.0;
            access + relevance
        })
    };
    let bump_file = |scores: &mut HashMap<NodeId, f32>, id: &NodeId, amount: f32| {
        if required.contains(id) || is_noise_node(graph, id) {
            return;
        }
        *scores.entry(id.clone()).or_insert(0.0) += amount + learning_boost(id);
    };
    let bump_file_max = |scores: &mut HashMap<NodeId, f32>, id: &NodeId, amount: f32| {
        if required.contains(id) || is_noise_node(graph, id) {
            return;
        }
        let total = amount + learning_boost(id);
        let entry = scores.entry(id.clone()).or_insert(0.0);
        if total > *entry {
            *entry = total;
        }
    };

    for term in focus_terms {
        if term.len() < 4 {
            continue;
        }
        if let Some((id, _)) = graph.resolve_ranked(term, None, None) {
            if let Some(node) = graph.get_node(&id) {
                if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                    bump_file(&mut file_scores, &file_id, 15.0);
                }
            }
        }
    }

    for seed in seeds {
        let Some(seed_node) = graph.get_node(seed) else {
            continue;
        };
        if seed_node.node_type != NodeType::File {
            for (neighbor, edge) in graph.get_connected_neighbors(seed) {
                if edge.confidence == EdgeConfidence::Unresolved {
                    continue;
                }
                let outbound_call = edge.edge_type == EdgeType::Calls && edge.source == *seed;
                let inbound_use = hop_limit > 0
                    && matches!(
                        edge.edge_type,
                        EdgeType::Calls | EdgeType::Imports | EdgeType::References
                    )
                    && edge.target == *seed;
                if !outbound_call && !inbound_use {
                    continue;
                }
                if let Some(node) = graph.get_node(&neighbor) {
                    if outbound_call && is_common_call(&node.name) {
                        continue;
                    }
                    if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                        let mut amount = if outbound_call { 12.0 } else { 10.0 };
                        if outbound_call && edge.confidence != EdgeConfidence::Unresolved {
                            amount += 3.0;
                        }
                        if focus_terms.contains(&node.name.to_lowercase()) {
                            amount += 8.0;
                        }
                        if outbound_call {
                            callee_files.insert(file_id.clone());
                            bump_file_max(&mut file_scores, &file_id, amount);
                        } else {
                            bump_file(&mut file_scores, &file_id, amount);
                        }
                    }
                    scores.entry(neighbor.clone()).or_insert(9.0);
                }
            }
        }
        if hop_limit == 0 {
            continue;
        }
        if let Some(file_id) = graph.file_id_for_path(&seed_node.file_path) {
            for (neighbor, edge) in graph.get_connected_neighbors(&file_id) {
                if edge.edge_type != EdgeType::Imports || edge.source != file_id {
                    continue;
                }
                if edge.confidence == EdgeConfidence::Unresolved {
                    continue;
                }
                if let Some(node) = graph.get_node(&neighbor) {
                    if is_common_import(&node.name) {
                        continue;
                    }
                    if let Some(imported_file) = graph.file_id_for_path(&node.file_path) {
                        bump_file(&mut file_scores, &imported_file, 8.0);
                    }
                }
            }
        }
    }

    const SYNAPTIC_FILL_MIN: f32 = 0.58;
    for seed in seeds {
        let Some(seed_node) = graph.get_node(seed) else {
            continue;
        };
        let mut endpoints = vec![seed.clone()];
        if let Some(file_id) = graph.file_id_for_path(&seed_node.file_path) {
            endpoints.push(file_id);
        }
        for endpoint in endpoints {
            for (neighbor, edge) in graph.get_connected_neighbors(&endpoint) {
                if edge.pheromone_weight < SYNAPTIC_FILL_MIN {
                    continue;
                }
                if let Some(node) = graph.get_node(&neighbor) {
                    if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                        bump_file_max(&mut file_scores, &file_id, 9.0 * edge.pheromone_weight);
                    }
                }
            }
        }
    }

    for u in graph.unresolved_refs() {
        let from_seed = seeds.iter().any(|s| {
            graph
                .get_node(s)
                .is_some_and(|n| n.file_path == u.from_file)
        });
        if !from_seed {
            continue;
        }
        if let Some((id, _)) = graph.resolve_ranked(&u.name, None, None) {
            if let Some(node) = graph.get_node(&id) {
                if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                    bump_file(&mut file_scores, &file_id, 11.0);
                }
            }
        }
    }

    if hop_limit >= 2 {
        for id in graph.steiner_union(seeds) {
            if let Some(node) = graph.get_node(&id) {
                if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                    bump_file(&mut file_scores, &file_id, 1.5);
                }
            }
        }
    }

    for id in neighborhood {
        if let Some(node) = graph.get_node(id) {
            let file_id = if node.node_type == NodeType::File {
                Some(id.clone())
            } else {
                graph.file_id_for_path(&node.file_path)
            };
            if let Some(file_id) = file_id {
                if file_scores.contains_key(&file_id) {
                    bump_file(&mut file_scores, &file_id, 1.0);
                }
            }
        }
    }

    let mut optional_files: Vec<(NodeId, f32)> = file_scores
        .into_iter()
        .map(|(id, gain)| (id, gain.min(24.0)))
        .collect();
    if let Some(lock) = locked_hmvc_prefix(graph, &required) {
        optional_files.retain(|(id, _)| {
            graph
                .get_node(id)
                .is_some_and(|node| match hmvc_app_prefix(&node.file_path) {
                    Some(prefix) => prefix == lock,
                    None => true,
                })
        });
    }
    let mut owned_stems: HashSet<String> = HashSet::new();
    for id in required
        .iter()
        .chain(optional_files.iter().map(|(id, _)| id))
    {
        if let Some(node) = graph.get_node(id) {
            if let Some(stem) = node.file_path.file_stem().and_then(|s| s.to_str()) {
                let stem_l = stem.to_lowercase();
                if focus_terms.contains(&stem_l) {
                    owned_stems.insert(stem_l);
                }
            }
        }
    }
    optional_files.retain(|(id, _)| {
        let Some(node) = graph.get_node(id) else {
            return false;
        };
        !owned_stems.iter().any(|t| {
            !file_stem_eq(&node.file_path, t)
                && graph.search_symbols(t, 12).iter().any(|hit| {
                    hit.name.eq_ignore_ascii_case(t)
                        && graph.file_id_for_path(&hit.file_path).as_ref() == Some(id)
                })
        })
    });
    for term in focus_terms {
        if term.len() < 4 {
            continue;
        }
        if let Some((id, _)) = graph.resolve_ranked(term, None, None) {
            if let Some(node) = graph.get_node(&id) {
                if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                    if required.contains(&file_id) {
                        continue;
                    }
                    if required_owns_term_stem(graph, &required, term)
                        && !file_stem_eq(&node.file_path, term)
                    {
                        continue;
                    }
                    if let Some(entry) = optional_files.iter_mut().find(|(fid, _)| *fid == file_id)
                    {
                        if entry.1 < 36.0 {
                            entry.1 = 36.0;
                        }
                    } else if !is_noise_node(graph, &file_id) {
                        optional_files.push((file_id, 36.0));
                    }
                }
            }
        }
    }
    optional_files.sort_by(|a, b| {
        let score = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
        if score != std::cmp::Ordering::Equal {
            return score;
        }
        match (callee_files.contains(&a.0), callee_files.contains(&b.0)) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        let pa = graph
            .get_node(&a.0)
            .map(|n| n.file_path.to_string_lossy().to_string())
            .unwrap_or_default();
        let pb = graph
            .get_node(&b.0)
            .map(|n| n.file_path.to_string_lossy().to_string())
            .unwrap_or_default();
        pa.cmp(&pb)
    });

    let per_crate_limit = match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 2,
        OptimizationMode::MaxQuality => 3,
    };
    let overflow_limit = match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 4,
        OptimizationMode::MaxQuality => 6,
    };
    let mut per_crate: HashMap<String, usize> = HashMap::new();
    let mut limited = Vec::new();
    let mut overflow: Vec<(NodeId, f32)> = Vec::new();
    for (id, gain) in optional_files {
        if limited.len() >= max_extra_files {
            break;
        }
        if gain < 15.0 && graph.get_node(&id).is_some_and(|n| n.token_cost > 10_000) {
            continue;
        }
        let crate_key = graph
            .get_node(&id)
            .map(|n| crate_dir(&n.file_path))
            .unwrap_or_default();
        let count = per_crate.entry(crate_key.clone()).or_insert(0);
        if *count >= per_crate_limit && !callee_files.contains(&id) {
            overflow.push((id, gain));
            continue;
        }
        *count += 1;
        limited.push((id, gain));
    }
    for (id, gain) in overflow {
        if limited.len() >= max_extra_files {
            break;
        }
        if gain < 12.0 {
            continue;
        }
        let crate_key = graph
            .get_node(&id)
            .map(|n| crate_dir(&n.file_path))
            .unwrap_or_default();
        let count = per_crate.entry(crate_key).or_insert(0);
        if *count >= overflow_limit {
            continue;
        }
        *count += 1;
        limited.push((id, gain));
    }
    optional_files = limited;

    let mut optional_ids = Vec::new();
    for (id, gain) in optional_files {
        scores.entry(id.clone()).or_insert(gain);
        optional_ids.push(id);
    }

    let mut required_ids: Vec<NodeId> = required.iter().cloned().collect();
    required_ids.sort_by(|a, b| {
        let sa = scores.get(a).copied().unwrap_or(0.0);
        let sb = scores.get(b).copied().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut node_ids = required_ids.clone();
    node_ids.extend(optional_ids.iter().cloned());

    Selection {
        node_ids,
        required: required_ids,
        optional: optional_ids,
        scores,
        budget_used: 0,
        budget_cap: fill_cap,
        optional_cap: max_extra_files,
        method: "seed_then_fill",
    }
}

fn is_noise_node(graph: &NeuralProjectGraph, id: &NodeId) -> bool {
    graph
        .get_node(id)
        .is_some_and(|n| is_noise_path(&n.file_path))
}

fn crate_dir(path: &Path) -> String {
    let parts: Vec<String> = path
        .iter()
        .map(|s| s.to_string_lossy().replace('\\', "/"))
        .collect();
    if let Some(idx) = parts
        .iter()
        .position(|p| p == "crates" || p == "packages" || p == "apps")
    {
        if let Some(name) = parts.get(idx + 1) {
            if !name.is_empty() && name != "src" {
                return name.clone();
            }
        }
    }
    if let Some(src_idx) = parts.iter().rposition(|p| p == "src") {
        if src_idx > 0 {
            let parent = &parts[src_idx - 1];
            if !parent.is_empty() {
                return parent.clone();
            }
        }
    }
    path.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "pkg".into())
}

fn locked_hmvc_prefix(graph: &NeuralProjectGraph, required: &HashSet<NodeId>) -> Option<String> {
    let mut prefixes = HashSet::new();
    for id in required {
        if let Some(prefix) = graph
            .get_node(id)
            .and_then(|node| hmvc_app_prefix(&node.file_path))
        {
            prefixes.insert(prefix);
        }
    }
    if prefixes.len() == 1 {
        prefixes.into_iter().next()
    } else {
        None
    }
}

fn hmvc_apps_conflict(seed: &Path, other: &Path) -> bool {
    match (hmvc_app_prefix(seed), hmvc_app_prefix(other)) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    }
}

fn file_stem_eq(path: &Path, query: &str) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(query))
}

fn required_owns_term_stem(
    graph: &NeuralProjectGraph,
    required: &HashSet<NodeId>,
    term: &str,
) -> bool {
    required.iter().any(|id| {
        graph
            .get_node(id)
            .is_some_and(|n| file_stem_eq(&n.file_path, term))
    })
}

pub fn is_noise_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    lower.ends_with(".md")
        || lower.ends_with(".txt")
        || lower.ends_with(".rst")
        || lower.contains("/docs/")
        || lower.contains("/changelog")
        || lower.ends_with("/license")
        || lower.contains("/editors/")
        || lower.starts_with("editors/")
        || neuromesh_core::is_low_priority_source_path(path)
}

pub fn is_common_call(name: &str) -> bool {
    matches!(
        name,
        "as_str"
            | "as_u64"
            | "as_array"
            | "as_ref"
            | "as_mut"
            | "and_then"
            | "or_else"
            | "unwrap_or"
            | "unwrap_or_else"
            | "unwrap_or_default"
            | "unwrap"
            | "expect"
            | "clone"
            | "cloned"
            | "copied"
            | "to_string"
            | "to_lowercase"
            | "to_owned"
            | "into_owned"
            | "into_iter"
            | "into"
            | "from"
            | "insert"
            | "push"
            | "get"
            | "len"
            | "is_empty"
            | "is_some"
            | "is_none"
            | "take"
            | "next"
            | "collect"
            | "iter"
            | "filter"
            | "map"
            | "any"
            | "all"
            | "find"
            | "default"
            | "format"
            | "json"
            | "ok"
            | "err"
            | "now"
            | "elapsed"
            | "as_millis"
            | "chars"
            | "replace"
            | "split"
            | "trim"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "parse"
            | "from_str"
            | "record_global_telemetry"
            | "max"
            | "min"
            | "saturating_sub"
            | "saturating_add"
    )
}

fn is_common_import(name: &str) -> bool {
    matches!(
        name,
        "Result"
            | "Option"
            | "Error"
            | "NodeId"
            | "HashSet"
            | "HashMap"
            | "Arc"
            | "Vec"
            | "String"
            | "Value"
            | "Instant"
            | "PathBuf"
            | "Path"
            | "OptimizationMode"
            | "Config"
            | "Duration"
            | "Utc"
            | "NeuralProjectGraph"
            | "CodeIntelligenceEngine"
            | "IndexedFile"
            | "SourceLanguage"
            | "ProjectId"
            | "TokenCounter"
    )
}

/// Baseline used only in tests: first N file nodes by path order.
pub fn first_n_files(graph: &NeuralProjectGraph, n: usize) -> HashSet<String> {
    let mut files: Vec<String> = graph
        .get_all_nodes()
        .into_iter()
        .filter(|node| node.node_type == NodeType::File)
        .map(|node| node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();
    files.into_iter().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::PathBuf;

    fn indexed(rel: &str, tokens: usize) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("gold"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: rel.to_string(),
            byte_size: tokens as u64,
            token_count: tokens,
            language: SourceLanguage::Rust,
            last_modified: chrono::Utc::now(),
        }
    }

    #[test]
    fn steiner_greedy_beats_first_five_files() {
        let graph = NeuralProjectGraph::new(ProjectId::new("gold"));
        let filler = "pub fn unused() { let a = 1; let b = 2; let c = 3; }\n";
        for name in ["aaa.rs", "bbb.rs", "ccc.rs", "ddd.rs", "eee.rs"] {
            graph.ingest_file(
                &indexed(name, 80),
                &CodeIntelligenceEngine::analyze(
                    &PathBuf::from(name),
                    filler,
                    SourceLanguage::Rust,
                ),
                Some(filler),
            );
        }
        let seed_src = r#"
pub fn start() {
    helper();
}
"#;
        let helper_src = r#"
pub fn helper() {
    target();
}
"#;
        let target_src = r#"
pub fn target() {
    let value = 1;
    let other = 2;
    value + other
}
"#;
        graph.ingest_file(
            &indexed("seed.rs", 40),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("seed.rs"),
                seed_src,
                SourceLanguage::Rust,
            ),
            Some(seed_src),
        );
        graph.ingest_file(
            &indexed("helper.rs", 40),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("helper.rs"),
                helper_src,
                SourceLanguage::Rust,
            ),
            Some(helper_src),
        );
        graph.ingest_file(
            &indexed("target.rs", 40),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("target.rs"),
                target_src,
                SourceLanguage::Rust,
            ),
            Some(target_src),
        );
        graph.finalize_links();

        let start = graph
            .resolve_unique("start", Some("seed.rs"))
            .expect("start");
        let mut seeds = HashSet::new();
        seeds.insert(start.clone());
        let neighborhood = graph.neighborhood(&seeds, 3);
        let mut energies = HashMap::new();
        energies.insert(start, 1.0);
        let selected = select(
            &graph,
            &neighborhood,
            &seeds,
            &energies,
            &HashSet::new(),
            OptimizationMode::Balanced,
        );

        let selected_files: HashSet<String> = selected
            .node_ids
            .iter()
            .filter_map(|id| graph.get_node(id))
            .filter(|n| n.node_type == NodeType::File)
            .map(|n| {
                n.file_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        let gold = HashSet::from([
            "seed.rs".to_string(),
            "helper.rs".to_string(),
            "target.rs".to_string(),
        ]);
        let steiner_hits = gold.intersection(&selected_files).count();
        let baseline = first_n_files(&graph, 5);
        let baseline_names: HashSet<String> = baseline
            .iter()
            .filter_map(|p| p.rsplit('/').next().map(|s| s.to_string()))
            .collect();
        let baseline_hits = gold.intersection(&baseline_names).count();

        assert!(
            steiner_hits > baseline_hits,
            "seed+connectors recall {steiner_hits} should beat first-5 {baseline_hits}; selected={selected_files:?}"
        );
        assert_eq!(selected.method, "seed_then_fill");
        assert!(selected.required.iter().any(|id| {
            graph
                .get_node(id)
                .is_some_and(|n| n.file_path.ends_with("seed.rs"))
        }));
    }

    #[test]
    fn max_savings_does_not_queue_connectors() {
        let graph = NeuralProjectGraph::new(ProjectId::new("gold"));
        let seed_src = "pub fn start() { helper(); }\n";
        let helper_src = "pub fn helper() {}\n";
        graph.ingest_file(
            &indexed("seed.rs", 20),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("seed.rs"),
                seed_src,
                SourceLanguage::Rust,
            ),
            Some(seed_src),
        );
        graph.ingest_file(
            &indexed("helper.rs", 20),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("helper.rs"),
                helper_src,
                SourceLanguage::Rust,
            ),
            Some(helper_src),
        );
        graph.finalize_links();
        let start = graph
            .resolve_unique("start", Some("seed.rs"))
            .expect("start");
        let mut seeds = HashSet::new();
        seeds.insert(start.clone());
        let neighborhood = graph.neighborhood(&seeds, 2);
        let mut energies = HashMap::new();
        energies.insert(start, 1.0);
        let selected = select(
            &graph,
            &neighborhood,
            &seeds,
            &energies,
            &HashSet::new(),
            OptimizationMode::MaxSavings,
        );
        assert!(selected.optional.is_empty());
        assert_eq!(selected.budget_cap, 0);
    }

    #[test]
    fn packet_cap_is_separate_from_fill_budget() {
        assert_eq!(fill_budget(OptimizationMode::MaxSavings), 0);
        assert_eq!(fill_budget(OptimizationMode::Balanced), 5_000);
        assert_eq!(fill_budget(OptimizationMode::MaxQuality), 16_000);
        assert_eq!(packet_cap(OptimizationMode::MaxSavings), 6_000);
        assert_eq!(packet_cap(OptimizationMode::Balanced), 12_000);
        assert_eq!(packet_cap(OptimizationMode::MaxQuality), 24_000);
    }

    #[test]
    fn crate_dir_reads_workspace_crate_name() {
        assert_eq!(
            crate_dir(Path::new("crates/neuromesh-context/src/activator.rs")),
            "neuromesh-context"
        );
        let nested = PathBuf::from("crates")
            .join("neuromesh-task")
            .join("src")
            .join("signature.rs");
        assert_eq!(crate_dir(&nested), "neuromesh-task");
        assert_eq!(crate_dir(Path::new("seed.rs")), "pkg");
    }
}
