//! CI gate: the packet is a function of the graph and the question, nothing else.
//!
//! Every gold prompt on every fixture is answered several times, each round
//! with a fresh activator and registry (so every `HashMap` inside gets a new
//! hash seed), and the file list must not move. Until issue #15 the Physarum
//! sidecar was gated on elapsed milliseconds and the solver walked hash maps
//! whose order changes per instance, so the same question gave different
//! packets between two runs — and a gold failure on CI could not be told
//! apart from a retrieval regression.
//!
//! The gate also demands that the sidecar actually fired on some cases; a
//! sidecar that never runs is trivially stable and would hide a regression
//! that switched it off. `NM_DET_ROUNDS` overrides the round count.

use neuromesh_context::gold::{builtin_gold_tasks, fixture_gold_cases, GoldTask};
use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_task::TaskSignatureExtractor;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DEFAULT_ROUNDS: usize = 6;
/// Cases on which the sidecar must have fired, otherwise this gate proves nothing.
const MIN_SIDECAR_CASES: usize = 3;

fn rounds() -> usize {
    std::env::var("NM_DET_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n: &usize| *n >= 2)
        .unwrap_or(DEFAULT_ROUNDS)
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
}

/// Everything a caller can observe about a packet short of the rendered text:
/// files in emission order with their reason and sidecar flag, the folds, the
/// token count, and the selection method. Order is compared too — the packet
/// is read top-down, so two runs that ship the same files in a different order
/// are not the same answer.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Answer {
    files: Vec<(String, String, bool)>,
    fold_ids: Vec<String>,
    active_tokens: usize,
    method: String,
    physarum_used: bool,
}

impl Answer {
    fn of(view: &neuromesh_core::ContextView) -> Self {
        Self {
            files: view
                .active_nodes
                .iter()
                .filter(|n| n.node.node_type == neuromesh_core::NodeType::File)
                .map(|n| {
                    (
                        n.node.file_path.to_string_lossy().replace('\\', "/"),
                        n.expansion_reason.clone().unwrap_or_default(),
                        n.sidecar,
                    )
                })
                .collect(),
            fold_ids: view.fold_ids.clone(),
            active_tokens: view.active_tokens,
            method: view.selection_method.clone(),
            physarum_used: view.physarum_used,
        }
    }
}

#[test]
fn same_question_same_packet_across_fresh_activators() {
    let rounds = rounds();
    let mut graphs: BTreeMap<&'static str, NeuralProjectGraph> = BTreeMap::new();
    let mut unstable: Vec<String> = Vec::new();
    let mut sidecar_cases = 0usize;
    let mut cases = 0usize;

    for (dir, task) in fixture_gold_cases() {
        let fixture = fixtures_root().join(dir);
        if !fixture.exists() {
            continue;
        }
        let graph = graphs.entry(dir).or_insert_with(|| {
            let graph = NeuralProjectGraph::new(ProjectId::new(dir));
            let walker = ProjectWalker::new(fixture.clone(), ProjectId::new(dir));
            let scanned = walker.scan().expect("scan fixture");
            graph.ingest_workspace(&scanned);
            graph
        });
        let signature = production_signature(&task);
        let mut seen: Vec<Answer> = Vec::new();
        for _ in 0..rounds {
            let registry = Arc::new(ReversibleContextRegistry::new());
            let activator = ContextActivator::new(registry);
            let view = activator.activate(graph, &signature, OptimizationMode::Balanced);
            seen.push(Answer::of(&view));
        }
        cases += 1;
        if seen.iter().any(|a| a.physarum_used) {
            sidecar_cases += 1;
        }
        let distinct: BTreeSet<String> = seen.iter().map(|a| format!("{a:?}")).collect();
        if distinct.len() > 1 {
            unstable.push(format!(
                "{dir}/{}: {} distinct packets in {rounds} rounds:\n  {}",
                task.id,
                distinct.len(),
                distinct.into_iter().collect::<Vec<_>>().join("\n  ")
            ));
        }
    }

    assert!(
        cases > 0,
        "no fixture gold cases found under {:?}",
        fixtures_root()
    );
    assert!(
        unstable.is_empty(),
        "{} of {cases} gold cases changed their packet between runs:\n{}",
        unstable.len(),
        unstable.join("\n")
    );
    assert!(
        sidecar_cases >= MIN_SIDECAR_CASES,
        "Physarum sidecar fired on only {sidecar_cases} of {cases} cases (need {MIN_SIDECAR_CASES}); \
         the gate is not exercising the sidecar path"
    );
    eprintln!(
        "packet_determinism: {cases} cases x {rounds} rounds stable; sidecar fired on {sidecar_cases}"
    );
}

/// The same check on this repository's own graph. The fixtures are small
/// enough that a question rarely resolves more seeds than the cap allows;
/// here it does, and which seeds survive the cap used to follow hash order.
/// Fewer rounds because indexing the workspace is the expensive part.
#[test]
fn same_question_same_packet_on_this_repository() {
    let rounds = rounds().clamp(2, 3);
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = loop {
        if root.join("Cargo.toml").exists() && root.join("crates").exists() {
            break root;
        }
        let Some(parent) = root.parent() else {
            panic!(
                "workspace root not found above {}",
                env!("CARGO_MANIFEST_DIR")
            );
        };
        root = parent.to_path_buf();
    };
    let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
    let walker = ProjectWalker::new(root.clone(), ProjectId::new("neuromesh"));
    let scanned = walker.scan().expect("scan workspace");
    graph.ingest_workspace(&scanned);

    let mut unstable: Vec<String> = Vec::new();
    let mut cases = 0usize;
    let mut sidecar_cases = 0usize;
    for task in builtin_gold_tasks() {
        let signature = production_signature(&task);
        let mut seen: Vec<Answer> = Vec::new();
        for _ in 0..rounds {
            let activator = ContextActivator::new(Arc::new(ReversibleContextRegistry::new()));
            let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
            seen.push(Answer::of(&view));
        }
        cases += 1;
        if seen.iter().any(|a| a.physarum_used) {
            sidecar_cases += 1;
        }
        let distinct: BTreeSet<String> = seen.iter().map(|a| format!("{a:?}")).collect();
        if distinct.len() > 1 {
            unstable.push(format!(
                "{}: {} distinct packets in {rounds} rounds:\n  {}",
                task.id,
                distinct.len(),
                distinct.into_iter().collect::<Vec<_>>().join("\n  ")
            ));
        }
    }
    assert!(
        unstable.is_empty(),
        "{} of {cases} repo gold cases changed their packet between runs:\n{}",
        unstable.len(),
        unstable.join("\n")
    );
    eprintln!(
        "packet_determinism(repo): {cases} cases x {rounds} rounds stable; sidecar fired on {sidecar_cases}"
    );
}

/// The signature a live session builds: extractor plus auto-extracted
/// keywords, seed engine on. `signature_for_gold_task` switches the seed
/// engine off for the gold gate; this gate is about stability of the path
/// users hit, so it must not.
fn production_signature(task: &GoldTask) -> neuromesh_core::TaskSignature {
    let mut signature = TaskSignatureExtractor::extract(&task.prompt);
    apply_auto_extract_keywords(&mut signature, &task.prompt, true);
    signature
}
