use neuromesh_core::Result;
use neuromesh_memory::MemoryDatabase;

pub fn execute() -> Result<()> {
    let current_dir = neuromesh_index::assert_safe_workspace(&std::env::current_dir()?)?;
    let project_id = neuromesh_core::stable_project_id(&current_dir);
    let db_path = neuromesh_core::memory_db_path(&current_dir);

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
        println!(
            "{:<15} {:<20} {:<40} {:<10}",
            "Category", "Key", "Content", "Confidence"
        );
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
        println!(
            "{:<15} {:<35} {:<10} {:<12}",
            "Intent", "Summary", "Success", "Tokens Saved"
        );
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
