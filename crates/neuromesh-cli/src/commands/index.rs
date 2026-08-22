use neuromesh_core::{ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_memory::{MemoryDatabase, ProjectFact};
use neuromesh_parser::CodeIntelligenceEngine;
use std::fs;
use std::sync::Arc;

pub fn execute() -> Result<(Arc<NeuralProjectGraph>, Arc<MemoryDatabase>)> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("neuromesh-project")
        .to_string();

    let project_id = ProjectId::new(&project_name);
    let walker = ProjectWalker::new(current_dir.clone(), project_id.clone());

    println!("🔍 Indexing Project Workspace...");
    let scanned_files = walker.scan()?;
    let total_files = scanned_files.len();

    let graph = Arc::new(NeuralProjectGraph::new(project_id.clone()));
    let _ = graph.load_persisted(&current_dir);
    let db_dir = current_dir.join(".neuromesh");
    fs::create_dir_all(&db_dir)?;
    let memory_db = Arc::new(MemoryDatabase::open(&db_dir.join("neuromesh.json"))?);

    let mut total_tokens = 0;
    let mut has_vue = false;
    let mut has_scss = false;
    let mut has_ts = false;
    let mut has_html = false;

    for (file, content) in &scanned_files {
        total_tokens += file.token_count;
        let ast = CodeIntelligenceEngine::analyze(&file.relative_path, content, file.language);
        graph.ingest_file(file, &ast, Some(content));

        let path_str = file.relative_path.to_string_lossy().to_lowercase();
        if path_str.ends_with(".html") || path_str.ends_with(".htm") {
            has_html = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Vue {
            has_vue = true;
        }
        if file.language == neuromesh_index::SourceLanguage::SCSS || path_str.ends_with(".css") {
            has_scss = true;
        }
        if file.language == neuromesh_index::SourceLanguage::TypeScript {
            has_ts = true;
        }
    }
    graph.finalize_links();
    let present: std::collections::HashSet<String> = scanned_files
        .iter()
        .map(|(file, _)| file.relative_path.to_string_lossy().replace('\\', "/"))
        .collect();
    graph.prune_absent_files(&present);
    graph.save_persisted(&current_dir)?;

    if has_html {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "structure",
            "markup_language",
            "HTML5 semantic layout with RTL Persian typography and responsive styling",
        ))?;
    }
    if has_vue {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "framework",
            "frontend_framework",
            "Vue 3 (Composition API, Single File Components)",
        ))?;
    }
    if has_scss {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "styling",
            "design_tokens",
            "CSS variables (--saffron, --paper, --ink) with responsive breakpoints",
        ))?;
    }
    if has_ts {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "type_system",
            "TypeScript strict mode with interface definitions",
        ))?;
    }

    for fact in neuromesh_memory::extract_project_facts(&current_dir, &project_id) {
        memory_db.save_project_fact(&fact)?;
    }

    let stats = graph.stats();
    println!("✓ Indexing Complete");
    println!("  Indexed Files  : {}", total_files);
    println!("  Total Tokens   : {}", total_tokens);
    println!("  Graph Nodes    : {}", stats.total_nodes);
    println!("  Pheromone Edges: {}", stats.total_edges);

    Ok((graph, memory_db))
}
