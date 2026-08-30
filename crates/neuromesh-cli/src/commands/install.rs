use neuromesh_core::{NeuroMeshError, Result, RetrievalEngine};
use std::io::{self, IsTerminal, Write};

#[cfg(feature = "embeddings")]
use neuromesh_embed::{
    bundled_minilm_available, install_hint_with_flag, install_model, list_installed,
    parse_model_id, InstallOptions, CATALOG, MINILM_MULTILINGUAL_Q,
};

pub fn execute(args: &[String]) -> Result<()> {
    let sub = args.get(1).map(String::as_str).unwrap_or("help");
    match sub {
        "embed" => execute_embed(&args[2..]),
        "help" | "-h" | "--help" | "" => {
            print_help();
            Ok(())
        }
        other => Err(NeuroMeshError::Config(format!(
            "unknown install target: {other} (try: install embed minilm)"
        ))),
    }
}

fn execute_embed(args: &[String]) -> Result<()> {
    #[cfg(not(feature = "embeddings"))]
    {
        return Err(NeuroMeshError::Config(
            "install embed requires embeddings feature (reinstall release binary)".into(),
        ));
    }
    #[cfg(feature = "embeddings")]
    {
        let sub = args.first().map(String::as_str).unwrap_or("help");
        match sub {
            "list" => print_embed_list(),
            "status" => print_embed_status(),
            "help" | "-h" | "--help" => {
                print_embed_help();
                Ok(())
            }
            id => install_embed_id(id, args),
        }
    }
}

#[cfg(feature = "embeddings")]
fn install_embed_id(id: &str, args: &[String]) -> Result<()> {
    let spec = parse_model_id(id).ok_or_else(|| {
        NeuroMeshError::Config(format!(
            "unknown embed model: {id} (try: install embed list)"
        ))
    })?;
    let quiet = args.iter().any(|a| a == "--quiet" || a == "-q");
    let force = args.iter().any(|a| a == "--force");
    install_model(spec, InstallOptions { quiet, force })
        .map_err(|e| NeuroMeshError::Internal(e.to_string()))?;
    if !quiet {
        println!("OK");
    }
    Ok(())
}

#[cfg(feature = "embeddings")]
fn print_embed_list() -> Result<()> {
    println!("\nAvailable embedding models:\n");
    for spec in CATALOG {
        let installed = list_installed().iter().any(|(s, _)| s.id == spec.id);
        println!(
            "  {:<24} {}{}",
            spec.id,
            spec.label,
            if installed { " [installed]" } else { "" }
        );
        if !spec.aliases.is_empty() {
            println!("    aliases: {}", spec.aliases.join(", "));
        }
    }
    let installed = list_installed();
    if installed.is_empty() {
        println!("\nNo models installed yet.");
        println!("  neuromesh install embed minilm\n");
    } else {
        println!("\nInstalled:");
        for (spec, path) in installed {
            println!("  {} → {}", spec.id, path.display());
        }
    }
    Ok(())
}

#[cfg(feature = "embeddings")]
fn print_embed_status() -> Result<()> {
    if bundled_minilm_available() {
        if let Some(dir) = neuromesh_embed::resolve_bundled_minilm_dir() {
            println!("MiniLM: installed ({})", dir.display());
        } else {
            println!("MiniLM: installed");
        }
    } else {
        println!("MiniLM: not installed");
        println!("  neuromesh install embed minilm");
    }
    Ok(())
}

/// Ensure MiniLM is present before hybrid/deep; interactive or flag-driven install.
#[cfg(feature = "embeddings")]
pub fn ensure_minilm_for_engine(args: &[String], engine: RetrievalEngine) -> Result<()> {
    if engine == RetrievalEngine::Fast || bundled_minilm_available() {
        return Ok(());
    }

    if wants_auto_install(args) {
        return install_default_embed_model();
    }

    if prompts_allowed() {
        eprintln!(
            "\nThe {} engine needs the MiniLM embedding model (~250 MB, one-time download).",
            engine.as_str()
        );
        eprintln!("MiniLM is not installed yet.");
        eprint!("Install now? [Y/n] ");
        io::stderr().flush().ok();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(NeuroMeshError::Io)?;
        if confirms_install(&line) {
            return install_default_embed_model();
        }
        return Err(NeuroMeshError::Config(format!(
            "aborted — {} engine unchanged",
            engine.as_str()
        )));
    }

    Err(NeuroMeshError::Config(install_hint_with_flag(
        engine.as_str(),
    )))
}

#[cfg(feature = "embeddings")]
fn wants_auto_install(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--yes" || a == "-y" || a == "--install")
}

#[cfg(feature = "embeddings")]
fn prompts_allowed() -> bool {
    io::stdin().is_terminal() || io::stderr().is_terminal()
}

#[cfg(feature = "embeddings")]
fn confirms_install(line: &str) -> bool {
    matches!(line.trim().to_lowercase().as_str(), "" | "y" | "yes")
}

#[cfg(feature = "embeddings")]
fn install_default_embed_model() -> Result<()> {
    let path = install_model(
        &MINILM_MULTILINGUAL_Q,
        InstallOptions {
            quiet: false,
            force: false,
        },
    )
    .map_err(|e| NeuroMeshError::Config(e.to_string()))?;
    eprintln!("MiniLM installed at {}.", path.display());
    Ok(())
}

#[cfg(not(feature = "embeddings"))]
pub fn ensure_minilm_for_engine(_args: &[String], engine: RetrievalEngine) -> Result<()> {
    if engine == RetrievalEngine::Fast {
        return Ok(());
    }
    Err(NeuroMeshError::Config(format!(
        "{} requires embeddings feature (reinstall release binary)",
        engine.as_str()
    )))
}

#[cfg(all(test, feature = "embeddings"))]
mod install_prompt_tests {
    use super::{confirms_install, wants_auto_install};

    #[test]
    fn confirms_y_and_yes() {
        assert!(confirms_install("y"));
        assert!(confirms_install("Y"));
        assert!(confirms_install("yes"));
        assert!(confirms_install("YES"));
        assert!(confirms_install("\n"));
        assert!(!confirms_install("n"));
        assert!(!confirms_install("no"));
    }

    #[test]
    fn auto_install_flags() {
        let args = vec![
            "config".into(),
            "engine".into(),
            "hybrid".into(),
            "--yes".into(),
        ];
        assert!(wants_auto_install(&args));
    }
}

fn print_help() {
    println!("\nUsage:");
    println!("  neuromesh install embed list              Catalog + installed models");
    println!("  neuromesh install embed minilm            Download MiniLM Q weights");
    println!("  neuromesh install embed status            Check install state");
    println!("\n  hybrid/deep engines require an installed embed model.");
    println!("  neuromesh config engine hybrid --yes     install MiniLM without prompting\n");
}

fn print_embed_help() {
    print_help();
}
