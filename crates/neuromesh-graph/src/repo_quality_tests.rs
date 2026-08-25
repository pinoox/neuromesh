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

    /// Compact mesh gates: snapshot cold load and one-file reindex must both stay
    /// far under a full workspace index, and the snapshot must carry no file bodies.
    #[test]
    fn snapshot_load_and_single_file_reindex_beat_full_index() {
        let Some(root) = workspace_root() else {
            return;
        };
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let walker = ProjectWalker::new(root.clone(), ProjectId::new("neuromesh"));
        let report = walker.scan_report().expect("scan workspace");
        assert!(report.files.len() >= 20);

        let started = Instant::now();
        graph.ingest_scan_report(&report);
        let full_index_ms = started.elapsed().as_millis();
        let nodes_before = graph.stats().total_nodes;
        assert!(nodes_before > 50);

        let dir = std::env::temp_dir().join(format!("neuromesh-gate-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let snapshot_path = dir.join("graph.bin");
        graph.save_to(&snapshot_path).expect("save snapshot");
        let snapshot_bytes = std::fs::metadata(&snapshot_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let reloaded = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let load_started = Instant::now();
        assert!(reloaded.load_from(&snapshot_path).expect("load snapshot"));
        let load_ms = load_started.elapsed().as_millis();
        assert_eq!(reloaded.stats().total_nodes, nodes_before);
        assert!(
            reloaded.get_all_nodes().iter().all(|n| n.content.is_none()),
            "snapshot must not carry file bodies"
        );
        assert!(
            load_ms <= full_index_ms.max(1),
            "snapshot load {load_ms}ms should not exceed full index {full_index_ms}ms"
        );

        // One changed file: parse that file plus local relink, not a whole-mesh rebuild.
        let (file, content) = report
            .files
            .iter()
            .find(|(f, _)| {
                f.relative_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with("crates/neuromesh-graph/src/node.rs")
            })
            .or_else(|| report.files.first())
            .expect("a scanned file");
        let mut edited = file.clone();
        edited.blake3_hash = format!("{}-edited", file.blake3_hash);
        let ast = CodeIntelligenceEngine::analyze(&file.relative_path, content, file.language);

        let reindex_started = Instant::now();
        reloaded.ingest_file(&edited, &ast, Some(content));
        reloaded.finalize_links();
        let reindex_ms = reindex_started.elapsed().as_millis();
        assert!(
            reindex_ms <= full_index_ms.max(1),
            "single-file reindex {reindex_ms}ms should not exceed full index {full_index_ms}ms"
        );
        assert!(reloaded.stats().total_nodes >= nodes_before - 50);

        eprintln!(
            "compact mesh gates: files={} nodes={} full_index_ms={} snapshot_kb={} snapshot_load_ms={} one_file_reindex_ms={} unchanged_skipped={}",
            report.files.len(),
            nodes_before,
            full_index_ms,
            snapshot_bytes / 1024,
            load_ms,
            reindex_ms,
            report.unchanged
        );
        let _ = std::fs::remove_dir_all(dir);
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
        graph.ingest_workspace(&scanned);
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
        assert!(
            stats.resolved_imports >= 5,
            "no resolved imports: {:?}",
            stats
        );
        assert!(index_ms < 30_000, "index too slow: {index_ms}ms");

        let search_started = Instant::now();
        let hits = graph.search_symbols("handle_tool_call", 10);
        let search_ms = search_started.elapsed().as_millis();
        assert!(
            search_ms < 80,
            "search timeout-class latency: {search_ms}ms"
        );
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

        let tools_path = root
            .join("crates")
            .join("neuromesh-mcp")
            .join("src")
            .join("tools.rs");
        if let Ok(tools_src) = std::fs::read_to_string(&tools_path) {
            let ast = CodeIntelligenceEngine::analyze(
                &tools_path,
                &tools_src,
                neuromesh_index::SourceLanguage::Rust,
            );
            let handle = ast
                .symbols
                .iter()
                .find(|s| s.name == "handle_tool_call")
                .expect("real handle_tool_call");
            assert!(
                handle.calls.iter().any(|c| c == "extract"),
                "extract missing: {:?}",
                handle.calls
            );
            assert!(
                handle
                    .calls
                    .iter()
                    .any(|c| c == "evaluate" || c == "activate"),
                "evaluate/activate missing: {:?}",
                handle.calls
            );
            assert!(
                ast.relationships.iter().any(|r| {
                    r.source_symbol == "handle_tool_call"
                        && r.target_symbol == "activate"
                        && r.receiver_hint.as_deref() == Some("field:activator")
                }),
                "self.activator.activate should be field-scoped"
            );
        }

        let handle = graph.resolve_best("handle_tool_call").expect("resolve");
        let neighbors = graph.get_neighbor_views(&handle.id);
        let activate = neighbors.iter().find(|n| {
            n.node.name == "activate" && n.edge.edge_type == neuromesh_core::EdgeType::Calls
        });
        if let Some(activate) = activate {
            let path = activate.node.file_path.to_string_lossy().replace('\\', "/");
            assert!(
                path.ends_with("activator.rs"),
                "activate should be ContextActivator::activate, got {path}"
            );
            assert_eq!(
                activate.edge.confidence,
                neuromesh_core::EdgeConfidence::Proven
            );
        }

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
