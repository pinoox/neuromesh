use neuromesh_graph::{NeuralProjectGraph, TraceDirection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRetrievalResult {
    pub changed_node: String,
    pub affected_files: Vec<String>,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
    pub related_tests: Vec<String>,
    pub config_files: Vec<String>,
}

/// Retrieve minimum sufficient impact context for a changed node via existing graph.
pub fn retrieve_impact_context(
    graph: &NeuralProjectGraph,
    query: &str,
    depth: u8,
) -> ImpactRetrievalResult {
    let mut affected = HashSet::new();
    let mut callers = Vec::new();
    let mut callees = Vec::new();
    let mut related_tests = Vec::new();
    let mut config_files = Vec::new();

    let depth = depth as usize;
    let trace_in = graph.trace_symbol(query, TraceDirection::Inbound, depth);
    for c in &trace_in.callers {
        let path = c.file_path.to_string_lossy().replace('\\', "/");
        callers.push(path.clone());
        affected.insert(path);
    }

    let trace_out = graph.trace_symbol(query, TraceDirection::Outbound, depth);
    for c in &trace_out.callees {
        let path = c.file_path.to_string_lossy().replace('\\', "/");
        callees.push(path.clone());
        affected.insert(path);
    }

    if let Some(node) = graph.resolve_best(query) {
        let origin = node.file_path.to_string_lossy().replace('\\', "/");
        affected.insert(origin);
    }

    for path in &affected {
        let pl = path.to_lowercase();
        if pl.contains("test") || pl.contains("spec") || pl.contains("_test.") {
            related_tests.push(path.clone());
        }
        if pl.contains("config")
            || pl.ends_with(".json")
            || pl.ends_with(".toml")
            || pl.ends_with(".yaml")
            || pl.ends_with(".yml")
            || pl.contains(".env")
        {
            config_files.push(path.clone());
        }
    }

    let mut affected_files: Vec<String> = affected.into_iter().collect();
    affected_files.sort();

    ImpactRetrievalResult {
        changed_node: query.to_string(),
        affected_files,
        callers,
        callees,
        related_tests,
        config_files,
    }
}

/// Impact Recall: fraction of gold affected files retrieved.
pub fn impact_recall(gold: &[String], retrieved: &[String]) -> f32 {
    if gold.is_empty() {
        return 1.0;
    }
    let hits = gold
        .iter()
        .filter(|g| retrieved.iter().any(|r| paths_match(g, r)))
        .count();
    hits as f32 / gold.len() as f32
}

/// Impact Precision: relevant retrieved / all retrieved.
pub fn impact_precision(gold: &[String], retrieved: &[String]) -> f32 {
    if retrieved.is_empty() {
        return 0.0;
    }
    let hits = retrieved
        .iter()
        .filter(|r| gold.iter().any(|g| paths_match(g, r)))
        .count();
    hits as f32 / retrieved.len() as f32
}

fn paths_match(gold: &str, path: &str) -> bool {
    let g = gold.replace('\\', "/");
    let p = path.replace('\\', "/");
    p.ends_with(&g) || p.contains(&g) || g.ends_with(&p)
}
