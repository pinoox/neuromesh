use neuromesh_core::{
    Config, EmbeddingModelId, GraphBackendId, NeuroMeshError, Result, SeedEngineId,
};
use neuromesh_graph_proxy::resolve_for_workspace;

pub fn execute(args: &[String]) -> Result<()> {
    let sub = args.get(2).map(String::as_str);
    match sub {
        None | Some("get") | Some("show") | Some("-v") | Some("--show") => print_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
            Ok(())
        }
        Some("seed-engine") | Some("seed_engine") | Some("engine") => {
            handle_seed_engine(args.get(3).map(String::as_str), global_flag(args))
        }
        Some("graph-backend") | Some("graph_backend") | Some("graph") => {
            handle_graph_backend(args.get(3).map(String::as_str), global_flag(args))
        }
        Some("embeddings") | Some("embedding") => {
            handle_embeddings(args.get(3).map(String::as_str), global_flag(args))
        }
        Some(other) if SeedEngineId::parse(other).is_some() => {
            handle_seed_engine(Some(other), global_flag(args))
        }
        Some(other) if GraphBackendId::parse(other).is_some() => {
            handle_graph_backend(Some(other), global_flag(args))
        }
        Some(other) => Err(NeuroMeshError::Config(format!(
            "unknown config command: {other} (try: config seed-engine, config graph-backend, config show)"
        ))),
    }
}

fn global_flag(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--global" || a == "-g" || a == "global")
}

fn handle_graph_backend(value: Option<&str>, global: bool) -> Result<()> {
    match value {
        None | Some("get") | Some("show") => print_graph_backend_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_graph_backend_help();
            Ok(())
        }
        Some(raw) => {
            let backend = parse_graph_backend(raw)?;
            if global {
                set_global_graph_backend(backend)
            } else {
                set_project_graph_backend(backend)
            }
        }
    }
}

fn parse_graph_backend(raw: &str) -> Result<GraphBackendId> {
    GraphBackendId::parse(raw).ok_or_else(|| {
        NeuroMeshError::Config(format!(
            "invalid graph backend: {raw} (expected: {})",
            GraphBackendId::help_line()
        ))
    })
}

fn set_global_graph_backend(backend: GraphBackendId) -> Result<()> {
    let path = Config::set_global_graph_backend(backend)?;
    println!("Global graph backend: {}", backend.as_str());
    println!("Saved               : {}", path.display());
    Ok(())
}

fn set_project_graph_backend(backend: GraphBackendId) -> Result<()> {
    let ws = std::env::current_dir()?;
    let path = Config::set_workspace_graph_backend(&ws, backend)?;
    println!("Project graph backend: {}", backend.as_str());
    println!("Saved                : {}", path.display());
    Ok(())
}

fn print_graph_backend_status() -> Result<()> {
    let ws = std::env::current_dir()?;
    let cfg = Config::load();
    println!(
        "Effective graph backend: {}",
        cfg.graph_backend.backend.as_str()
    );
    if let Some(spec) = resolve_for_workspace(&cfg.graph_backend, &ws) {
        println!(
            "Resolved proxy         : {} ({})",
            spec.provider.as_str(),
            spec.command
        );
        if let Some(path) = &spec.config_path {
            println!("From MCP config        : {}", path.display());
        }
    } else if cfg.graph_backend.backend != GraphBackendId::Native {
        println!("Resolved proxy         : (not found — will use native if fallback_native)");
    }
    if let Ok(raw) = std::env::var("NEUROMESH_GRAPH_BACKEND") {
        println!("Env override           : {raw}");
    }
    Ok(())
}

fn handle_embeddings(value: Option<&str>, global: bool) -> Result<()> {
    match value {
        None | Some("get") | Some("show") => print_embeddings_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_embeddings_help();
            Ok(())
        }
        Some("on") | Some("true") | Some("1") => set_embeddings_enabled(true, global),
        Some("off") | Some("false") | Some("0") => set_embeddings_enabled(false, global),
        Some(raw) if EmbeddingModelId::parse(raw).is_some() => {
            let model = EmbeddingModelId::parse(raw).expect("checked");
            let ws = std::env::current_dir()?;
            let path = Config::set_workspace_embedding_model(&ws, model)?;
            println!("Project embedding model: {}", model.as_str());
            println!("Saved                  : {}", path.display());
            Ok(())
        }
        Some(other) => Err(NeuroMeshError::Config(format!(
            "invalid embeddings config: {other} (use: on, off, minilm_multilingual_q)"
        ))),
    }
}

fn set_embeddings_enabled(enabled: bool, global: bool) -> Result<()> {
    if global {
        let path = Config::set_global_embeddings(enabled)?;
        println!(
            "Global embeddings      : {}",
            if enabled { "on" } else { "off" }
        );
        println!("Saved                  : {}", path.display());
    } else {
        let ws = std::env::current_dir()?;
        let path = Config::set_workspace_embeddings(&ws, enabled)?;
        println!(
            "Project embeddings     : {}",
            if enabled { "on" } else { "off" }
        );
        println!("Saved                  : {}", path.display());
    }
    Ok(())
}

fn print_embeddings_status() -> Result<()> {
    let cfg = Config::load();
    println!(
        "Embeddings enabled     : {}",
        if cfg.embeddings.enabled { "yes" } else { "no" }
    );
    println!("Model                  : {}", cfg.embeddings.model.as_str());
    println!("Matryoshka dim         : {}", cfg.embeddings.matryoshka_dim);
    println!("ANN top-k              : {}", cfg.embeddings.ann_top_k);
    println!("Embed seed cap         : {}", cfg.embeddings.embed_seed_cap);
    println!("Min cosine             : {}", cfg.embeddings.min_cosine);
    println!("Index on build         : {}", cfg.embeddings.index_on_build);
    println!(
        "Semantic cache         : {} ({} entries, min cosine {})",
        if cfg.embeddings.semantic_cache_enabled {
            "on"
        } else {
            "off"
        },
        cfg.embeddings.semantic_cache_entries,
        cfg.embeddings.semantic_cache_min_cosine
    );
    println!(
        "Optional dedup         : {}",
        cfg.embeddings
            .optional_dedup_min_cosine
            .map(|v| v.to_string())
            .unwrap_or_else(|| "off".into())
    );
    println!(
        "Module centroids       : {}",
        if cfg.embeddings.module_cluster_enabled {
            "on"
        } else {
            "off"
        }
    );
    println!(
        "Embed intent (General) : {}",
        if cfg.embeddings.embed_intent_for_general {
            "on"
        } else {
            "off"
        }
    );
    if let Ok(raw) = std::env::var("NEUROMESH_EMBEDDINGS") {
        println!("Env NEUROMESH_EMBEDDINGS: {raw}");
    }
    if let Ok(raw) = std::env::var("NEUROMESH_EMBED_MODEL") {
        println!("Env NEUROMESH_EMBED_MODEL: {raw}");
    }
    #[cfg(not(feature = "embeddings"))]
    println!("Binary feature         : embeddings disabled (rebuild with --features embeddings)");
    #[cfg(feature = "embeddings")]
    println!("Binary feature         : embeddings enabled");
    Ok(())
}

fn print_embeddings_help() {
    println!(
        "\
Usage: neuromesh config embeddings [on|off] [--global]

  neuromesh config embeddings              show effective embedding settings
  neuromesh config embeddings on           enable MiniLM vector recovery (project)
  neuromesh config embeddings off          disable embeddings

Model  : minilm_multilingual_q (only — auto-download on first index)
Env    : NEUROMESH_EMBEDDINGS=1, NEUROMESH_EMBED_THREADS=4
         NEUROMESH_SEMANTIC_CACHE=0, NEUROMESH_OPTIONAL_DEDUP=0.93
Install: curl -fsSL …/install.sh | bash   (or install.ps1 on Windows)
Doctor : neuromesh doctor --embed [--bench]
Build  : cargo build -p neuromesh-cli --features embeddings
"
    );
}

fn handle_seed_engine(value: Option<&str>, global: bool) -> Result<()> {
    match value {
        None | Some("get") | Some("show") => print_seed_engine_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_seed_engine_help();
            Ok(())
        }
        Some(raw) => {
            let engine = parse_engine(raw)?;
            if global {
                set_global(engine)
            } else {
                set_project(engine)
            }
        }
    }
}

fn parse_engine(raw: &str) -> Result<SeedEngineId> {
    SeedEngineId::parse(raw).ok_or_else(|| {
        NeuroMeshError::Config(format!(
            "invalid seed engine: {raw} (expected: {})",
            SeedEngineId::help_line()
        ))
    })
}

fn set_global(engine: SeedEngineId) -> Result<()> {
    let path = Config::set_global_seed_engine(engine)?;
    println!("Global seed engine : {}", engine.as_str());
    println!("Saved              : {}", path.display());
    println!("Applies to         : all workspaces (unless overridden by nm.config.json or NEUROMESH_SEED_ENGINE)");
    Ok(())
}

fn set_project(engine: SeedEngineId) -> Result<()> {
    let ws = std::env::current_dir()?;
    let path = Config::set_workspace_seed_engine(&ws, engine)?;
    println!("Project seed engine: {}", engine.as_str());
    println!("Saved              : {}", path.display());
    println!("Commit nm.config.json to share engine choice with the team.");
    Ok(())
}

fn print_status() -> Result<()> {
    let ws = std::env::current_dir()?;
    let cfg = Config::load();
    println!("\nNeuroMesh config (effective for this workspace)");
    println!(
        "Seed engine        : {}",
        cfg.seed_resolution.engine.as_str()
    );
    println!(
        "Graph backend      : {}",
        cfg.graph_backend.backend.as_str()
    );
    print_seed_engine_layers(&ws)?;
    println!("Monitor port       : {}  ({})", cfg.port, cfg.host);
    println!(
        "Packet header      : {}",
        if cfg.packet_header.enabled {
            "on"
        } else {
            "off"
        }
    );
    println!();
    println!("Manage:");
    println!("  neuromesh config seed-engine <engine>           project (nm.config.json)");
    println!("  neuromesh config seed-engine <engine> --global  ~/.neuromesh/config.json");
    println!(
        "  neuromesh config graph-backend auto              detect CBM/Graphify from MCP configs"
    );
    println!("  NEUROMESH_GRAPH_BACKEND=native|auto|proxy_cbm     one-shot env override");
    println!();
    Ok(())
}

fn print_seed_engine_status() -> Result<()> {
    let ws = std::env::current_dir()?;
    let cfg = Config::load();
    println!(
        "Effective seed engine: {}",
        cfg.seed_resolution.engine.as_str()
    );
    print_seed_engine_layers(&ws)?;
    Ok(())
}

fn print_seed_engine_layers(ws: &std::path::Path) -> Result<()> {
    if let Some(global) = Config::global_seed_engine() {
        println!("  global (~/.neuromesh/config.json): {}", global.as_str());
    }
    if let Some(project) = Config::workspace_seed_engine(ws) {
        println!("  project (nm.config.json)         : {}", project.as_str());
    }
    if let Some(slot) = Config::project_slot_config(ws) {
        println!(
            "  project slot (managed config)    : {}",
            slot.seed_resolution.engine.as_str()
        );
    }
    if let Ok(raw) = std::env::var("NEUROMESH_SEED_ENGINE") {
        println!("  env (NEUROMESH_SEED_ENGINE)      : {raw}");
    }
    Ok(())
}

fn print_help() {
    println!(
        "\
Usage: neuromesh config [show|seed-engine|graph-backend]

Show or change NeuroMesh settings globally and per project.

  neuromesh config                         effective settings for cwd
  neuromesh config seed-engine             show seed engine + layers
  neuromesh config seed-engine hybrid      write nm.config.json in cwd
  neuromesh config seed-engine hybrid -g     write ~/.neuromesh/config.json

Engines: {}
Env     : NEUROMESH_SEED_ENGINE=<engine>

Global defaults live in ~/.neuromesh/config.json.
Project overrides live in nm.config.json (commit-friendly).
Managed project slot config.json can also set seed_resolution when using neuromesh port/index persist.
",
        SeedEngineId::help_line()
    );
}

fn print_graph_backend_help() {
    println!(
        "\
Usage: neuromesh config graph-backend [BACKEND] [--global]

  neuromesh config graph-backend              show effective backend
  neuromesh config graph-backend auto         detect CBM/Graphify when MCP starts
  neuromesh config graph-backend proxy_cbm    always use codebase-memory MCP
  neuromesh config graph-backend native       built-in graph (default)

Backends: {}
Env     : NEUROMESH_GRAPH_BACKEND=<backend>
Doctor  : neuromesh doctor --proxy
",
        GraphBackendId::help_line()
    );
}

fn print_seed_engine_help() {
    println!(
        "\
Usage: neuromesh config seed-engine [ENGINE] [--global]

  neuromesh config seed-engine                  show effective engine
  neuromesh config seed-engine semantic_lite    project override (nm.config.json)
  neuromesh config seed-engine hybrid --global  machine-wide default

Engines: {}
",
        SeedEngineId::help_line()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_engine_names() {
        assert_eq!(
            parse_engine("keywords_expanded").unwrap(),
            SeedEngineId::KeywordsExpanded
        );
        assert!(parse_engine("nope").is_err());
    }
}
