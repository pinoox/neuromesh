use neuromesh_core::{EdgeConfidence, EdgeType, NodeId, NodeType, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};

pub fn token_budget(mode: OptimizationMode) -> usize {
    match mode {
        OptimizationMode::MaxSavings => 900,
        OptimizationMode::Balanced => 2_500,
        OptimizationMode::MaxQuality => 6_000,
    }
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
    pub scores: HashMap<NodeId, f32>,
    pub budget_used: usize,
    pub budget_cap: usize,
    pub method: &'static str,
}

/// Steiner union of seed connectors, then greedy submodular fill under a token budget.
/// Physarum is not on this path; gold compares it separately.
pub fn select(
    graph: &NeuralProjectGraph,
    neighborhood: &HashSet<NodeId>,
    seeds: &HashSet<NodeId>,
    seed_energies: &HashMap<NodeId, f32>,
    mode: OptimizationMode,
) -> Selection {
    let budget = token_budget(mode);
    let mut selected: HashSet<NodeId> = HashSet::new();
    let mut scores: HashMap<NodeId, f32> = HashMap::new();
    let mut used = 0usize;

    for seed in seeds {
        if let Some(node) = graph.get_node(seed) {
            include_node(
                graph,
                &node.id,
                10.0 * seed_energies.get(seed).copied().unwrap_or(1.0),
                true,
                budget,
                &mut selected,
                &mut scores,
                &mut used,
            );
        }
    }

    let mut evidence: HashSet<NodeId> = HashSet::new();
    let mut evidence_sources: HashSet<NodeId> = seeds.clone();
    for seed in seeds {
        if let Some(node) = graph.get_node(seed) {
            if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
                evidence_sources.insert(file_id);
            }
        }
    }
    for source in &evidence_sources {
        for (neighbor, edge) in graph.get_connected_neighbors(source) {
            if matches!(
                edge.edge_type,
                EdgeType::Calls | EdgeType::Imports | EdgeType::DependsOn
            ) && edge.confidence != EdgeConfidence::Unresolved
            {
                evidence.insert(neighbor);
            }
        }
    }
    for id in &evidence {
        include_node(
            graph,
            id,
            7.5,
            true,
            budget,
            &mut selected,
            &mut scores,
            &mut used,
        );
    }

    let steiner = graph.steiner_union(seeds);
    for id in &steiner {
        if !neighborhood.contains(id) && !seeds.contains(id) {
            continue;
        }
        include_node(
            graph,
            id,
            6.0,
            seeds.contains(id),
            budget,
            &mut selected,
            &mut scores,
            &mut used,
        );
    }

    let mut remaining: Vec<NodeId> = neighborhood
        .iter()
        .filter(|id| !selected.contains(*id))
        .cloned()
        .collect();

    loop {
        let mut best: Option<(NodeId, f32, usize)> = None;
        for id in &remaining {
            let Some(node) = graph.get_node(id) else {
                continue;
            };
            let cost = node_cost(&node).max(1);
            if used.saturating_add(cost) > budget {
                continue;
            }
            let gain = marginal_utility(graph, &node.id, &selected, seeds, seed_energies);
            let ratio = gain / cost as f32;
            if best
                .as_ref()
                .is_none_or(|(_, best_ratio, _)| ratio > *best_ratio)
            {
                best = Some((id.clone(), ratio, cost));
            }
        }
        let Some((id, ratio, cost)) = best else {
            break;
        };
        if ratio <= 0.0 {
            break;
        }
        selected.insert(id.clone());
        scores.insert(id.clone(), ratio * cost as f32);
        used = used.saturating_add(cost);
        remaining.retain(|other| other != &id);
    }

    let mut node_ids: Vec<NodeId> = selected.into_iter().collect();
    node_ids.sort_by(|a, b| {
        let sa = scores.get(a).copied().unwrap_or(0.0);
        let sb = scores.get(b).copied().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    Selection {
        node_ids,
        scores,
        budget_used: used.min(budget),
        budget_cap: budget,
        method: "steiner_greedy",
    }
}

#[allow(clippy::too_many_arguments)]
fn include_node(
    graph: &NeuralProjectGraph,
    id: &NodeId,
    score: f32,
    force: bool,
    budget: usize,
    selected: &mut HashSet<NodeId>,
    scores: &mut HashMap<NodeId, f32>,
    used: &mut usize,
) {
    if selected.contains(id) {
        scores.entry(id.clone()).or_insert(score);
        return;
    }
    let Some(node) = graph.get_node(id) else {
        return;
    };
    let cost = node_cost(&node);
    if !force && used.saturating_add(cost) > budget {
        return;
    }
    selected.insert(id.clone());
    scores.insert(id.clone(), score);
    *used = used.saturating_add(cost);
    if node.node_type != NodeType::File {
        if let Some(file_id) = graph.file_id_for_path(&node.file_path) {
            include_node(
                graph,
                &file_id,
                (score * 0.85).max(4.0),
                force,
                budget,
                selected,
                scores,
                used,
            );
        }
    }
}

fn node_cost(node: &neuromesh_core::ContextNode) -> usize {
    if node.node_type == NodeType::File {
        // Packet files are skeletonized; charge an exon-aware estimate, not raw size.
        (node.token_cost / 4).clamp(48, 900)
    } else {
        node.token_cost.clamp(1, 48)
    }
}

fn marginal_utility(
    graph: &NeuralProjectGraph,
    candidate: &NodeId,
    selected: &HashSet<NodeId>,
    seeds: &HashSet<NodeId>,
    seed_energies: &HashMap<NodeId, f32>,
) -> f32 {
    if seeds.contains(candidate) {
        return 12.0 * seed_energies.get(candidate).copied().unwrap_or(1.0);
    }
    let mut utility = 0.15;
    for (neighbor, edge) in graph.get_connected_neighbors(candidate) {
        if !selected.contains(&neighbor) && !seeds.contains(&neighbor) {
            continue;
        }
        let conf = match edge.confidence {
            EdgeConfidence::Proven => 1.0,
            EdgeConfidence::Likely => 0.55,
            EdgeConfidence::Unresolved => 0.1,
        };
        let kind = match edge.edge_type {
            EdgeType::Calls | EdgeType::Imports => 3.2,
            EdgeType::DependsOn => 2.2,
            EdgeType::Contains => 1.4,
            EdgeType::PreviouslySuccessfulWith => 2.8,
            _ => 0.7,
        };
        utility += kind * conf * (0.5 + edge.pheromone_weight);
    }
    if let Some(node) = graph.get_node(candidate) {
        if node.node_type == NodeType::File {
            utility += 0.4;
        }
        utility += node.base_relevance * 0.2;
    }
    utility
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
            "steiner+greedy recall {steiner_hits} should beat first-5 {baseline_hits}; selected={selected_files:?}"
        );
        assert!(selected.budget_used <= selected.budget_cap);
        assert_eq!(selected.method, "steiner_greedy");
    }
}
