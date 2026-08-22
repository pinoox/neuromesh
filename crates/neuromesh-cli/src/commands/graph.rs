use neuromesh_core::{ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_parser::CodeIntelligenceEngine;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let project_id = ProjectId::new(&project_name);
    let walker = ProjectWalker::new(current_dir.clone(), project_id.clone());
    let scanned = walker.scan().unwrap_or_default();

    let graph = NeuralProjectGraph::new(project_id);
    for (file, content) in &scanned {
        let ast = CodeIntelligenceEngine::analyze(&file.relative_path, content, file.language);
        graph.ingest_ast(file, &ast);
    }
    graph.finalize_links();

    let stats = graph.stats();
    let nodes = graph.get_all_nodes();

    println!("\n🕸 ══════════════════════════════════════════════════════════════");
    println!(
        "   NEURAL PROJECT GRAPH & SYNAPTIC TOPOLOGY — [{}]",
        project_name
    );
    println!("══════════════════════════════════════════════════════════════════\n");
    println!("📊 Graph Density & Synaptic Health:");
    println!("  • Total Graph Nodes         : {}", stats.total_nodes);
    println!("  • File Nodes                : {}", stats.file_nodes);
    println!("  • Symbol / Token Nodes      : {}", stats.symbol_nodes);
    println!("  • Total Synapses (Edges)    : {}", stats.total_edges);
    println!(
        "  • Average Pheromone Weight  : {:.2}",
        stats.average_pheromone_weight
    );
    println!(
        "  • High Conductance Synapses : {} (LTP Potentiated)",
        stats.high_conductance_synapses
    );
    println!(
        "  • Atrophied Synapses        : {} (Pruned by Physarum/LTD)",
        stats.atrophied_synapses
    );
    println!();
    println!(
        "{:<32} {:<14} {:<24} {:<8}",
        "Node ID", "Type", "Name", "Tokens"
    );
    println!("{:-<84}", "");

    for n in nodes.iter().take(15) {
        println!(
            "{:<32} {:<14} {:<24} {:<8}",
            n.id.0.chars().take(30).collect::<String>(),
            format!("{:?}", n.node_type),
            n.name.chars().take(22).collect::<String>(),
            n.token_cost
        );
    }
    if nodes.len() > 15 {
        println!("... and {} more nodes in neural memory", nodes.len() - 15);
    }
    println!();

    Ok(())
}
