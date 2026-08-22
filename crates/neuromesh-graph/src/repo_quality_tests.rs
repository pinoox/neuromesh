#[cfg(test)]
mod tests {
    use crate::NeuralProjectGraph;
    use neuromesh_core::ProjectId;
    use neuromesh_index::ProjectWalker;
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::PathBuf;
    use std::time::Instant;

    fn workspace_root() -> Option<PathBuf> {
        let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..4 {
            if current.join("Cargo.toml").exists() && current.join("crates").exists() {
                return Some(current);
            }
            current = current.parent()?.to_path_buf();
        }
        None
    }

    #[test]
    fn indexes_real_neuromesh_repo_with_usable_graph() {
        let Some(root) = workspace_root() else {
            return;
        };
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let walker = ProjectWalker::new(root.clone(), ProjectId::new("neuromesh"));
        let scanned = walker.scan().expect("scan workspace");
        assert!(scanned.len() >= 20, "too few files: {}", scanned.len());

        let started = Instant::now();
        for (file, content) in &scanned {
            let ast = CodeIntelligenceEngine::analyze(&file.relative_path, content, file.language);
            graph.ingest_file(file, &ast, Some(content));
        }
        graph.finalize_links();
        let index_ms = started.elapsed().as_millis();

        let stats = graph.stats();
        assert!(stats.total_nodes > 50);
        assert!(
            stats.total_edges < stats.total_nodes * 8,
            "edge explosion: {} edges for {} nodes",
            stats.total_edges,
            stats.total_nodes
        );
        assert!(stats.resolved_calls >= 5, "no resolved calls: {:?}", stats);
        assert!(stats.resolved_imports >= 5, "no resolved imports: {:?}", stats);
        assert!(index_ms < 30_000, "index too slow: {index_ms}ms");

        let search_started = Instant::now();
        let hits = graph.search_symbols("handle_tool_call", 10);
        let search_ms = search_started.elapsed().as_millis();
        assert!(search_ms < 80, "search timeout-class latency: {search_ms}ms");
        assert!(
            hits.iter().any(|h| h.name == "handle_tool_call"),
            "missing handle_tool_call in {:?}",
            hits.iter().map(|h| h.name.clone()).collect::<Vec<_>>()
        );

        let deps = graph.resolve_best("handle_tool_call").expect("resolve");
        let neighbors = graph.get_neighbor_views(&deps.id);
        assert!(
            !neighbors.is_empty(),
            "handle_tool_call should have structural neighbors"
        );

        let arch = graph.architecture_summary();
        assert!(arch.languages.iter().any(|(lang, _)| lang == "rs"));
        assert!(arch.packages.iter().any(|p| p.name.contains("neuromesh")));

        let trace = graph.trace_symbol("handle_tool_call", crate::TraceDirection::Both, 3);
        assert!(trace.origin.is_some());

        eprintln!(
            "NeuroMesh quality: files={} nodes={} edges={} calls={} imports={} index_ms={} search_ms={} neighbors={}",
            scanned.len(),
            stats.total_nodes,
            stats.total_edges,
            stats.resolved_calls,
            stats.resolved_imports,
            index_ms,
            search_ms,
            neighbors.len()
        );
    }
}
