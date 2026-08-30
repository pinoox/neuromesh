//! Generic-repo regression: intent packs must not cause spurious no_seed on non-Express fixtures.

use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::{IndexedFile, SourceLanguage};
use neuromesh_parser::CodeIntelligenceEngine;
use neuromesh_task::TaskSignatureExtractor;
use std::path::PathBuf;
use std::sync::Arc;

fn ingest_py(graph: &NeuralProjectGraph, rel: &str, src: &str) {
    let path = PathBuf::from(rel);
    graph.ingest_file(
        &IndexedFile {
            project_id: graph.project_id().clone(),
            relative_path: path.clone(),
            full_path: path,
            blake3_hash: "t".into(),
            byte_size: src.len() as u64,
            token_count: 40,
            language: SourceLanguage::Python,
            last_modified: chrono::Utc::now(),
        },
        &CodeIntelligenceEngine::analyze(&PathBuf::from(rel), src, SourceLanguage::Python),
        Some(src),
    );
}

#[test]
fn generic_fastapi_prompt_does_not_no_seed_with_auto_extract() {
    let graph = NeuralProjectGraph::new(ProjectId::new("mini-fastapi-generic"));
    ingest_py(
        &graph,
        "src/main.py",
        r#"
from fastapi import FastAPI
app = FastAPI()

@app.get("/health")
def health():
    return {"ok": True}
"#,
    );
    ingest_py(
        &graph,
        "src/sms_store.py",
        r#"
def save_message(body: str) -> None:
    pass
"#,
    );
    graph.finalize_links();

    let registry = Arc::new(ReversibleContextRegistry::new());
    let activator = ContextActivator::new(registry);
    let prompt = "How does the health endpoint work in this FastAPI app?";
    let mut sig = TaskSignatureExtractor::extract(prompt);
    apply_auto_extract_keywords(&mut sig, prompt, true);

    let view = activator.activate(&graph, &sig, OptimizationMode::Balanced);
    let claim = view.coverage.as_ref().map(|c| c.claim.as_str());
    assert_ne!(
        claim,
        Some("no_seed_resolved"),
        "generic FastAPI prompt must not no_seed, coverage={claim:?} keywords={:?}",
        sig.client_keywords
    );
}

#[test]
fn generic_symbol_prompt_stays_explain_not_trace_pack_only() {
    use neuromesh_context::retrieval::query_intent::{assisted_signals, classify_intent};

    let sig = TaskSignatureExtractor::extract("Where is save_message defined?");
    let intent = classify_intent(&sig);
    let (kw, exp) = assisted_signals(intent);
    assert!(
        kw.is_empty() && exp.is_empty(),
        "symbol explain/general prompts must not get trace packs, intent={intent:?}"
    );
}
