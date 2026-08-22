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
            gold_files: vec!["physarum.rs".into(), "graph.rs".into()],
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
                assert!(view.budget_cap >= 900);
                assert_eq!(
                    view.coverage.as_ref().map(|c| c.claim.as_str()),
                    Some("no_recorded_gap")
                );
            }
            scored.push(metrics);
        }
        assert_eq!(scored.len(), 3);
    }
}
