use neuromesh_core::{ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_memory::{MemoryDatabase, ProjectFact};
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
    let report = walker.scan_report()?;
    let skipped_count = report.skipped_count();
    let skipped_summary = report.skipped_summary();
    let scanned_files = report.files;
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
    let mut has_kotlin = false;
    let mut has_svelte = false;
    let mut has_js = false;

    for (file, _) in &scanned_files {
        total_tokens += file.token_count;
        let path_str = file.relative_path.to_string_lossy().to_lowercase();
        if path_str.ends_with(".html") || path_str.ends_with(".htm") || path_str.ends_with(".twig")
        {
            has_html = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Vue {
            has_vue = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Svelte {
            has_svelte = true;
        }
        if file.language == neuromesh_index::SourceLanguage::SCSS || path_str.ends_with(".css") {
            has_scss = true;
        }
        if file.language == neuromesh_index::SourceLanguage::TypeScript {
            has_ts = true;
        }
        if file.language == neuromesh_index::SourceLanguage::JavaScript {
            has_js = true;
        }
        if file.language == neuromesh_index::SourceLanguage::Kotlin {
            has_kotlin = true;
        }
    }
    graph.ingest_workspace(&scanned_files);
    graph.save_persisted(&current_dir)?;

    if has_html {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "structure",
            "markup_language",
            "HTML files are present in the workspace",
        ))?;
    }
    if has_vue {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "framework",
            "frontend_framework",
            "Vue single-file components are present",
        ))?;
    }
    if has_svelte {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "framework",
            "svelte",
            "Svelte components are present",
        ))?;
    }
    if has_js {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "javascript",
            "JavaScript files are present",
        ))?;
    }
    if has_scss {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "styling",
            "stylesheets",
            "SCSS or CSS files are present",
        ))?;
    }
    if has_ts {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "type_system",
            "TypeScript files are present",
        ))?;
    }
    if has_kotlin {
        memory_db.save_project_fact(&ProjectFact::new(
            project_id.clone(),
            "language",
            "kotlin",
            "Kotlin files are present",
        ))?;
    }

    for fact in neuromesh_memory::extract_project_facts(&current_dir, &project_id) {
        memory_db.save_project_fact(&fact)?;
    }

    let stats = graph.stats();
    println!("✓ Indexing Complete");
    println!("  Indexed Files  : {}", total_files);
    if skipped_count > 0 {
        println!("  Skipped        : {} ({})", skipped_count, skipped_summary);
    }
    println!("  Total Tokens   : {}", total_tokens);
    println!("  Graph Nodes    : {}", stats.total_nodes);
    println!("  Pheromone Edges: {}", stats.total_edges);

    Ok((graph, memory_db))
}
