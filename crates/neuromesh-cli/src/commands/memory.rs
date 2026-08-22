use neuromesh_core::{ProjectId, Result};
use neuromesh_memory::MemoryDatabase;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let project_id = ProjectId::new(&project_name);
    let db_path = current_dir.join(".neuromesh").join("neuromesh.json");

    println!("\n🧠 NeuroMesh Persistent Memory");
    println!("===============================================");

    if !db_path.exists() {
        println!("No database initialized yet. Run 'neuromesh init' and 'neuromesh index'.");
        return Ok(());
    }

    let db = MemoryDatabase::open(&db_path)?;
    let facts = db.get_project_facts(&project_id)?;

    println!("\n1. Project Memory (Stable Facts & Conventions):");
    if facts.is_empty() {
        println!("  No project facts recorded yet.");
    } else {
        println!("{:<15} {:<20} {:<40} {:<10}", "Category", "Key", "Content", "Confidence");
        println!("{:-<90}", "");
        for f in facts {
            println!(
                "{:<15} {:<20} {:<40} {:<10.2}",
                f.category,
                f.key,
                f.content.chars().take(38).collect::<String>(),
                f.confidence
            );
        }
    }

    println!("\n2. Episodic Memory (Experience Traces):");
    let episodes = db.find_similar_episodes(&project_id, "")?;
    if episodes.is_empty() {
        println!("  No episodic traces recorded yet.");
    } else {
        println!("{:<15} {:<35} {:<10} {:<12}", "Intent", "Summary", "Success", "Tokens Saved");
        println!("{:-<75}", "");
        for ep in episodes {
            println!(
                "{:<15} {:<35} {:<10} {:<12}",
                ep.intent,
                ep.summary.chars().take(33).collect::<String>(),
                if ep.success { "Yes" } else { "No" },
                ep.tokens_saved
            );
        }
    }
    println!();

    Ok(())
}
