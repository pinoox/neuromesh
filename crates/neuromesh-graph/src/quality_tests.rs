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
        assert!(
            stats.total_edges < 20,
            "edges exploded: {}",
            stats.total_edges
        );
        assert!(stats.resolved_imports >= 1);
        assert!(stats.resolved_calls >= 1);

        let start = Instant::now();
        let hits = graph.search_symbols("handle_tool_call", 10);
        assert!(start.elapsed().as_millis() < 50);
        assert!(hits.iter().any(|h| h.name == "handle_tool_call"));

        let deps = graph.resolve_best("handle_tool_call").unwrap();
        let neighbors = graph.get_neighbor_views(&deps.id);
        assert!(
            neighbors
                .iter()
                .any(|n| n.node.name == "extract" || n.node.name == "TaskSignatureExtractor"),
            "neighbors = {:?}",
            neighbors
                .iter()
                .map(|n| n.node.name.clone())
                .collect::<Vec<_>>()
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
        assert!(!hits
            .iter()
            .any(|h| h.name == "activate" && h.score >= hits[0].score));
    }

    #[test]
    fn impl_self_call_is_proven_and_ambiguous_is_likely_or_unresolved() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let src = r#"
pub struct Bar;
impl Bar {
    pub fn foo(&self) { self.bar(); }
    pub fn bar(&self) {}
}
pub fn bar() {}
"#;
        graph.ingest_file(
            &indexed("src/bar.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("bar.rs"), src, SourceLanguage::Rust),
            Some(src),
        );
        graph.finalize_links();
        let foo = graph.resolve_unique("foo", Some("bar.rs")).unwrap();
        let neighbors = graph.get_neighbor_views(&foo);
        let bar_edge = neighbors
            .iter()
            .find(|n| n.node.name == "bar" && n.edge.edge_type == neuromesh_core::EdgeType::Calls)
            .expect("self.bar should resolve");
        assert_eq!(
            bar_edge.edge.confidence,
            neuromesh_core::EdgeConfidence::Proven
        );
    }

    #[test]
    fn field_receiver_resolves_activator_not_spreading_activate() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let tools = r#"
use neuromesh_context::ContextActivator;
use neuromesh_task::TaskSignatureExtractor;
pub struct Handler {
    activator: ContextActivator,
}
impl Handler {
    pub fn handle_tool_call(&self) {
        TaskSignatureExtractor::extract("demo");
        self.activator.activate();
    }
}
"#;
        let activator = r#"
pub struct ContextActivator;
impl ContextActivator {
    pub fn activate(&self) {}
}
"#;
        let spreading = r#"
pub struct SpreadingActivation;
impl SpreadingActivation {
    pub fn activate(&self) {}
}
"#;
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-mcp/src/tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.ingest_file(
            &indexed("crates/neuromesh-context/src/activator.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-context/src/activator.rs"),
                activator,
                SourceLanguage::Rust,
            ),
            Some(activator),
        );
        graph.ingest_file(
            &indexed("crates/neuromesh-graph/src/activation.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-graph/src/activation.rs"),
                spreading,
                SourceLanguage::Rust,
            ),
            Some(spreading),
        );
        graph.finalize_links();
        let handle = graph
            .resolve_unique("handle_tool_call", Some("tools.rs"))
            .expect("handle_tool_call");
        let neighbors = graph.get_neighbor_views(&handle);
        let activate = neighbors
            .iter()
            .find(|n| {
                n.node.name == "activate" && n.edge.edge_type == neuromesh_core::EdgeType::Calls
            })
            .expect("activate call");
        assert!(
            activate
                .node
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("activator.rs"),
            "activate resolved to {:?}",
            activate.node.file_path
        );
        assert_eq!(
            activate.edge.confidence,
            neuromesh_core::EdgeConfidence::Proven
        );
    }

    #[test]
    fn js_extension_activate_does_not_steal_rust_callee() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let tools = r#"
use neuromesh_context::ContextActivator;
pub struct Handler {
    activator: ContextActivator,
}
impl Handler {
    pub fn handle_tool_call(&self) {
        self.activator.activate();
    }
}
"#;
        let activator = r#"
pub struct ContextActivator;
impl ContextActivator {
    pub fn activate(&self) {}
}
"#;
        let js = "function activate(context) { return 1; }\n";
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-mcp/src/tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.ingest_file(
            &indexed("crates/neuromesh-context/src/activator.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("crates/neuromesh-context/src/activator.rs"),
                activator,
                SourceLanguage::Rust,
            ),
            Some(activator),
        );
        graph.ingest_file(
            &indexed("editors/vscode-neuromesh/extension.js"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("editors/vscode-neuromesh/extension.js"),
                js,
                SourceLanguage::JavaScript,
            ),
            Some(js),
        );
        graph.finalize_links();
        let handle = graph
            .resolve_unique("handle_tool_call", Some("tools.rs"))
            .expect("handle_tool_call");
        let activate = graph
            .get_neighbor_views(&handle)
            .into_iter()
            .find(|n| {
                n.node.name == "activate" && n.edge.edge_type == neuromesh_core::EdgeType::Calls
            })
            .expect("activate call");
        assert!(
            activate
                .node
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("activator.rs"),
            "activate resolved to {:?}",
            activate.node.file_path
        );
        let (ranked, _) = graph
            .resolve_ranked("activate", None, None)
            .expect("ranked activate");
        let ranked_node = graph.get_node(&ranked).expect("ranked node");
        assert!(
            ranked_node
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("activator.rs"),
            "resolve_ranked(activate) picked {:?}",
            ranked_node.file_path
        );
    }

    #[test]
    fn incremental_hash_skips_and_persist_roundtrips() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let src = "pub fn persist_me() {}\n";
        let mut file = indexed("src/persist.rs");
        file.blake3_hash = "hash-a".into();
        graph.ingest_file(
            &file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("persist.rs"),
                src,
                SourceLanguage::Rust,
            ),
            Some(src),
        );
        graph.finalize_links();
        let nodes_after_first = graph.stats().total_nodes;

        graph.ingest_file(
            &file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("persist.rs"),
                src,
                SourceLanguage::Rust,
            ),
            Some(src),
        );
        assert_eq!(graph.stats().total_nodes, nodes_after_first);

        let dir = std::env::temp_dir().join(format!("neuromesh-persist-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        graph.save_persisted(&dir).expect("save");
        let loaded = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        assert!(loaded.load_persisted(&dir));
        assert!(loaded
            .resolve_unique("persist_me", Some("persist.rs"))
            .is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn typescript_export_table_resolves_import() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let lib = "export function extractIntent() { return 1; }\n";
        let app =
            "import { extractIntent } from './lib';\nexport function run() { extractIntent(); }\n";
        let lib_file = IndexedFile {
            project_id: ProjectId::new("neuromesh"),
            relative_path: PathBuf::from("src/lib.ts"),
            full_path: PathBuf::from("src/lib.ts"),
            blake3_hash: "lib".into(),
            byte_size: 40,
            token_count: 20,
            language: SourceLanguage::TypeScript,
            last_modified: chrono::Utc::now(),
        };
        let mut app_file = lib_file.clone();
        app_file.relative_path = PathBuf::from("src/app.ts");
        app_file.full_path = PathBuf::from("src/app.ts");
        app_file.blake3_hash = "app".into();
        graph.ingest_file(
            &lib_file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("lib.ts"),
                lib,
                SourceLanguage::TypeScript,
            ),
            Some(lib),
        );
        graph.ingest_file(
            &app_file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("app.ts"),
                app,
                SourceLanguage::TypeScript,
            ),
            Some(app),
        );
        graph.finalize_links();
        assert!(graph.stats().resolved_imports >= 1);
        let resolved = graph.resolve_ranked("extractIntent", Some("./lib"), None);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().1, neuromesh_core::EdgeConfidence::Proven);
    }

    #[test]
    fn searcher_beats_lowercase_field_twin() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        let searcher_mod = r#"
pub struct Searcher {
    needle: String,
}

impl Searcher {
    pub fn search(&self, haystack: &str) -> bool {
        haystack.contains(&self.needle)
    }
}
"#;
        let query_fn = r#"
pub fn searcher(haystack: &str, needle: &str) -> bool {
    let a = haystack.len();
    let b = needle.len();
    let c = a.saturating_sub(b);
    let d = c.saturating_add(2);
    haystack.contains(needle) && d > 0
}
"#;
        graph.ingest_file(
            &indexed("src/searcher/mod.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/searcher/mod.rs"),
                searcher_mod,
                SourceLanguage::Rust,
            ),
            Some(searcher_mod),
        );
        graph.ingest_file(
            &indexed("src/query.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/query.rs"),
                query_fn,
                SourceLanguage::Rust,
            ),
            Some(query_fn),
        );
        graph.finalize_links();

        let hits = graph.search_symbols("Searcher", 5);
        assert!(
            !hits.is_empty(),
            "search_symbols must find Searcher: {hits:?}"
        );
        let top = &hits[0];
        assert_eq!(top.name, "Searcher");
        assert!(
            top.file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("searcher/mod.rs"),
            "search_symbols top path {:?}",
            top.file_path
        );

        let best = graph.resolve_best("Searcher").expect("resolve_best");
        assert_eq!(best.name, "Searcher");
        assert!(
            best.file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("searcher/mod.rs"),
            "resolve_best path {:?}",
            best.file_path
        );

        let (ranked, _) = graph
            .resolve_ranked("Searcher", None, None)
            .expect("resolve_ranked");
        let ranked_node = graph.get_node(&ranked).expect("ranked node");
        assert_eq!(ranked_node.name, "Searcher");
        assert!(ranked_node
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("searcher/mod.rs"));
    }

    #[test]
    fn php_throw_and_catch_are_inbound_calls() {
        let graph = NeuralProjectGraph::new(ProjectId::new("php"));
        let exception = r#"
<?php
namespace App\Installer\Component;
final class InstallPlatformException extends \RuntimeException {}
"#;
        let config = r#"
<?php
namespace App\Installer\Component;
final class InstallPlatformConfig
{
    public function load(): array
    {
        throw new InstallPlatformException('missing');
    }
    public function validate(array $config): void
    {
        throw new InstallPlatformException('invalid');
    }
}
"#;
        let command = r#"
<?php
use App\Installer\Component\InstallPlatformException;
final class InstallPlatformCommand
{
    public function execute(): int
    {
        try {
            (new InstallPlatformConfig())->load();
        } catch (InstallPlatformException $e) {
            return 1;
        }
        return 0;
    }
}
"#;
        graph.ingest_file(
            &indexed("src/InstallPlatformException.php"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/InstallPlatformException.php"),
                exception,
                SourceLanguage::PHP,
            ),
            Some(exception),
        );
        graph.ingest_file(
            &indexed("src/InstallPlatformConfig.php"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/InstallPlatformConfig.php"),
                config,
                SourceLanguage::PHP,
            ),
            Some(config),
        );
        graph.ingest_file(
            &indexed("src/InstallPlatformCommand.php"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/InstallPlatformCommand.php"),
                command,
                SourceLanguage::PHP,
            ),
            Some(command),
        );
        graph.finalize_links();

        let trace = graph.trace_symbol(
            "InstallPlatformException",
            crate::TraceDirection::Inbound,
            3,
        );
        let callers: Vec<_> = trace.callers.iter().map(|h| h.name.as_str()).collect();
        assert!(
            callers.contains(&"load") && callers.contains(&"validate"),
            "expected throw sites as callers, got {callers:?}"
        );
        assert!(
            callers.contains(&"execute"),
            "expected catch site as caller, got {callers:?}"
        );
    }
}
