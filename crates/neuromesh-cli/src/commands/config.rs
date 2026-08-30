use neuromesh_core::{Config, GraphBackendId, NeuroMeshError, Result, RetrievalEngine};
use neuromesh_graph_proxy::resolve_for_workspace;

pub fn execute(args: &[String]) -> Result<()> {
    let sub = args.get(2).map(String::as_str);
    match sub {
        None | Some("get") | Some("show") | Some("-v") | Some("--show") => print_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
            Ok(())
        }
        Some("retrieval-engine") | Some("retrieval_engine") | Some("engine") => {
            handle_retrieval_engine(args, args.get(3).map(String::as_str), global_flag(args))
        }
        Some("graph-backend") | Some("graph_backend") | Some("graph") => {
            handle_graph_backend(args.get(3).map(String::as_str), global_flag(args))
        }
        Some(other) if RetrievalEngine::parse(other).is_some() => {
            handle_retrieval_engine(args, Some(other), global_flag(args))
        }
        Some(other) if GraphBackendId::parse(other).is_some() => {
            handle_graph_backend(Some(other), global_flag(args))
        }
        Some(other) => Err(NeuroMeshError::Config(format!(
            "unknown config command: {other} (try: config engine, config graph-backend, config show)"
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

fn handle_retrieval_engine(args: &[String], value: Option<&str>, global: bool) -> Result<()> {
    match value {
        None | Some("get") | Some("show") => print_retrieval_engine_status(),
        Some("help") | Some("-h") | Some("--help") => {
            print_retrieval_engine_help();
            Ok(())
        }
        Some(raw) => {
            let engine = parse_retrieval_engine(raw)?;
            crate::commands::install::ensure_minilm_for_engine(args, engine)?;
            if global {
                set_global_retrieval(engine)
            } else {
                set_project_retrieval(engine)
            }
        }
    }
}

fn parse_retrieval_engine(raw: &str) -> Result<RetrievalEngine> {
    RetrievalEngine::parse(raw).ok_or_else(|| {
        NeuroMeshError::Config(format!(
            "invalid retrieval engine: {raw} (expected: {})",
            RetrievalEngine::help_line()
        ))
    })
}

fn set_global_retrieval(engine: RetrievalEngine) -> Result<()> {
    let path = Config::set_global_retrieval_engine(engine)?;
    println!("Global retrieval engine : {}", engine.as_str());
    println!("Saved                   : {}", path.display());
    if engine != RetrievalEngine::Fast {
        println!("Next                    : neuromesh embed rebuild");
    }
    Ok(())
}

fn set_project_retrieval(engine: RetrievalEngine) -> Result<()> {
    let ws = std::env::current_dir()?;
    let path = Config::set_workspace_retrieval_engine(&ws, engine)?;
    println!("Project retrieval engine: {}", engine.as_str());
    println!("Saved                   : {}", path.display());
    println!("Commit nm.config.json to share engine choice with the team.");
    if engine != RetrievalEngine::Fast {
        println!("Next                    : neuromesh embed rebuild");
    }
    Ok(())
}

fn print_retrieval_engine_status() -> Result<()> {
    let ws = std::env::current_dir()?;
    let cfg = Config::load();
    println!(
        "Effective retrieval engine: {}",
        cfg.retrieval.engine.as_str()
    );
    println!(
        "  embeddings (derived)      : {}",
        if cfg.embeddings.enabled { "on" } else { "off" }
    );
    #[cfg(feature = "embeddings")]
    if cfg.retrieval.engine != RetrievalEngine::Fast {
        if neuromesh_embed::bundled_minilm_available() {
            if let Some(dir) = neuromesh_embed::resolve_bundled_minilm_dir() {
                println!(
                    "  embed model               : installed ({})",
                    dir.display()
                );
            } else {
                println!("  embed model               : installed");
            }
        } else {
            println!(
                "  embed model               : not installed ({})",
                neuromesh_embed::install_hint()
            );
        }
    }
    println!(
        "  optimization mode         : {}",
        format!("{:?}", cfg.mode).to_lowercase()
    );
    print_retrieval_engine_layers(&ws)?;
    Ok(())
}

fn print_retrieval_engine_help() {
    println!(
        "\
Usage: neuromesh config engine [ENGINE] [--global]

  neuromesh config engine                  show effective retrieval engine
  neuromesh config engine fast             zero-embed graph + lexical (default)
  neuromesh config engine hybrid           prompts to install MiniLM if missing
  neuromesh config engine hybrid --yes     install MiniLM without prompting
  neuromesh config engine deep             max quality + dedup + centroids

  neuromesh install embed minilm           download MiniLM Q (~250 MB, once)

Engines: {}
Env     : NEUROMESH_ENGINE=<engine>
",
        RetrievalEngine::help_line()
    );
}

fn print_status() -> Result<()> {
    let ws = std::env::current_dir()?;
    let cfg = Config::load();
    println!("\nNeuroMesh config (effective for this workspace)");
    println!(
        "Retrieval engine   : {} (embed={})",
        cfg.retrieval.engine.as_str(),
        if cfg.embeddings.enabled { "on" } else { "off" }
    );
    println!(
        "Graph backend      : {}",
        cfg.graph_backend.backend.as_str()
    );
    print_retrieval_engine_layers(&ws)?;
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
    println!("  neuromesh config engine fast|hybrid|deep       project (nm.config.json)");
    println!("  neuromesh config engine hybrid --global        ~/.neuromesh/config.json");
    println!(
        "  neuromesh config graph-backend auto              detect CBM/Graphify from MCP configs"
    );
    println!("  NEUROMESH_GRAPH_BACKEND=native|auto|proxy_cbm     one-shot env override");
    println!();
    Ok(())
}

fn print_retrieval_engine_layers(ws: &std::path::Path) -> Result<()> {
    if let Some(global) = Config::global_retrieval_engine() {
        println!("  global (~/.neuromesh/config.json): {}", global.as_str());
    }
    if let Some(project) = Config::workspace_retrieval_engine(ws) {
        println!("  project (nm.config.json)         : {}", project.as_str());
    }
    if let Ok(raw) = std::env::var("NEUROMESH_ENGINE") {
        println!("  env (NEUROMESH_ENGINE)           : {raw}");
    }
    Ok(())
}

fn print_help() {
    println!(
        "\
Usage: neuromesh config [show|engine|graph-backend]

Show or change NeuroMesh settings globally and per project.

  neuromesh config                         effective settings for cwd
  neuromesh config engine                  show retrieval engine + layers
  neuromesh config engine hybrid           write nm.config.json in cwd
  neuromesh config engine hybrid -g        write ~/.neuromesh/config.json

Retrieval engines: {}
Env               : NEUROMESH_ENGINE=<engine>

Global defaults live in ~/.neuromesh/config.json.
Project overrides live in nm.config.json (commit-friendly).
",
        RetrievalEngine::help_line()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_retrieval_engine_names() {
        assert_eq!(
            parse_retrieval_engine("hybrid").unwrap(),
            RetrievalEngine::Hybrid
        );
        assert!(parse_retrieval_engine("nope").is_err());
    }
}
