use neuromesh_core::{ContextView, NodeType};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldTask {
    pub id: String,
    pub prompt: String,
    pub gold_files: Vec<String>,
    #[serde(default)]
    pub expect_seeds_missed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldMetrics {
    pub id: String,
    pub recall: f32,
    pub precision: f32,
    pub tokens: usize,
    pub latency_ms: u64,
    pub unresolved: usize,
    pub seeds_missed: usize,
    pub coverage_claim: String,
}

pub fn builtin_gold_tasks() -> Vec<GoldTask> {
    vec![
        GoldTask {
            id: "handle_tool_call_intent".into(),
            prompt: "How does handle_tool_call extract intent?".into(),
            gold_files: vec![
                "tools.rs".into(),
                "signature.rs".into(),
                "activator.rs".into(),
            ],
            expect_seeds_missed: false,
        },
        GoldTask {
            id: "physarum_usage".into(),
            prompt: "Where is Physarum used?".into(),
            gold_files: vec!["physarum.rs".into(), "activation.rs".into()],
            expect_seeds_missed: false,
        },
        GoldTask {
            id: "missing_seed".into(),
            prompt: "What does `__no_such_symbol_xyz__` do?".into(),
            gold_files: Vec::new(),
            expect_seeds_missed: true,
        },
    ]
}

pub fn load_gold_tasks(path: &Path) -> Vec<GoldTask> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return builtin_gold_tasks();
    };
    parse_gold_toml(&raw).unwrap_or_else(builtin_gold_tasks)
}

fn parse_gold_toml(raw: &str) -> Option<Vec<GoldTask>> {
    let mut tasks = Vec::new();
    let mut current: Option<GoldTask> = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[task]]" {
            if let Some(task) = current.take() {
                tasks.push(task);
            }
            current = Some(GoldTask {
                id: String::new(),
                prompt: String::new(),
                gold_files: Vec::new(),
                expect_seeds_missed: false,
            });
            continue;
        }
        let task = current.as_mut()?;
        let (key, value) = line.split_once('=')?;
        let key = key.trim();
        let value = value.trim();
        match key {
            "id" => task.id = unquote(value),
            "prompt" => task.prompt = unquote(value),
            "expect_seeds_missed" => task.expect_seeds_missed = value == "true",
            "gold_files" => task.gold_files = parse_string_array(value),
            _ => {}
        }
    }
    if let Some(task) = current {
        tasks.push(task);
    }
    if tasks.is_empty() {
        None
    } else {
        Some(tasks)
    }
}

fn unquote(value: &str) -> String {
    value.trim_matches('"').to_string()
}

fn parse_string_array(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|part| unquote(part.trim()))
        .filter(|part| !part.is_empty())
        .collect()
}

pub fn packet_file_names(view: &ContextView) -> HashSet<String> {
    view.active_nodes
        .iter()
        .filter(|n| n.node.node_type == NodeType::File)
        .filter_map(|n| {
            n.node
                .file_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}

pub fn evaluate_view(task: &GoldTask, view: &ContextView, latency_ms: u64) -> GoldMetrics {
    let packet = packet_file_names(view);
    let gold: HashSet<String> = task.gold_files.iter().cloned().collect();
    let hits = gold.intersection(&packet).count();
    let recall = if gold.is_empty() {
        1.0
    } else {
        hits as f32 / gold.len() as f32
    };
    let precision = if packet.is_empty() {
        0.0
    } else {
        hits as f32 / packet.len() as f32
    };
    let seeds_missed = view
        .coverage
        .as_ref()
        .map(|c| c.seeds_missed.len())
        .unwrap_or(0);
    GoldMetrics {
        id: task.id.clone(),
        recall,
        precision,
        tokens: view.active_tokens,
        latency_ms,
        unresolved: view.unresolved.len(),
        seeds_missed,
        coverage_claim: view
            .coverage
            .as_ref()
            .map(|c| c.claim.clone())
            .unwrap_or_else(|| "unknown".into()),
    }
}

pub fn workspace_gold_path() -> Option<std::path::PathBuf> {
    let mut current = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..4 {
        let candidate = current.join("tests").join("gold_tasks.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        current = current.parent()?.to_path_buf();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activator::ContextActivator;
    use crate::registry::ReversibleContextRegistry;
    use neuromesh_core::{OptimizationMode, ProjectId};
    use neuromesh_graph::NeuralProjectGraph;
    use neuromesh_index::ProjectWalker;
    use neuromesh_task::TaskSignatureExtractor;
    use std::sync::Arc;
    use std::time::Instant;

    fn workspace_root() -> Option<std::path::PathBuf> {
        let mut current = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..4 {
            if current.join("Cargo.toml").exists() && current.join("crates").exists() {
                return Some(current);
            }
            current = current.parent()?.to_path_buf();
        }
        None
    }

    #[test]
    fn gold_tasks_toml_matches_builtin() {
        let Some(path) = workspace_gold_path() else {
            return;
        };
        let loaded = load_gold_tasks(&path);
        let builtin = builtin_gold_tasks();
        assert_eq!(loaded.len(), builtin.len());
        assert_eq!(loaded[0].id, "handle_tool_call_intent");
        assert!(loaded[2].expect_seeds_missed);
    }

    #[test]
    fn gold_harness_on_neuromesh_repo() {
        let Some(root) = workspace_root() else {
            return;
        };
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let walker = ProjectWalker::new(root.clone(), ProjectId::new("neuromesh"));
        let scanned = walker.scan().expect("scan workspace");
        graph.ingest_workspace(&scanned);

        let tasks = workspace_gold_path()
            .map(|p| load_gold_tasks(&p))
            .unwrap_or_else(builtin_gold_tasks);
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);

        let mut scored = Vec::new();
        for task in &tasks {
            let signature = TaskSignatureExtractor::extract(&task.prompt);
            let started = Instant::now();
            let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
            let latency_ms = started.elapsed().as_millis() as u64;
            assert!(
                latency_ms < 50,
                "{} context latency {latency_ms}ms",
                task.id
            );
            let metrics = evaluate_view(task, &view, latency_ms);
            if task.expect_seeds_missed {
                assert!(
                    !view
                        .coverage
                        .as_ref()
                        .map(|c| c.seeds_missed.is_empty())
                        .unwrap_or(true),
                    "missing seed must be reported: {:?}",
                    view.coverage
                );
                assert_eq!(metrics.coverage_claim, "partial");
            } else {
                assert!(
                    metrics.recall >= 0.8,
                    "{} recall {} packet={:?} gold={:?}",
                    task.id,
                    metrics.recall,
                    packet_file_names(&view),
                    task.gold_files
                );
                assert_eq!(view.budget_fill_cap, 8_000);
                assert!(
                    view.budget_fill_used <= view.budget_fill_cap,
                    "{} fill {} exceeded cap {}",
                    task.id,
                    view.budget_fill_used,
                    view.budget_fill_cap
                );
                assert!(
                    !packet_file_names(&view)
                        .iter()
                        .any(|name| name.ends_with(".md")),
                    "{} leaked markdown into packet: {:?}",
                    task.id,
                    packet_file_names(&view)
                );
                assert_eq!(
                    view.coverage.as_ref().map(|c| c.claim.as_str()),
                    Some("no_recorded_gap")
                );
            }
            scored.push(metrics);
        }
        assert_eq!(scored.len(), 3);

        let sig = TaskSignatureExtractor::extract("How does handle_tool_call extract intent?");
        let savings = activator.activate(&graph, &sig, OptimizationMode::MaxSavings);
        let quality = activator.activate(&graph, &sig, OptimizationMode::MaxQuality);
        let savings_files = packet_file_names(&savings).len();
        let quality_files = packet_file_names(&quality).len();
        assert!(
            savings.budget_fill_used <= savings.budget_fill_cap,
            "max_savings fill must respect 0 extra cap"
        );
        assert_eq!(savings.budget_fill_cap, 0);
        assert_eq!(quality.budget_fill_cap, 16_000);
        assert!(
            savings_files <= quality_files,
            "max_savings files {savings_files} should be <= max_quality {quality_files}"
        );
    }

    /// Honest live measurement on this workspace. Prints JSON for the v0.4 claim check.
    /// Baselines: dump-all files, v0.3 neighborhood dump, gold-file dump. Not the fake CLI `eval`.
    /// Run: cargo test -p neuromesh-context live_v04_measurement -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_v04_measurement_on_neuromesh_repo() {
        use neuromesh_core::NodeType;
        use std::collections::HashMap;

        let Some(root) = workspace_root() else {
            return;
        };
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let walker = ProjectWalker::new(root.clone(), ProjectId::new("neuromesh"));
        let scanned = walker.scan().expect("scan workspace");
        let index_started = Instant::now();
        graph.ingest_workspace(&scanned);
        let index_ms = index_started.elapsed().as_millis() as u64;
        let stats = graph.stats();
        let dump_all_tokens = graph.total_tokens();

        let mut gold_file_tokens: HashMap<String, usize> = HashMap::new();
        for (file, content) in &scanned {
            let name = file
                .relative_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let tokens = neuromesh_core::TokenCounter::count_tokens(content);
            gold_file_tokens
                .entry(name)
                .and_modify(|t| *t += tokens)
                .or_insert(tokens);
        }

        let search_started = Instant::now();
        let hits = graph.search_symbols("handle_tool_call", 10);
        let search_cold_ms = search_started.elapsed().as_millis() as u64;
        let mut search_warm = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            let _ = graph.search_symbols("handle_tool_call", 10);
            search_warm.push(t.elapsed().as_micros() as u64);
        }
        search_warm.sort_unstable();
        let search_warm_us = search_warm[search_warm.len() / 2];

        let deps = graph
            .resolve_best("handle_tool_call")
            .map(|n| graph.get_neighbor_views(&n.id).len())
            .unwrap_or(0);

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let modes = [
            OptimizationMode::MaxSavings,
            OptimizationMode::Balanced,
            OptimizationMode::MaxQuality,
        ];

        let mut extra = builtin_gold_tasks();
        extra.push(GoldTask {
            id: "search_ranking".into(),
            prompt: "How does neuromesh_search_symbols rank prefix vs exact matches?".into(),
            gold_files: vec!["graph.rs".into(), "tools.rs".into()],
            expect_seeds_missed: false,
        });
        extra.push(GoldTask {
            id: "finalize_links".into(),
            prompt: "Where does finalize_links unique-resolve import and call edges?".into(),
            gold_files: vec!["graph.rs".into()],
            expect_seeds_missed: false,
        });

        let mut runs = Vec::new();
        for task in &extra {
            let signature = TaskSignatureExtractor::extract(&task.prompt);
            let gold_dump: usize = task
                .gold_files
                .iter()
                .map(|name| gold_file_tokens.get(name).copied().unwrap_or(0))
                .sum();
            for mode in modes {
                let started = Instant::now();
                let view = activator.activate(&graph, &signature, mode);
                let first_ms = started.elapsed().as_millis() as u64;
                let mut warm = Vec::new();
                for _ in 0..8 {
                    let t = Instant::now();
                    let _ = activator.activate(&graph, &signature, mode);
                    warm.push(t.elapsed().as_micros() as u64);
                }
                warm.sort_unstable();
                let warm_us = warm[warm.len() / 2];
                let hops = match mode {
                    OptimizationMode::MaxQuality => 3,
                    OptimizationMode::Balanced => 2,
                    OptimizationMode::MaxSavings => 1,
                };
                let seed_ids: std::collections::HashSet<_> = view
                    .seeds
                    .iter()
                    .filter_map(|s| s.resolved_id.clone())
                    .collect();
                let neighborhood = if seed_ids.is_empty() {
                    std::collections::HashSet::new()
                } else {
                    graph.neighborhood(&seed_ids, hops)
                };
                let neighborhood_file_tokens: usize = neighborhood
                    .iter()
                    .filter_map(|id| graph.get_node(id))
                    .filter(|n| n.node_type == NodeType::File)
                    .map(|n| n.token_cost)
                    .sum();
                let metrics = evaluate_view(task, &view, first_ms);
                let packet_files = packet_file_names(&view);
                let vs_dump = if dump_all_tokens > 0 {
                    (dump_all_tokens.saturating_sub(view.active_tokens) as f32
                        / dump_all_tokens as f32)
                        * 100.0
                } else {
                    0.0
                };
                let vs_neigh = if neighborhood_file_tokens > 0 {
                    (neighborhood_file_tokens.saturating_sub(view.active_tokens) as f32
                        / neighborhood_file_tokens as f32)
                        * 100.0
                } else {
                    0.0
                };
                let vs_gold = if gold_dump > 0 {
                    (gold_dump.saturating_sub(view.active_tokens) as f32 / gold_dump as f32) * 100.0
                } else {
                    0.0
                };
                runs.push(serde_json::json!({
                    "id": task.id,
                    "prompt": task.prompt,
                    "mode": view.budget_mode,
                    "recall": metrics.recall,
                    "precision": metrics.precision,
                    "gold_files": task.gold_files,
                    "packet_files": packet_files.into_iter().collect::<Vec<_>>(),
                    "packet_tokens": view.active_tokens,
                    "budget_used": view.budget_used,
                    "budget_cap": view.budget_cap,
                    "coverage": metrics.coverage_claim,
                    "seeds_missed": metrics.seeds_missed,
                    "folds": view.fold_ids.len(),
                    "active_nodes": view.active_nodes.len(),
                    "neighborhood_nodes": neighborhood.len(),
                    "neighborhood_file_tokens": neighborhood_file_tokens,
                    "gold_file_dump_tokens": gold_dump,
                    "dump_all_tokens": dump_all_tokens,
                    "reduction_vs_dump_all_pct": vs_dump,
                    "reduction_vs_neighborhood_pct": vs_neigh,
                    "reduction_vs_gold_files_pct": vs_gold,
                    "first_ms": first_ms,
                    "warm_median_us": warm_us,
                    "under_50ms_warm": warm_us < 50_000,
                    "under_budget": view.active_tokens <= view.budget_cap,
                }));
            }
        }

        let report = serde_json::json!({
            "workspace": root.display().to_string(),
            "index_ms": index_ms,
            "files": scanned.len(),
            "stats": stats,
            "dump_all_file_tokens": dump_all_tokens,
            "search_handle_tool_call": {
                "cold_ms": search_cold_ms,
                "warm_median_us": search_warm_us,
                "hit": hits.iter().any(|h| h.name == "handle_tool_call"),
                "neighbors": deps,
            },
            "runs": runs,
        });
        eprintln!("LIVE_V04_JSON {}", report);
        assert!(scanned.len() >= 20);
        assert!(hits.iter().any(|h| h.name == "handle_tool_call"));
        assert!(
            index_ms < 30_000,
            "index too slow for this repo: {index_ms}ms"
        );
    }
}
