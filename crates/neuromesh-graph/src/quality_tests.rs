#[cfg(test)]
mod tests {
    use crate::NeuralProjectGraph;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::PathBuf;
    use std::time::Instant;

    fn indexed(rel: &str) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("neuromesh"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: "test".into(),
            byte_size: 100,
            token_count: 80,
            language: SourceLanguage::Rust,
            last_modified: chrono::Utc::now(),
        }
    }

    #[test]
    fn unique_resolution_does_not_explode_edges() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let caller = r#"
use neuromesh_task::TaskSignatureExtractor;
pub fn handle_tool_call() {
    TaskSignatureExtractor::extract("x");
}
"#;
        let callee = r#"
pub struct TaskSignatureExtractor;
impl TaskSignatureExtractor {
    pub fn extract(prompt: &str) -> String { prompt.to_string() }
}
"#;
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-mcp/src/tools.rs"),
                caller,
                SourceLanguage::Rust,
            ),
            Some(caller),
        );
        graph.ingest_file(
            &indexed("crates/neuromesh-task/src/signature.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-task/src/signature.rs"),
                callee,
                SourceLanguage::Rust,
            ),
            Some(callee),
        );
        graph.finalize_links();

        let stats = graph.stats();
        assert!(stats.total_edges < 20, "edges exploded: {}", stats.total_edges);
        assert!(stats.resolved_imports >= 1);
        assert!(stats.resolved_calls >= 1);

        let start = Instant::now();
        let hits = graph.search_symbols("handle_tool_call", 10);
        assert!(start.elapsed().as_millis() < 50);
        assert!(hits.iter().any(|h| h.name == "handle_tool_call"));

        let deps = graph.resolve_best("handle_tool_call").unwrap();
        let neighbors = graph.get_neighbor_views(&deps.id);
        assert!(
            neighbors.iter().any(|n| n.node.name == "extract" || n.node.name == "TaskSignatureExtractor"),
            "neighbors = {:?}",
            neighbors.iter().map(|n| n.node.name.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn search_is_ranked_not_bidirectional_contains() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let src = "pub fn neuromesh_get_context() {}\npub fn activate() {}\n";
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/lib.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("lib.rs"), src, SourceLanguage::Rust),
            Some(src),
        );
        graph.finalize_links();
        let hits = graph.search_symbols("get_context", 8);
        assert!(hits.iter().any(|h| h.name == "neuromesh_get_context"));
        assert!(!hits.iter().any(|h| h.name == "activate" && h.score >= hits[0].score));
    }
}
