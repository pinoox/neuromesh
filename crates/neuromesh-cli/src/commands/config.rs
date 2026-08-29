use neuromesh_core::{Config, NeuroMeshError, Result, SeedEngineId};

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
        Some(other) if SeedEngineId::parse(other).is_some() => {
            handle_seed_engine(Some(other), global_flag(args))
        }
        Some(other) => Err(NeuroMeshError::Config(format!(
            "unknown config command: {other} (try: config seed-engine, config show)"
        ))),
    }
}

fn global_flag(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--global" || a == "-g" || a == "global")
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
    println!("  NEUROMESH_SEED_ENGINE=<engine>                  one-shot env override");
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
Usage: neuromesh config [show|seed-engine]

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
