mod merge;

use merge::{
    load_json_object, load_text, upsert_kilo_mcp, upsert_mcp_servers, upsert_toml_table,
    upsert_vscode_servers, write_pretty_json, write_text, LaunchSpec,
};
use neuromesh_core::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Flags {
    print_only: bool,
    dry_run: bool,
    project_only: bool,
    force_global: bool,
    pinned: bool,
    help: bool,
}

enum Kind {
    McpServers,
    Vscode,
    Kilo,
    CodexToml,
}

struct Target {
    label: &'static str,
    path: PathBuf,
    kind: Kind,
    global: bool,
}

pub fn execute(args: &[String]) -> Result<()> {
    let flags = parse_flags(args.get(2..).unwrap_or(&[]))?;
    if flags.help {
        print_help();
        return Ok(());
    }

    let spec = launch_spec(flags.pinned)?;
    print_banner();
    if flags.print_only {
        print_snippets(&spec);
        return Ok(());
    }

    let mut wrote = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for target in all_targets() {
        if target.global && flags.project_only {
            skipped.push(format!(
                "{} ({}) — project-only",
                target.path.display(),
                target.label
            ));
            continue;
        }
        if target.global && !flags.force_global && !app_present(&target.path) {
            skipped.push(format!(
                "{} ({}) — app not installed, use --global",
                target.path.display(),
                target.label
            ));
            continue;
        }
        if target.path.file_name().and_then(|n| n.to_str()) == Some("settings.json")
            && !target.path.exists()
        {
            skipped.push(format!(
                "{} ({}) — will not create a full settings file",
                target.path.display(),
                target.label
            ));
            continue;
        }
        if flags.dry_run {
            wrote.push(format!(
                "{} ({}) [dry-run]",
                target.path.display(),
                target.label
            ));
            continue;
        }
        match write_target(&target, &spec) {
            Ok(()) => wrote.push(format!("{} ({})", target.path.display(), target.label)),
            Err(e) => failed.push(format!("{} ({}): {e}", target.path.display(), target.label)),
        }
    }

    println!("Binary    : {}", spec.command);
    println!("Workspace : {}", spec.cwd);
    if !wrote.is_empty() {
        println!("\nWrote:");
        for line in &wrote {
            println!("  {line}");
        }
    }
    if !skipped.is_empty() {
        println!("\nSkipped:");
        for line in &skipped {
            println!("  {line}");
        }
    }
    if !failed.is_empty() {
        println!("\nFailed:");
        for line in &failed {
            println!("  {line}");
        }
        return Err(neuromesh_core::NeuroMeshError::Config(
            "some MCP configs could not be written".into(),
        ));
    }
    println!("\nRestart the agent / IDE so it reloads MCP.");
    println!("Print snippets only: neuromesh connect --print");
    println!();
    Ok(())
}

fn parse_flags(args: &[String]) -> Result<Flags> {
    let mut flags = Flags {
        print_only: false,
        dry_run: false,
        project_only: false,
        force_global: false,
        pinned: false,
        help: false,
    };
    for a in args {
        match a.as_str() {
            "--print" | "-n" => flags.print_only = true,
            "--dry-run" | "--dry" => flags.dry_run = true,
            "--project" => flags.project_only = true,
            "--global" | "--user" => flags.force_global = true,
            "--pinned" => flags.pinned = true,
            "--help" | "-h" => flags.help = true,
            other if other.starts_with('-') => {
                return Err(neuromesh_core::NeuroMeshError::Config(format!(
                    "unknown connect flag: {other} (see neuromesh connect --help)"
                )));
            }
            other => {
                return Err(neuromesh_core::NeuroMeshError::Config(format!(
                    "unknown connect argument: {other}"
                )));
            }
        }
    }
    Ok(flags)
}

fn print_banner() {
    const INNER: usize = 83;
    let title = format!("NeuroMesh v{} — MCP connect", env!("CARGO_PKG_VERSION"));
    let vis = title.chars().count();
    let pad = INNER.saturating_sub(vis);
    let left = pad / 2;
    let right = pad - left;
    println!();
    println!("╔{}╗", "═".repeat(INNER));
    println!("║{}{}{}║", " ".repeat(left), title, " ".repeat(right));
    println!("╚{}╝", "═".repeat(INNER));
    println!();
}

fn print_help() {
    print_banner();
    println!(
        "\
Usage: neuromesh connect [OPTIONS]

Install NeuroMesh as a local MCP stdio server. Default config is portable:
`neuromesh mcp` on PATH with automatic workspace detection per IDE project.
Use `--pinned` to write an absolute binary path and pin the current workspace.

  neuromesh connect              write project configs + globals for installed apps
  neuromesh connect --print      print snippets only (do not write)
  neuromesh connect --dry-run    show target files without writing
  neuromesh connect --project    project files only
  neuromesh connect --global     also create user-level configs (Cursor, Codex, …)
  neuromesh connect --pinned     absolute binary + workspace args (legacy)

Project files: .cursor .vscode .agents .codex .kilo .trae .mcp.json .minimax
Globals (when the app is present, or with --global): Cursor, Claude Desktop,
Codex, Antigravity/Gemini, Windsurf, Trae, Kilo Code, Cline/Roo.
"
    );
}

fn launch_spec(pinned: bool) -> Result<LaunchSpec> {
    if pinned {
        return launch_spec_pinned();
    }
    Ok(LaunchSpec::simple())
}

fn launch_spec_pinned() -> Result<LaunchSpec> {
    let command = resolve_binary()?.to_string_lossy().into_owned();
    let workspace = neuromesh_index::assert_safe_workspace(
        &neuromesh_index::ProjectWalker::discover_workspace(
            &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        ),
    )?;
    let cwd = workspace.to_string_lossy().into_owned();
    let mut env = BTreeMap::new();
    env.insert("NEUROMESH_WORKSPACE".into(), cwd.clone());
    Ok(LaunchSpec {
        command,
        args: vec!["mcp".into(), cwd.clone()],
        cwd,
        env,
    })
}

fn resolve_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| {
        neuromesh_core::NeuroMeshError::Config(format!("cannot resolve neuromesh binary: {e}"))
    })?;
    let exe = strip_verbatim(std::fs::canonicalize(&exe).unwrap_or(exe));
    if is_ephemeral_build(&exe) {
        if let Some(stable) = find_stable_binary(&exe) {
            return Ok(stable);
        }
    }
    Ok(exe)
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "neuromesh.exe"
    } else {
        "neuromesh"
    }
}

fn is_ephemeral_build(path: &Path) -> bool {
    let s = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    s.contains("/target/debug/") || s.contains("/deps/") || s.contains("cursor-sandbox-cache")
}

fn find_stable_binary(current: &Path) -> Option<PathBuf> {
    let ws = neuromesh_index::ProjectWalker::discover_workspace(
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );
    let local_programs = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Programs")
        .join("neuromesh")
        .join(binary_name());
    let cargo_bin = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cargo")
        .join("bin")
        .join(binary_name());
    let local_bin = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("bin")
        .join(binary_name());
    let release = ws.join("target").join("release").join(binary_name());
    for cand in [release, local_programs, cargo_bin, local_bin] {
        if cand.is_file() {
            let resolved = strip_verbatim(std::fs::canonicalize(&cand).unwrap_or(cand));
            if resolved != *current {
                return Some(resolved);
            }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(binary_name());
            if cand.is_file() {
                let resolved = strip_verbatim(std::fs::canonicalize(&cand).unwrap_or(cand));
                if resolved != *current && !is_ephemeral_build(&resolved) {
                    return Some(resolved);
                }
            }
        }
    }
    None
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn all_targets() -> Vec<Target> {
    let ws = neuromesh_index::ProjectWalker::discover_workspace(
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );
    let mut targets = vec![
        Target {
            label: "Cursor",
            path: ws.join(".cursor").join("mcp.json"),
            kind: Kind::McpServers,
            global: false,
        },
        Target {
            label: "VS Code / Copilot",
            path: ws.join(".vscode").join("mcp.json"),
            kind: Kind::Vscode,
            global: false,
        },
        Target {
            label: "Antigravity",
            path: ws.join(".agents").join("mcp_config.json"),
            kind: Kind::McpServers,
            global: false,
        },
        Target {
            label: "Codex",
            path: ws.join(".codex").join("config.toml"),
            kind: Kind::CodexToml,
            global: false,
        },
        Target {
            label: "Kilo Code",
            path: ws.join(".kilo").join("kilo.jsonc"),
            kind: Kind::Kilo,
            global: false,
        },
        Target {
            label: "Trae",
            path: ws.join(".trae").join("mcp.json"),
            kind: Kind::McpServers,
            global: false,
        },
        Target {
            label: "Claude Code / generic",
            path: ws.join(".mcp.json"),
            kind: Kind::McpServers,
            global: false,
        },
        Target {
            label: "MiniMax Code",
            path: ws.join(".minimax").join("mcp.json"),
            kind: Kind::McpServers,
            global: false,
        },
    ];
    targets.extend(global_targets());
    targets
}

fn global_targets() -> Vec<Target> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let config = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
    let mut out = vec![
        Target {
            label: "Cursor (user)",
            path: home.join(".cursor").join("mcp.json"),
            kind: Kind::McpServers,
            global: true,
        },
        Target {
            label: "Claude Desktop",
            path: config.join("Claude").join("claude_desktop_config.json"),
            kind: Kind::McpServers,
            global: true,
        },
        Target {
            label: "Codex (user)",
            path: home.join(".codex").join("config.toml"),
            kind: Kind::CodexToml,
            global: true,
        },
        Target {
            label: "Antigravity / Gemini",
            path: home.join(".gemini").join("config").join("mcp_config.json"),
            kind: Kind::McpServers,
            global: true,
        },
        Target {
            label: "Gemini CLI",
            path: home.join(".gemini").join("settings.json"),
            kind: Kind::McpServers,
            global: true,
        },
        Target {
            label: "Windsurf",
            path: home
                .join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
            kind: Kind::McpServers,
            global: true,
        },
        Target {
            label: "Trae (user)",
            path: config.join("Trae").join("User").join("mcp.json"),
            kind: Kind::McpServers,
            global: true,
        },
        Target {
            label: "Kilo Code (user)",
            path: config.join("kilo").join("kilo.jsonc"),
            kind: Kind::Kilo,
            global: true,
        },
    ];
    let kilo_home = home.join(".config").join("kilo").join("kilo.jsonc");
    if kilo_home != config.join("kilo").join("kilo.jsonc") {
        out.push(Target {
            label: "Kilo Code (home)",
            path: kilo_home,
            kind: Kind::Kilo,
            global: true,
        });
    }
    for (label, path) in cline_roo_paths(&config) {
        out.push(Target {
            label,
            path,
            kind: Kind::McpServers,
            global: true,
        });
    }
    out
}

fn cline_roo_paths(config: &Path) -> Vec<(&'static str, PathBuf)> {
    let editors = ["Code", "Cursor", "Code - Insiders", "VSCodium"];
    let ext_files = [
        ("Cline", "saoudrizwan.claude-dev", "cline_mcp_settings.json"),
        (
            "Roo Code",
            "rooveterinaryinc.roo-cline",
            "mcp_settings.json",
        ),
    ];
    let mut out = Vec::new();
    for editor in editors {
        for (label, ext, file) in ext_files {
            out.push((
                label,
                config
                    .join(editor)
                    .join("User")
                    .join("globalStorage")
                    .join(ext)
                    .join("settings")
                    .join(file),
            ));
        }
    }
    out
}

fn app_present(path: &Path) -> bool {
    if path.exists() {
        return true;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    if parent.exists() {
        return true;
    }
    let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(parent_name, "config" | "User" | "settings")
        && parent.parent().is_some_and(|p| p.exists())
}

fn write_target(target: &Target, spec: &LaunchSpec) -> std::result::Result<(), String> {
    match target.kind {
        Kind::McpServers => {
            let mut root = load_json_object(&target.path)?;
            upsert_mcp_servers(&mut root, "neuromesh", spec.mcp_servers_entry());
            write_pretty_json(&target.path, &root)
        }
        Kind::Vscode => {
            let mut root = load_json_object(&target.path)?;
            upsert_vscode_servers(&mut root, "neuromesh", spec.vscode_entry());
            write_pretty_json(&target.path, &root)
        }
        Kind::Kilo => {
            let mut root = load_json_object(&target.path)?;
            upsert_kilo_mcp(&mut root, "neuromesh", spec.kilo_entry());
            write_pretty_json(&target.path, &root)
        }
        Kind::CodexToml => {
            let existing = load_text(&target.path)?;
            let body = upsert_toml_table(&existing, "mcp_servers.neuromesh", &spec.toml_block());
            write_text(&target.path, &body)
        }
    }
}

fn print_snippets(spec: &LaunchSpec) {
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": { "neuromesh": spec.mcp_servers_entry() }
    }))
    .unwrap_or_default();
    let vscode = serde_json::to_string_pretty(&serde_json::json!({
        "servers": { "neuromesh": spec.vscode_entry() }
    }))
    .unwrap_or_default();
    let kilo = serde_json::to_string_pretty(&serde_json::json!({
        "mcp": { "neuromesh": spec.kilo_entry() }
    }))
    .unwrap_or_default();

    println!("Cursor / Claude / Trae / Antigravity / MiniMax / Windsurf — mcpServers:\n{json}\n");
    println!("VS Code / Copilot — .vscode/mcp.json:\n{vscode}\n");
    println!("Kilo Code — kilo.jsonc `mcp` (command array, environment):\n{kilo}\n");
    println!(
        "Codex — ~/.codex/config.toml or .codex/config.toml:\n{}",
        spec.toml_block()
    );
    println!(
        "claude mcp add neuromesh -- {} mcp{}\n",
        spec.command,
        if spec.args.len() > 1 {
            format!(" {}", spec.args[1..].join(" "))
        } else {
            String::new()
        }
    );
}

#[cfg(test)]
mod tests {
    use super::parse_flags;

    #[test]
    fn parses_connect_flags() {
        let f = parse_flags(&["--print".into(), "--project".into()]).unwrap();
        assert!(f.print_only && f.project_only && !f.force_global);
        let f = parse_flags(&["--global".into(), "--dry-run".into()]).unwrap();
        assert!(f.force_global && f.dry_run);
        let f = parse_flags(&["--pinned".into()]).unwrap();
        assert!(f.pinned);
        assert!(parse_flags(&["--nope".into()]).is_err());
    }

    #[test]
    fn debug_target_is_ephemeral() {
        assert!(super::is_ephemeral_build(std::path::Path::new(
            r"C:\proj\target\debug\neuromesh.exe"
        )));
        assert!(!super::is_ephemeral_build(std::path::Path::new(
            r"C:\Users\me\AppData\Local\Programs\neuromesh\neuromesh.exe"
        )));
    }
}
