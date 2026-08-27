#[cfg(test)]
mod tests {
    use crate::intern::GraphSnapshot;
    use crate::NeuralProjectGraph;
    use neuromesh_core::{NodeType, ProjectId};
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::time::Instant;

    fn indexed(rel: &str) -> IndexedFile {
        indexed_lang(rel, SourceLanguage::Rust)
    }

    fn indexed_lang(rel: &str, language: SourceLanguage) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("neuromesh"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: "test".into(),
            byte_size: 100,
            token_count: 80,
            language,
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
        let path = dir.join("graph.json");
        graph.save_to(&path).expect("save");
        let loaded = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        assert!(loaded.load_from(&path).expect("load"));
        assert!(loaded
            .resolve_unique("persist_me", Some("persist.rs"))
            .is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn persist_snapshot_strips_file_bodies() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let src = "pub fn persist_me() { let x = 1; let y = 2; x + y }\n";
        let mut file = indexed("src/persist.rs");
        file.blake3_hash = "hash-body".into();
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
        let node = graph
            .get_node(&neuromesh_core::NodeId::from_file_path("src/persist.rs"))
            .expect("file node");
        assert!(node.content.is_none(), "graph must not store source bodies");
        assert!(graph.read_source(&node.file_path).is_some());

        let dir = std::env::temp_dir().join(format!("neuromesh-bin-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("graph.bin");
        graph.save_to(&path).expect("save bin");
        let loaded = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let started = Instant::now();
        assert!(loaded.load_from(&path).expect("load bin"));
        let load_ms = started.elapsed().as_millis();
        assert!(load_ms < 2_000, "snapshot load too slow: {load_ms}ms");
        assert!(loaded
            .get_node(&neuromesh_core::NodeId::from_file_path("src/persist.rs"))
            .is_some_and(|n| n.content.is_none()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reingest_file_relinks_inbound_calls() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let lib = "pub fn persist_me() {}\n";
        let app = "pub fn run() { persist_me(); }\n";
        let mut lib_file = indexed("src/lib.rs");
        lib_file.blake3_hash = "lib-a".into();
        let mut app_file = indexed("src/app.rs");
        app_file.blake3_hash = "app-a".into();
        graph.ingest_file(
            &lib_file,
            &CodeIntelligenceEngine::analyze(&PathBuf::from("lib.rs"), lib, SourceLanguage::Rust),
            Some(lib),
        );
        graph.ingest_file(
            &app_file,
            &CodeIntelligenceEngine::analyze(&PathBuf::from("app.rs"), app, SourceLanguage::Rust),
            Some(app),
        );
        graph.finalize_links();
        let persist = graph
            .resolve_unique("persist_me", Some("lib.rs"))
            .expect("persist_me");
        let inbound_before = graph
            .get_connected_neighbors(&persist)
            .into_iter()
            .filter(|(_, e)| e.edge_type == neuromesh_core::EdgeType::Calls)
            .count();
        assert!(inbound_before >= 1, "run should call persist_me");

        let lib2 = "pub fn persist_me() { let _ = 1; }\n";
        lib_file.blake3_hash = "lib-b".into();
        graph.ingest_file(
            &lib_file,
            &CodeIntelligenceEngine::analyze(&PathBuf::from("lib.rs"), lib2, SourceLanguage::Rust),
            Some(lib2),
        );
        graph.finalize_links();
        let persist = graph
            .resolve_unique("persist_me", Some("lib.rs"))
            .expect("persist_me after replace");
        let inbound_after = graph
            .get_connected_neighbors(&persist)
            .into_iter()
            .filter(|(_, e)| e.edge_type == neuromesh_core::EdgeType::Calls)
            .count();
        assert!(
            inbound_after >= 1,
            "inbound Calls must be re-queued after file replace"
        );
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
    fn search_prefers_production_safeparse_over_bench_twin() {
        let graph = NeuralProjectGraph::new(ProjectId::new("schema"));
        let core = r#"
export function parse(schema: object, data: unknown) {
  return data;
}
export function safeParse(schema: object, data: unknown) {
  return { success: true, data };
}
"#;
        let bench = r#"
export function safeParse(schema: object, data: unknown) {
  return { success: true, data };
}
export function parseSimpleObject(data: unknown) {
  return typeof data === "object";
}
export function parseNestedObject(data: unknown) {
  return parseSimpleObject(data);
}
"#;
        graph.ingest_file(
            &indexed_lang(
                "packages/schema/src/core/parse.ts",
                SourceLanguage::TypeScript,
            ),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("packages/schema/src/core/parse.ts"),
                core,
                SourceLanguage::TypeScript,
            ),
            Some(core),
        );
        graph.ingest_file(
            &indexed_lang("packages/bench/safeparse.ts", SourceLanguage::TypeScript),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("packages/bench/safeparse.ts"),
                bench,
                SourceLanguage::TypeScript,
            ),
            Some(bench),
        );
        graph.finalize_links();

        let hits = graph.search_symbols("safeParse", 5);
        assert!(!hits.is_empty(), "safeParse must be indexed, hits={hits:?}");
        let top_path = hits[0].file_path.to_string_lossy().replace('\\', "/");
        assert!(
            top_path.ends_with("packages/schema/src/core/parse.ts"),
            "safeParse must rank production parse.ts over bench/safeparse.ts, top={top_path} hits={hits:?}"
        );

        let parse_hits = graph.search_symbols("parse", 8);
        let parse_top = parse_hits
            .iter()
            .find(|h| {
                h.name.eq_ignore_ascii_case("parse") || h.name.eq_ignore_ascii_case("safeParse")
            })
            .expect("parse-family hit");
        assert!(
            parse_top
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("packages/schema/src/core/parse.ts"),
            "parse must not pick bench parseSimpleObject first, top={parse_top:?}"
        );
    }

    #[test]
    fn search_prefers_core_over_v3_safeparse_twin() {
        let graph = NeuralProjectGraph::new(ProjectId::new("schema"));
        let core = r#"
export function safeParse(schema: object, data: unknown) {
  return { success: true, data };
}
"#;
        let v3 = r#"
export function safeParse(schema: object, data: unknown) {
  return { success: true, data };
}
"#;
        graph.ingest_file(
            &indexed_lang(
                "packages/schema/src/core/parse.ts",
                SourceLanguage::TypeScript,
            ),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("packages/schema/src/core/parse.ts"),
                core,
                SourceLanguage::TypeScript,
            ),
            Some(core),
        );
        graph.ingest_file(
            &indexed_lang(
                "packages/schema/src/v3/types.ts",
                SourceLanguage::TypeScript,
            ),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("packages/schema/src/v3/types.ts"),
                v3,
                SourceLanguage::TypeScript,
            ),
            Some(v3),
        );
        graph.finalize_links();

        let hits = graph.search_symbols("safeParse", 5);
        assert!(!hits.is_empty(), "safeParse must be indexed, hits={hits:?}");
        let top_path = hits[0].file_path.to_string_lossy().replace('\\', "/");
        assert!(
            top_path.ends_with("packages/schema/src/core/parse.ts"),
            "safeParse must rank core/parse.ts over v3/types.ts, top={top_path} hits={hits:?}"
        );
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

    #[test]
    fn exact_class_name_outranks_http_token_noise() {
        let graph = NeuralProjectGraph::new(ProjectId::new("symfony"));
        for i in 0..40 {
            let src = format!(
                "<?php\nclass HttpUtils{i} {{\n    public function getKernel() {{}}\n    public function doSendHttp() {{}}\n}}\n"
            );
            let path = format!("src/HttpUtils{i}.php");
            graph.ingest_file(
                &indexed(&path),
                &CodeIntelligenceEngine::analyze(&PathBuf::from(&path), &src, SourceLanguage::PHP),
                Some(&src),
            );
        }
        let kernel = "<?php\nclass HttpKernel {\n    public function handle($request) {}\n}\n";
        graph.ingest_file(
            &indexed("src/HttpKernel.php"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/HttpKernel.php"),
                kernel,
                SourceLanguage::PHP,
            ),
            Some(kernel),
        );
        graph.finalize_links();
        let hits = graph.search_symbols("HttpKernel", 20);
        assert_eq!(
            hits.first().map(|h| h.name.as_str()),
            Some("HttpKernel"),
            "exact class must be rank 1, got {hits:?}"
        );
        assert_eq!(hits[0].match_reason, "exact_name");
    }

    #[test]
    fn php_throw_inbound_recall_holds_with_many_callers() {
        let graph = NeuralProjectGraph::new(ProjectId::new("routing"));
        let exception = "<?php\nclass RouteNotFoundException extends \\RuntimeException {}\n";
        graph.ingest_file(
            &indexed("src/RouteNotFoundException.php"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/RouteNotFoundException.php"),
                exception,
                SourceLanguage::PHP,
            ),
            Some(exception),
        );
        let sites = [
            ("src/CompiledUrlGeneratorDumper.php", "dump"),
            ("src/CompiledUrlGenerator.php", "generate"),
            ("src/UrlGenerator.php", "doGenerate"),
            ("src/CompiledUrlMatcherDumper.php", "dumpMatcher"),
            ("src/RedirectableUrlMatcher.php", "redirect"),
        ];
        for (path, fn_name) in sites {
            let src = format!(
                "<?php\nclass Site {{\n    public function {fn_name}() {{\n        throw new RouteNotFoundException('x');\n    }}\n}}\n"
            );
            graph.ingest_file(
                &indexed(path),
                &CodeIntelligenceEngine::analyze(&PathBuf::from(path), &src, SourceLanguage::PHP),
                Some(&src),
            );
        }
        graph.finalize_links();
        let trace = graph.trace_symbol("RouteNotFoundException", crate::TraceDirection::Inbound, 3);
        let callers: Vec<_> = trace.callers.iter().map(|h| h.name.as_str()).collect();
        for expected in ["dump", "generate", "doGenerate", "dumpMatcher", "redirect"] {
            assert!(
                callers.contains(&expected),
                "missing throw site {expected}, got {callers:?}"
            );
        }
    }

    #[test]
    fn php_matcher_rethrow_is_inbound_to_resource_not_route() {
        let graph = NeuralProjectGraph::new(ProjectId::new("routing"));
        let route_ex = "<?php\nclass RouteNotFoundException extends \\RuntimeException {}\n";
        let resource_ex = "<?php\nclass ResourceNotFoundException extends \\RuntimeException {}\n";
        let method_ex = "<?php\nclass MethodNotAllowedException extends \\RuntimeException {}\n";
        let generator = r#"
<?php
class UrlGenerator {
    public function generate(string $name): string {
        throw new RouteNotFoundException('missing');
    }
}
"#;
        let matcher = r#"
<?php
class RedirectableUrlMatcher {
    public function match(string $pathinfo): array {
        try {
            return parent::match($pathinfo);
        } catch (ResourceNotFoundException $e) {
            throw $e;
        }
    }
}
"#;
        let dumper = r#"
<?php
class CompiledUrlMatcherDumper {
    public function dump(): string {
        throw 0 < $this->allow
            ? new MethodNotAllowedException()
            : new ResourceNotFoundException('no routes');
    }
}
"#;
        for (path, src) in [
            ("src/RouteNotFoundException.php", route_ex),
            ("src/ResourceNotFoundException.php", resource_ex),
            ("src/MethodNotAllowedException.php", method_ex),
            ("src/UrlGenerator.php", generator),
            ("src/RedirectableUrlMatcher.php", matcher),
            ("src/CompiledUrlMatcherDumper.php", dumper),
        ] {
            graph.ingest_file(
                &indexed(path),
                &CodeIntelligenceEngine::analyze(&PathBuf::from(path), src, SourceLanguage::PHP),
                Some(src),
            );
        }
        graph.finalize_links();

        let route_in =
            graph.trace_symbol("RouteNotFoundException", crate::TraceDirection::Inbound, 3);
        let route_files: Vec<_> = route_in
            .callers
            .iter()
            .map(|h| h.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(
            route_files.iter().any(|p| p.ends_with("UrlGenerator.php")),
            "generator throw site missing, callers {route_files:?}"
        );
        assert!(
            !route_files
                .iter()
                .any(|p| p.contains("RedirectableUrlMatcher")
                    || p.contains("CompiledUrlMatcherDumper")),
            "matcher files must not be inbound to RouteNotFoundException, got {route_files:?}"
        );

        let resource_in = graph.trace_symbol(
            "ResourceNotFoundException",
            crate::TraceDirection::Inbound,
            3,
        );
        let resource_files: Vec<_> = resource_in
            .callers
            .iter()
            .map(|h| h.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(
            resource_files
                .iter()
                .any(|p| p.ends_with("RedirectableUrlMatcher.php")),
            "rethrow site missing, callers {resource_files:?}"
        );
        assert!(
            resource_files
                .iter()
                .any(|p| p.ends_with("CompiledUrlMatcherDumper.php")),
            "ternary throw site missing, callers {resource_files:?}"
        );
    }

    #[test]
    fn kotlin_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("android"));
        let store = r#"
package com.example.app
object SmsStore {
    fun save(body: String?) {
        persist(body)
    }
    private fun persist(body: String?) {}
}
"#;
        let inbox = r#"
package com.example.app
object InboxStore {
    fun save(body: String?) {}
}
"#;
        let receiver = r#"
package com.example.app
import android.content.BroadcastReceiver
import com.example.app.SmsStore
class SmsReceiver : BroadcastReceiver() {
    fun onReceive(intent: Intent) {
        SmsStore.save(intent.getStringExtra("sms"))
    }
}
"#;
        graph.ingest_file(
            &indexed("app/src/main/java/com/example/app/SmsStore.kt"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("app/src/main/java/com/example/app/SmsStore.kt"),
                store,
                SourceLanguage::Kotlin,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("app/src/main/java/com/example/app/InboxStore.kt"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("app/src/main/java/com/example/app/InboxStore.kt"),
                inbox,
                SourceLanguage::Kotlin,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("app/src/main/java/com/example/app/SmsReceiver.kt"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("app/src/main/java/com/example/app/SmsReceiver.kt"),
                receiver,
                SourceLanguage::Kotlin,
            ),
            Some(receiver),
        );
        graph.finalize_links();

        let stats = graph.stats();
        assert!(
            stats.total_edges < 30,
            "edges exploded: {}",
            stats.total_edges
        );
        assert!(stats.resolved_calls >= 1);

        let on_receive = graph
            .resolve_unique(
                "onReceive",
                Some("app/src/main/java/com/example/app/SmsReceiver.kt"),
            )
            .expect("onReceive");
        let neighbors = graph.get_neighbor_views(&on_receive);
        let save_hits: Vec<_> = neighbors
            .iter()
            .filter(|n| {
                n.node.name == "save" && n.edge.edge_type == neuromesh_core::EdgeType::Calls
            })
            .collect();
        assert_eq!(
            save_hits.len(),
            1,
            "same-name save must not fan out: {:?}",
            neighbors
                .iter()
                .map(|n| format!("{}:{}", n.node.name, n.node.file_path.display()))
                .collect::<Vec<_>>()
        );
        assert!(
            save_hits[0]
                .node
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("SmsStore.kt"),
            "SmsStore.save should win, got {}",
            save_hits[0].node.file_path.display()
        );
    }

    fn assert_unique_call(
        graph: &NeuralProjectGraph,
        caller: &str,
        caller_file: &str,
        callee: &str,
        expected_suffix: &str,
        max_edges: usize,
    ) {
        let stats = graph.stats();
        assert!(
            stats.total_edges < max_edges,
            "edges exploded: {}",
            stats.total_edges
        );
        assert!(stats.resolved_calls >= 1);
        let node = graph
            .resolve_unique(caller, Some(caller_file))
            .unwrap_or_else(|| panic!("{caller}"));
        let neighbors = graph.get_neighbor_views(&node);
        let hits: Vec<_> = neighbors
            .iter()
            .filter(|n| {
                n.node.name == callee && n.edge.edge_type == neuromesh_core::EdgeType::Calls
            })
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "same-name {callee} must not fan out: {:?}",
            neighbors
                .iter()
                .map(|n| format!("{}:{}", n.node.name, n.node.file_path.display()))
                .collect::<Vec<_>>()
        );
        let path = hits[0].node.file_path.to_string_lossy().replace('\\', "/");
        assert!(
            path.ends_with(expected_suffix),
            "{expected_suffix} should win, got {path}"
        );
    }

    #[test]
    fn python_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("py"));
        let store = "class SmsStore:\n    def save(self, body):\n        self.persist(body)\n    def persist(self, body):\n        return body\n";
        let inbox = "class InboxStore:\n    def save(self, body):\n        return body\n";
        let receiver =
            "from sms_store import SmsStore\ndef on_receive(body):\n    SmsStore.save(body)\n";
        graph.ingest_file(
            &indexed("sms_store.py"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("sms_store.py"),
                store,
                SourceLanguage::Python,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("inbox_store.py"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("inbox_store.py"),
                inbox,
                SourceLanguage::Python,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("receiver.py"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("receiver.py"),
                receiver,
                SourceLanguage::Python,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "on_receive",
            "receiver.py",
            "save",
            "sms_store.py",
            30,
        );
    }

    #[test]
    fn go_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("go"));
        let store = "package smsstore\nfunc Save(body string) {}\n";
        let inbox = "package inboxstore\nfunc Save(body string) {}\n";
        let receiver = "package receiver\nimport \"example.com/app/smsstore\"\nfunc OnReceive(body string) { smsstore.Save(body) }\n";
        graph.ingest_file(
            &indexed("smsstore/store.go"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("smsstore/store.go"),
                store,
                SourceLanguage::Go,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("inboxstore/store.go"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("inboxstore/store.go"),
                inbox,
                SourceLanguage::Go,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("receiver/handler.go"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("receiver/handler.go"),
                receiver,
                SourceLanguage::Go,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "OnReceive",
            "receiver/handler.go",
            "Save",
            "smsstore/store.go",
            30,
        );
    }

    #[test]
    fn java_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("java"));
        let store = "package com.example.app;\nclass SmsStore {\n    static void save(String body) { persist(body); }\n    static void persist(String body) {}\n}\n";
        let inbox = "package com.example.app;\nclass InboxStore {\n    static void save(String body) {}\n}\n";
        let receiver = "package com.example.app;\nimport com.example.app.SmsStore;\nclass SmsReceiver {\n    void onReceive(String body) { SmsStore.save(body); }\n}\n";
        graph.ingest_file(
            &indexed("com/example/app/SmsStore.java"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("com/example/app/SmsStore.java"),
                store,
                SourceLanguage::Java,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("com/example/app/InboxStore.java"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("com/example/app/InboxStore.java"),
                inbox,
                SourceLanguage::Java,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("com/example/app/SmsReceiver.java"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("com/example/app/SmsReceiver.java"),
                receiver,
                SourceLanguage::Java,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "onReceive",
            "com/example/app/SmsReceiver.java",
            "save",
            "SmsStore.java",
            40,
        );
    }

    #[test]
    fn dart_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("dart"));
        let store = "class SmsStore {\n  void save(String body) { persist(body); }\n  void persist(String body) {}\n}\n";
        let inbox = "class InboxStore {\n  void save(String body) {}\n}\n";
        let receiver =
            "import 'sms_store.dart';\nvoid onReceive(String body) { SmsStore.save(body); }\n";
        graph.ingest_file(
            &indexed("lib/sms_store.dart"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("lib/sms_store.dart"),
                store,
                SourceLanguage::Dart,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("lib/inbox_store.dart"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("lib/inbox_store.dart"),
                inbox,
                SourceLanguage::Dart,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("lib/receiver.dart"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("lib/receiver.dart"),
                receiver,
                SourceLanguage::Dart,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "onReceive",
            "lib/receiver.dart",
            "save",
            "sms_store.dart",
            40,
        );
    }

    #[test]
    fn csharp_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("cs"));
        let store = "namespace App { class SmsStore { public static void Save(string body) { Persist(body); } static void Persist(string body) {} } }";
        let inbox =
            "namespace App { class InboxStore { public static void Save(string body) {} } }";
        let receiver = "namespace App { using App; class SmsReceiver { public void OnReceive(string body) { SmsStore.Save(body); } } }";
        graph.ingest_file(
            &indexed("SmsStore.cs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("SmsStore.cs"),
                store,
                SourceLanguage::CSharp,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("InboxStore.cs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("InboxStore.cs"),
                inbox,
                SourceLanguage::CSharp,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("SmsReceiver.cs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("SmsReceiver.cs"),
                receiver,
                SourceLanguage::CSharp,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "OnReceive",
            "SmsReceiver.cs",
            "Save",
            "SmsStore.cs",
            40,
        );
    }

    #[test]
    fn swift_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("swift"));
        let store = "class SmsStore {\n  func save(body: String?) { persist(body: body) }\n  private func persist(body: String?) {}\n}\n";
        let inbox = "class InboxStore {\n  func save(body: String?) {}\n}\n";
        let receiver = "class SmsReceiver {\n  func onReceive(body: String?) { SmsStore.save(body: body) }\n}\n";
        graph.ingest_file(
            &indexed("SmsStore.swift"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("SmsStore.swift"),
                store,
                SourceLanguage::Swift,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("InboxStore.swift"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("InboxStore.swift"),
                inbox,
                SourceLanguage::Swift,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("SmsReceiver.swift"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("SmsReceiver.swift"),
                receiver,
                SourceLanguage::Swift,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "onReceive",
            "SmsReceiver.swift",
            "save",
            "SmsStore.swift",
            40,
        );
    }

    #[test]
    fn ruby_unique_save_does_not_explode_or_cross_link() {
        let graph = NeuralProjectGraph::new(ProjectId::new("ruby"));
        let store = "class SmsStore\n  def self.save(body)\n    persist(body)\n  end\n  def self.persist(body)\n  end\nend\n";
        let inbox = "class InboxStore\n  def self.save(body)\n  end\nend\n";
        let receiver = "require_relative 'sms_store'\nclass SmsReceiver\n  def on_receive(body)\n    SmsStore.save(body)\n  end\nend\n";
        graph.ingest_file(
            &indexed("sms_store.rb"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("sms_store.rb"),
                store,
                SourceLanguage::Ruby,
            ),
            Some(store),
        );
        graph.ingest_file(
            &indexed("inbox_store.rb"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("inbox_store.rb"),
                inbox,
                SourceLanguage::Ruby,
            ),
            Some(inbox),
        );
        graph.ingest_file(
            &indexed("receiver.rb"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("receiver.rb"),
                receiver,
                SourceLanguage::Ruby,
            ),
            Some(receiver),
        );
        graph.finalize_links();
        assert_unique_call(
            &graph,
            "on_receive",
            "receiver.rb",
            "save",
            "sms_store.rb",
            40,
        );
    }

    #[test]
    fn stylesheet_import_does_not_explode_class_namesakes() {
        let graph = NeuralProjectGraph::new(ProjectId::new("css"));
        let tokens = ".card { color: red; }\n";
        let sms = "@import \"tokens.css\";\n.smsBadge { color: blue; }\n.card { padding: 1rem; }\n";
        graph.ingest_file(
            &indexed("styles/tokens.css"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("styles/tokens.css"),
                tokens,
                SourceLanguage::CSS,
            ),
            Some(tokens),
        );
        graph.ingest_file(
            &indexed("styles/sms.css"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("styles/sms.css"),
                sms,
                SourceLanguage::CSS,
            ),
            Some(sms),
        );
        graph.finalize_links();
        let stats = graph.stats();
        assert!(
            stats.total_edges < 20,
            "edges exploded: {}",
            stats.total_edges
        );
        assert!(
            stats.resolved_imports >= 1,
            "expected unique @import to tokens.css"
        );
        let card_hits = graph.search_symbols("card", 10);
        assert!(
            card_hits.len() >= 2,
            "both card class definitions should stay searchable, got {card_hits:?}"
        );
    }

    #[test]
    fn java_inner_class_write_nodes_are_distinct() {
        let graph = NeuralProjectGraph::new(ProjectId::new("gson"));
        let src = r#"
package com.google.gson;
public class TypeAdapter<T> {
    public void write(JsonWriter out, T value) throws IOException {
        out.value(String.valueOf(value));
    }
    private final class NullSafeTypeAdapter extends TypeAdapter<T> {
        public void write(JsonWriter out, T value) throws IOException {
            if (value != null) {
                out.nullValue();
            }
        }
    }
}
"#;
        let mut file = indexed("gson/TypeAdapter.java");
        file.language = SourceLanguage::Java;
        graph.ingest_file(
            &file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("TypeAdapter.java"),
                src,
                SourceLanguage::Java,
            ),
            Some(src),
        );
        graph.finalize_links();
        let writes: Vec<_> = graph
            .find_nodes_by_name("write")
            .into_iter()
            .filter(|n| n.node_type == NodeType::Function && n.name == "write")
            .collect();
        assert_eq!(
            writes.len(),
            2,
            "inner and outer write must both exist: {:?}",
            writes
                .iter()
                .map(|n| (n.id.as_str().to_string(), n.parent.clone()))
                .collect::<Vec<_>>()
        );
        assert_ne!(writes[0].id, writes[1].id);
        let parents: Vec<Option<String>> = writes.iter().map(|n| n.parent.clone()).collect();
        assert!(
            parents.iter().any(|p| p.as_deref() == Some("TypeAdapter")),
            "outer write parent: {parents:?}"
        );
        assert!(
            parents
                .iter()
                .any(|p| p.as_deref() == Some("NullSafeTypeAdapter")),
            "inner write parent: {parents:?}"
        );
        let hits = graph.search_symbols("write", 10);
        assert!(
            hits.iter().filter(|h| h.name == "write").count() >= 2,
            "search must return both write spans: {hits:?}"
        );
    }

    fn ingest_lang(graph: &NeuralProjectGraph, rel: &str, src: &str, language: SourceLanguage) {
        graph.ingest_file(
            &indexed_lang(rel, language),
            &CodeIntelligenceEngine::analyze(&PathBuf::from(rel), src, language),
            Some(src),
        );
    }

    #[test]
    fn pinoox_view_render_calls_twig_file_not_hello_symbol() {
        let graph = NeuralProjectGraph::new(ProjectId::new("pinoox"));
        ingest_lang(
            &graph,
            "Controller/MainController.php",
            r#"<?php
class MainController extends Controller {
    public function index() {
        return View::render('hello', ['title' => 'Hi']);
    }
}
"#,
            SourceLanguage::PHP,
        );
        ingest_lang(
            &graph,
            "Helper/Greeter.php",
            r#"<?php
class Greeter {
    public function hello() {
        return 'hi';
    }
}
"#,
            SourceLanguage::PHP,
        );
        ingest_lang(
            &graph,
            "theme/default/hello.twig",
            "<html><title>{{ title }}</title><p>{{ message }}</p></html>\n",
            SourceLanguage::Twig,
        );
        ingest_lang(
            &graph,
            "theme/default/layout.twig",
            "<html>{% block body %}{% endblock %}</html>\n",
            SourceLanguage::Twig,
        );
        ingest_lang(
            &graph,
            "routes/web.php",
            "<?php action([MainController::class, 'index'])->name('home');\n",
            SourceLanguage::PHP,
        );
        graph.finalize_links();

        let index = graph
            .resolve_unique("index", Some("Controller/MainController.php"))
            .expect("index method");
        let qualified = graph
            .resolve_best("MainController::index")
            .expect("Type::method resolves to the index method");
        assert_eq!(qualified.id, index);
        let hello_hits = graph.search_symbols("hello", 5);
        assert!(
            hello_hits.iter().any(|h| {
                h.file_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with("theme/default/hello.twig")
            }),
            "hello.twig must be searchable, got {:?}",
            hello_hits
                .iter()
                .map(|h| format!("{}:{}", h.name, h.file_path.display()))
                .collect::<Vec<_>>()
        );
        let neighbors = graph.get_neighbor_views(&index);
        assert!(
            neighbors.iter().any(|n| {
                n.edge.edge_type == neuromesh_core::EdgeType::Calls
                    && n.node
                        .file_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("theme/default/hello.twig")
            }),
            "index must Call the Twig file, neighbors={:?}",
            neighbors
                .iter()
                .map(|n| format!(
                    "{:?}:{}:{}",
                    n.edge.edge_type,
                    n.node.name,
                    n.node.file_path.to_string_lossy()
                ))
                .collect::<Vec<_>>()
        );
        let hint = graph
            .resolve_file_hint("theme/default/hello.twig")
            .expect("unique twig file hint");
        assert!(graph.get_node(&hint).is_some_and(|n| n
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("theme/default/hello.twig")));
        let trace = graph.trace_symbol("MainController::index", crate::TraceDirection::Both, 3);
        assert!(
            trace.callees.iter().any(|h| {
                h.file_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .contains("hello.twig")
            }) || trace.hops.iter().any(|h| {
                h.to.file_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .contains("hello.twig")
            }),
            "trace(MainController::index) must reach hello.twig, callees={:?} hops={:?}",
            trace
                .callees
                .iter()
                .map(|h| h.file_path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            trace
                .hops
                .iter()
                .map(|h| h.to.file_path.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn learning_weights_persist_in_graph_snapshot() {
        let dir = std::env::temp_dir().join(format!("neuromesh-learn-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("graph.bin");
        let graph = NeuralProjectGraph::new(neuromesh_core::ProjectId::new("persist"));
        let file = IndexedFile {
            project_id: neuromesh_core::ProjectId::new("persist"),
            relative_path: std::path::PathBuf::from("src/a.rs"),
            full_path: dir.join("src/a.rs"),
            blake3_hash: "h1".into(),
            byte_size: 10,
            token_count: 10,
            language: neuromesh_index::SourceLanguage::Rust,
            last_modified: chrono::Utc::now(),
        };
        let ast = neuromesh_parser::CodeIntelligenceEngine::analyze(
            &file.relative_path,
            "pub fn touched() {}",
            file.language,
        );
        graph.ingest_file(&file, &ast, Some("pub fn touched() {}"));
        graph.finalize_links();
        let node = graph.resolve_best("touched").expect("node");
        graph.reinforce_node_access(&node.id, true);
        let before = graph
            .node_learning_profile("touched")
            .expect("profile")
            .access_count;
        assert!(before > 0);
        graph.save_to(&path).expect("save");
        let reloaded = NeuralProjectGraph::new(neuromesh_core::ProjectId::new("persist"));
        assert!(reloaded.load_from(&path).expect("load"));
        let after = reloaded
            .node_learning_profile("touched")
            .expect("profile")
            .access_count;
        assert_eq!(
            after, before,
            "learning weights must survive snapshot round-trip"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parser_epoch_mismatch_clears_file_hashes_for_relink() {
        let dir = std::env::temp_dir().join(format!("neuromesh-epoch-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("graph.bin");
        let snapshot = GraphSnapshot {
            version: 2,
            nodes: Vec::new(),
            edges: Vec::new(),
            pending: Vec::new(),
            unresolved: Vec::new(),
            file_hashes: [("src/App.vue".into(), "old-hash".into())]
                .into_iter()
                .collect(),
            file_fingerprints: HashMap::new(),
            export_index: HashMap::new(),
            generation: 1,
            indexed_at: None,
            stale_files: Vec::new(),
            workspace_root: Some(dir.clone()),
            parser_epoch: 0,
            applied_learning_episodes: HashSet::new(),
        };
        let bytes = bincode::serialize(&snapshot).expect("serialize");
        std::fs::write(&path, bytes).expect("write");
        let graph = NeuralProjectGraph::new(ProjectId::new("epoch"));
        assert!(graph.load_from(&path).expect("load"));
        assert!(graph.needs_parser_relink());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mini_shop_vue_trace_finds_template_callers() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mini-shop");
        if !root.join("src/stores/ui.js").exists() {
            return;
        }
        let graph = NeuralProjectGraph::new(ProjectId::new("mini-shop-trace"));
        for (rel, lang) in [
            ("src/stores/ui.js", SourceLanguage::JavaScript),
            ("src/stores/cart.js", SourceLanguage::JavaScript),
            ("src/components/Header.vue", SourceLanguage::Vue),
            ("src/components/CartDrawer.vue", SourceLanguage::Vue),
        ] {
            let path = root.join(rel);
            let src = std::fs::read_to_string(&path).expect("read fixture");
            ingest_fixture(&graph, rel, &src, lang);
        }
        graph.finalize_links();
        let go_checkout = graph.trace_symbol("goCheckout", crate::TraceDirection::Inbound, 2);
        assert!(
            go_checkout.callers.iter().any(|h| h.name == "CartDrawer"),
            "goCheckout callers={:?}",
            go_checkout.callers
        );
        let go_cart = graph.trace_symbol("goCart", crate::TraceDirection::Inbound, 2);
        assert!(
            go_cart.callers.iter().any(|h| h.name == "Header"),
            "goCart callers={:?}",
            go_cart.callers
        );
    }

    #[test]
    fn learning_episode_checkpoint_round_trips_in_snapshot() {
        let dir = std::env::temp_dir().join(format!("neuromesh-ep-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("graph.bin");
        let graph = NeuralProjectGraph::new(ProjectId::new("ep"));
        graph.mark_learning_episode_applied("episode-abc");
        graph.set_workspace(&dir);
        graph.save_to(&path).expect("save");
        let reloaded = NeuralProjectGraph::new(ProjectId::new("ep"));
        assert!(reloaded.load_from(&path).expect("load"));
        assert!(reloaded.learning_episode_applied("episode-abc"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn vue_template_callers_trace_to_store_action() {
        let graph = NeuralProjectGraph::new(neuromesh_core::ProjectId::new("vue-trace"));
        let ui_src = r#"import { defineStore } from 'pinia'
export const useUiStore = defineStore('ui', {
  actions: { goCheckout() {} },
})"#;
        let drawer_src = r#"<script setup>
import { useUiStore } from '../stores/ui'
const ui = useUiStore()
</script>
<template>
  <button @click="ui.goCheckout()">Checkout</button>
</template>"#;
        ingest_fixture(
            &graph,
            "src/stores/ui.js",
            ui_src,
            neuromesh_index::SourceLanguage::JavaScript,
        );
        ingest_fixture(
            &graph,
            "src/components/CartDrawer.vue",
            drawer_src,
            neuromesh_index::SourceLanguage::Vue,
        );
        graph.finalize_links();
        let trace = graph.trace_symbol("goCheckout", crate::TraceDirection::Inbound, 2);
        assert!(trace.origin_reliable, "goCheckout must resolve exactly");
        assert!(
            trace.callers.iter().any(|h| h.name == "CartDrawer"),
            "expected CartDrawer caller, callers={:?}",
            trace.callers
        );
    }

    #[test]
    fn trace_marks_fuzzy_symbol_origin_unreliable() {
        let graph = NeuralProjectGraph::new(neuromesh_core::ProjectId::new("fuzzy"));
        ingest_fixture(
            &graph,
            "src/views/CartView.vue",
            "<template><div /></template>",
            neuromesh_index::SourceLanguage::Vue,
        );
        graph.finalize_links();
        let trace = graph.trace_symbol("goCart", crate::TraceDirection::Inbound, 2);
        if let Some(origin) = &trace.origin {
            assert!(
                !trace.origin_reliable,
                "missing symbols must not look like exact trace hits"
            );
            assert_eq!(origin.match_reason, "fuzzy");
        }
    }

    fn ingest_fixture(
        graph: &NeuralProjectGraph,
        rel: &str,
        src: &str,
        language: neuromesh_index::SourceLanguage,
    ) {
        let file = IndexedFile {
            project_id: neuromesh_core::ProjectId::new("fixture"),
            relative_path: std::path::PathBuf::from(rel),
            full_path: std::path::PathBuf::from(rel),
            blake3_hash: rel.to_string(),
            byte_size: src.len() as u64,
            token_count: 40,
            language,
            last_modified: chrono::Utc::now(),
        };
        let ast =
            neuromesh_parser::CodeIntelligenceEngine::analyze(&file.relative_path, src, language);
        graph.ingest_file(&file, &ast, Some(src));
    }
}
