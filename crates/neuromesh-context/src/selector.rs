use neuromesh_core::{EdgeConfidence, EdgeType, NodeId, NodeType, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Extra tokens allowed *on top of* seed files. Seeds always ship.
pub fn fill_budget(mode: OptimizationMode) -> usize {
    match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 8_000,
        OptimizationMode::MaxQuality => 16_000,
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

#[derive(Debug, Clone)]
pub struct Selection {
    pub node_ids: Vec<NodeId>,
    pub required: Vec<NodeId>,
    pub optional: Vec<NodeId>,
    pub scores: HashMap<NodeId, f32>,
    pub budget_used: usize,
    pub budget_cap: usize,
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
    let _ = neighborhood;
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

    let hop_limit = match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 1,
        OptimizationMode::MaxQuality => 2,
    };
    let max_extra_files = match mode {
        OptimizationMode::MaxSavings => 0,
        OptimizationMode::Balanced => 8,
        OptimizationMode::MaxQuality => 14,
    };

    let mut file_scores: HashMap<NodeId, f32> = HashMap::new();
    let bump_file = |scores: &mut HashMap<NodeId, f32>, id: &NodeId, amount: f32| {
        if required.contains(id) || is_noise_node(graph, id) {
            return;
        }
        if graph
            .get_node(id)
            .is_some_and(|n| n.node_type == NodeType::File && n.token_cost > 10_000)
        {
            return;
        }
        *scores.entry(id.clone()).or_insert(0.0) += amount;
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
                    && (edge.edge_type == EdgeType::Calls || edge.edge_type == EdgeType::Imports)
                    && edge.target == *seed;
                if !outbound_call && !inbound_use {
                    continue;
                }
                if let Some(node) = graph.get_node(&neighbor) {
                    if outbound_call && is_common_call(&node.name) {
                        continue;
                    }
                    if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                        bump_file(
                            &mut file_scores,
                            &file_id,
                            if outbound_call { 12.0 } else { 10.0 },
                        );
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

    if hop_limit >= 2 {
        for id in graph.steiner_union(seeds) {
            if let Some(node) = graph.get_node(&id) {
                if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                    bump_file(&mut file_scores, &file_id, 1.5);
                }
            }
        }
    }

    let mut optional_files: Vec<(NodeId, f32)> = file_scores.into_iter().collect();
    optional_files.sort_by(|a, b| {
        let score = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
        if score != std::cmp::Ordering::Equal {
            return score;
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
    let mut per_crate: HashMap<String, usize> = HashMap::new();
    let mut limited = Vec::new();
    for (id, gain) in optional_files {
        if limited.len() >= max_extra_files {
            break;
        }
        let crate_key = graph
            .get_node(&id)
            .map(|n| crate_dir(&n.file_path))
            .unwrap_or_default();
        let count = per_crate.entry(crate_key).or_insert(0);
        if *count >= per_crate_limit {
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

pub fn is_noise_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    lower.ends_with(".md")
        || lower.ends_with(".txt")
        || lower.ends_with(".rst")
        || lower.contains("/docs/")
        || lower.contains("/changelog")
        || lower.ends_with("/license")
        || lower.contains("quality_tests")
        || lower.contains("repo_quality_tests")
        || lower.contains("/tests/")
        || lower.contains("_tests.rs")
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
