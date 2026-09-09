#[cfg(test)]
mod tests {
    use crate::NeuralProjectGraph;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::{Path, PathBuf};

    fn indexed(rel: &str, hash: &str) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("ignored-by-graph"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: hash.into(),
            byte_size: 100,
            token_count: 80,
            language: SourceLanguage::Rust,
            last_modified: chrono::Utc::now(),
        }
    }

    fn ingest(graph: &NeuralProjectGraph, rel: &str, hash: &str, src: &str) {
        let path = PathBuf::from(rel);
        graph.ingest_file(
            &indexed(rel, hash),
            &CodeIntelligenceEngine::analyze(&path, src, SourceLanguage::Rust),
            Some(src),
        );
    }

    const SHARED: &str = "pub fn shared_helper() -> usize { 42 }\n";

    #[test]
    fn a_clean_single_project_graph_satisfies_the_invariant() {
        let graph = NeuralProjectGraph::new(ProjectId::new("project-a"));
        ingest(&graph, "src/lib.rs", "hash-a", SHARED);
        ingest(&graph, "src/only_in_a.rs", "hash-b", "pub fn only_a() {}\n");

        assert_eq!(graph.assert_single_project(), Ok(()));
        assert_eq!(graph.foreign_nodes().len(), 0);
        assert_eq!(graph.enforce_single_project(), 0);
    }

    /// The real leak path (design doc case "A2").
    ///
    /// `ingest_file_keep` returns early when the stored hash for a relative path
    /// equals the incoming hash. Two projects very often share a relative path
    /// *and* its exact contents — `LICENSE`, `.gitignore`, an empty
    /// `__init__.py`, boilerplate config. When that happens after a project
    /// switch the new project's file is never ingested and the previous
    /// project's nodes stay in the graph, still tagged with its `project_id`.
    #[test]
    fn identical_shared_path_leaks_nodes_across_a_project_switch() {
        let graph = NeuralProjectGraph::new(ProjectId::new("project-a"));
        ingest(&graph, "src/lib.rs", "identical", SHARED);

        // What `adopt_workspace_from_initialize` does on a workspace change.
        graph.set_project_id(ProjectId::new("project-b"));
        // Project B has the same relative path with byte-identical contents.
        ingest(&graph, "src/lib.rs", "identical", SHARED);

        let foreign = graph
            .assert_single_project()
            .expect_err("project-a nodes must still be detectable after the switch");
        assert!(
            foreign
                .iter()
                .any(|(pid, path)| pid == "project-a" && path == "src/lib.rs"),
            "expected a project-a node at src/lib.rs, got {foreign:?}"
        );
    }

    #[test]
    fn enforcing_the_invariant_evicts_foreign_nodes_and_clears_their_hash() {
        let graph = NeuralProjectGraph::new(ProjectId::new("project-a"));
        ingest(&graph, "src/lib.rs", "identical", SHARED);
        ingest(&graph, "src/only_in_a.rs", "hash-b", "pub fn only_a() {}\n");

        graph.set_project_id(ProjectId::new("project-b"));
        assert!(graph.assert_single_project().is_err());

        let evicted = graph.enforce_single_project();
        assert_eq!(evicted, 2, "both project-a files should be evicted");
        assert_eq!(graph.assert_single_project(), Ok(()));

        // The stale hashes must be gone, otherwise the next scan would skip
        // these paths again and the leak would silently return.
        let hashes = graph.file_hashes();
        assert!(!hashes.contains_key("src/lib.rs"));
        assert!(!hashes.contains_key("src/only_in_a.rs"));
    }

    #[test]
    fn eviction_leaves_the_current_projects_nodes_alone() {
        let graph = NeuralProjectGraph::new(ProjectId::new("project-a"));
        ingest(&graph, "src/stale.rs", "hash-a", "pub fn stale() {}\n");

        graph.set_project_id(ProjectId::new("project-b"));
        ingest(&graph, "src/fresh.rs", "hash-b", "pub fn fresh() {}\n");

        assert_eq!(graph.enforce_single_project(), 1);
        let hashes = graph.file_hashes();
        assert!(hashes.contains_key("src/fresh.rs"), "current project kept");
        assert!(
            !hashes.contains_key("src/stale.rs"),
            "foreign project dropped"
        );
    }

    /// P0-5: an authoritative workspace root must survive ingest.
    ///
    /// `ingest_workspace_inner` used to overwrite `workspace_root` with a guess
    /// derived from the first file of the batch, undoing the root that
    /// `reindex_incremental` had just set. Inside a monorepo that guess can land
    /// on a nested package.
    #[test]
    fn ingest_does_not_overwrite_an_explicit_workspace_root() {
        let graph = NeuralProjectGraph::new(ProjectId::new("project-a"));
        let authoritative = PathBuf::from("/authoritative/root");
        graph.set_workspace(&authoritative);

        let mut file = indexed("src/main.rs", "hash-a");
        file.full_path = PathBuf::from("/guessed/elsewhere/src/main.rs");
        graph.ingest_workspace(&[(file, "pub fn main() {}\n".to_string())]);

        assert_eq!(
            graph.workspace_root().as_deref(),
            Some(authoritative.as_path())
        );
    }

    #[test]
    fn ingest_still_infers_a_root_when_none_was_set() {
        let graph = NeuralProjectGraph::new(ProjectId::new("project-a"));
        assert!(graph.workspace_root().is_none());

        let mut file = indexed("src/main.rs", "hash-a");
        file.full_path = PathBuf::from("/inferred/root/src/main.rs");
        graph.ingest_workspace(&[(file, "pub fn main() {}\n".to_string())]);

        assert_eq!(
            graph.workspace_root().as_deref(),
            Some(Path::new("/inferred/root"))
        );
    }
}
