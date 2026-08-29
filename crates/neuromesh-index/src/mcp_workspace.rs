use crate::walker::ProjectWalker;
use std::path::{Path, PathBuf};

/// First workspace folder from IDE env vars (Cursor, VS Code, Copilot, …).
pub fn workspace_from_ide_env() -> Option<PathBuf> {
    for key in [
        "WORKSPACE_FOLDER_PATHS",
        "VSCODE_CWD",
        "CURSOR_WORKSPACE",
        "CURSOR_PROJECT_DIR",
    ] {
        if let Ok(raw) = std::env::var(key) {
            if let Some(path) = parse_workspace_folder_paths(&raw) {
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Resolve the MCP workspace at process start (no CLI path / no explicit env pin).
pub fn resolve_mcp_startup_workspace() -> PathBuf {
    if let Some(path) = workspace_from_ide_env() {
        return ProjectWalker::explicit_workspace(&path);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    ProjectWalker::discover_workspace(&cwd)
}

/// Parse Cursor/VS Code multi-root env values and plain paths.
pub fn parse_workspace_folder_paths(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('[') {
        if let Ok(values) = serde_json::from_str::<Vec<String>>(raw) {
            return values
                .into_iter()
                .find_map(|entry| first_existing_path(&entry));
        }
    }
    for sep in [';', '\n'] {
        if raw.contains(sep) {
            for part in raw.split(sep) {
                if let Some(path) = first_existing_path(part) {
                    return Some(path);
                }
            }
            return None;
        }
    }
    first_existing_path(raw)
}

fn first_existing_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }
    let path = normalize_env_path(trimmed);
    if !path.exists() {
        return None;
    }
    Some(
        path.canonicalize()
            .map(strip_verbatim_prefix)
            .unwrap_or(path),
    )
}

fn normalize_env_path(raw: &str) -> PathBuf {
    if raw.starts_with(r"\\?\") {
        PathBuf::from(&raw[4..])
    } else {
        PathBuf::from(raw)
    }
}

fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

/// True when two paths refer to the same existing directory (best-effort canonicalize).
pub fn same_workspace_path(a: Option<&Path>, b: &Path) -> bool {
    let Some(a) = a else {
        return false;
    };
    let a_canon = a
        .canonicalize()
        .map(strip_verbatim_prefix)
        .unwrap_or_else(|_| strip_verbatim_prefix(a.to_path_buf()));
    let b_canon = b
        .canonicalize()
        .map(strip_verbatim_prefix)
        .unwrap_or_else(|_| strip_verbatim_prefix(b.to_path_buf()));
    a_canon == b_canon
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_json_workspace_list() {
        let tmp = std::env::temp_dir().join(format!("nm-mcp-ws-{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp);
        let json = serde_json::to_string(&vec![tmp.to_string_lossy().to_string()]).unwrap();
        let expected = tmp.canonicalize().map(strip_verbatim_prefix).unwrap_or(tmp.clone());
        assert_eq!(parse_workspace_folder_paths(&json).unwrap(), expected);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parses_semicolon_separated_paths() {
        let tmp = std::env::temp_dir().join(format!("nm-mcp-ws2-{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp);
        let raw = format!("C:\\missing\\nope;{}", tmp.display());
        let expected = tmp.canonicalize().map(strip_verbatim_prefix).unwrap_or(tmp.clone());
        assert_eq!(parse_workspace_folder_paths(&raw).unwrap(), expected);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ignores_missing_paths() {
        assert!(
            parse_workspace_folder_paths("C:\\definitely-not-a-neuromesh-workspace-zz").is_none()
        );
    }
}
