use neuromesh_core::{NeuroMeshError, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// True when `path` is a project root, not home / Users / a drive or UNC root.
pub fn is_safe_workspace(path: &Path) -> bool {
    let candidate = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let candidate = strip_verbatim_prefix(&candidate);

    if is_filesystem_root(&candidate) {
        return false;
    }

    if let Some(home) = dirs::home_dir() {
        let home = home.canonicalize().unwrap_or(home);
        let home = strip_verbatim_prefix(&home);
        if paths_equal(&candidate, &home) {
            return false;
        }
    }

    let name = candidate
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if matches!(
        name.as_str(),
        "" | "users"
            | "windows"
            | "program files"
            | "program files (x86)"
            | "appdata"
            | "home"
            | "/"
    ) {
        return false;
    }

    #[cfg(unix)]
    {
        if candidate == Path::new("/home") || candidate == Path::new("/Users") {
            return false;
        }
    }

    true
}

/// Canonicalize `path` and refuse home / drive / Users roots. No directory walk.
pub fn assert_safe_workspace(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(NeuroMeshError::Config(format!(
            "refusing unsafe workspace: {} (does not exist)",
            path.display()
        )));
    }
    let canonical = path.canonicalize().map_err(|e| {
        NeuroMeshError::Config(format!(
            "refusing unsafe workspace: {} ({e})",
            path.display()
        ))
    })?;
    if !is_safe_workspace(&canonical) {
        return Err(NeuroMeshError::Config(format!(
            "refusing unsafe workspace: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

/// Resolve `requested` to a regular file that stays inside `workspace`.
pub fn resolve_workspace_file(workspace: &Path, requested: &Path) -> Result<PathBuf> {
    if requested.as_os_str().is_empty() {
        return Err(NeuroMeshError::Config("path is empty".into()));
    }
    let root = workspace.canonicalize().map_err(|e| {
        NeuroMeshError::Config(format!("workspace is not a readable directory: {e}"))
    })?;
    let root = strip_verbatim_prefix(&root);

    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let lexical = normalize_lexical(&candidate);
    if !is_path_within(&lexical, &root) {
        return Err(NeuroMeshError::Config("path is outside workspace".into()));
    }

    let canonical = candidate.canonicalize().map_err(|_| {
        NeuroMeshError::Config("path is outside workspace or is not a readable file".into())
    })?;
    let canonical_cmp = strip_verbatim_prefix(&canonical);
    if !is_path_within(&canonical_cmp, &root) {
        return Err(NeuroMeshError::Config("path is outside workspace".into()));
    }
    if !canonical.is_file() {
        return Err(NeuroMeshError::Config("path is not a regular file".into()));
    }
    Ok(canonical)
}

pub fn read_workspace_file(workspace: &Path, requested: &Path) -> Result<String> {
    let resolved = resolve_workspace_file(workspace, requested)?;
    fs::read_to_string(&resolved)
        .map_err(|e| NeuroMeshError::Config(format!("unable to read workspace file: {e}")))
}

/// True when `child` is `root` or a descendant (component-safe, not byte `starts_with`).
pub fn is_path_within(child: &Path, root: &Path) -> bool {
    let child = strip_verbatim_prefix(child);
    let root = strip_verbatim_prefix(root);
    child.strip_prefix(&root).is_ok()
}

/// Skip files whose canonical target leaves `root` (symlink escape).
pub fn path_escapes_workspace(full_path: &Path, root: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return true;
    };
    let root = strip_verbatim_prefix(&root);
    match full_path.canonicalize() {
        Ok(canon) => !is_path_within(&strip_verbatim_prefix(&canon), &root),
        Err(_) => fs::symlink_metadata(full_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(true),
    }
}

fn is_filesystem_root(path: &Path) -> bool {
    let mut comps = path.components();
    match comps.next() {
        Some(Component::RootDir) => comps.next().is_none(),
        Some(Component::Prefix(_)) => {
            matches!(comps.next(), Some(Component::RootDir)) && comps.next().is_none()
        }
        _ => false,
    }
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
        path.to_path_buf()
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn temp_workspace() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "nm-confine-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("lib.rs"), "pub fn ok() {}\n").unwrap();
        root
    }

    #[test]
    fn skeleton_allows_canonical_file_inside_workspace() {
        let root = temp_workspace();
        let resolved = resolve_workspace_file(&root, Path::new("src/lib.rs")).unwrap();
        assert!(resolved.ends_with("lib.rs"));
        let body = read_workspace_file(&root, Path::new("src/lib.rs")).unwrap();
        assert!(body.contains("pub fn ok"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skeleton_rejects_absolute_path_outside_workspace() {
        let root = temp_workspace();
        #[cfg(unix)]
        let outside = Path::new("/etc/passwd");
        #[cfg(windows)]
        let outside = Path::new(r"C:\Windows\win.ini");
        let err = resolve_workspace_file(&root, outside).unwrap_err();
        assert!(err.to_string().contains("outside workspace"), "{err}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skeleton_rejects_parent_directory_traversal() {
        let root = temp_workspace();
        let err = resolve_workspace_file(&root, Path::new("../../../../etc/passwd")).unwrap_err();
        assert!(err.to_string().contains("outside workspace"), "{err}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skeleton_never_returns_outside_file_content() {
        let root = temp_workspace();
        assert!(read_workspace_file(&root, Path::new("/etc/passwd")).is_err());
        assert!(read_workspace_file(&root, Path::new(r"C:\Windows\win.ini")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn skeleton_rejects_symlink_escaping_workspace() {
        let root = temp_workspace();
        let link = root.join("passwd-link");
        let _ = std::os::unix::fs::symlink("/etc/passwd", &link);
        if link.exists() {
            let err = resolve_workspace_file(&root, Path::new("passwd-link")).unwrap_err();
            assert!(err.to_string().contains("outside workspace"), "{err}");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_home_under_100ms() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let started = Instant::now();
        let err = assert_safe_workspace(&home).unwrap_err();
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "home refusal took {:?}",
            started.elapsed()
        );
        assert!(err.to_string().contains("refusing unsafe workspace"));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_unix_root_under_100ms() {
        let started = Instant::now();
        let err = assert_safe_workspace(Path::new("/")).unwrap_err();
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "root refusal took {:?}",
            started.elapsed()
        );
        assert!(err.to_string().contains("refusing unsafe workspace"));
        assert!(!is_safe_workspace(Path::new("/home")));
        assert!(!is_safe_workspace(Path::new("/Users")));
    }

    #[cfg(windows)]
    #[test]
    fn refuses_windows_drive_and_users_under_100ms() {
        let started = Instant::now();
        let err = assert_safe_workspace(Path::new(r"C:\")).unwrap_err();
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "drive-root refusal took {:?}",
            started.elapsed()
        );
        assert!(err.to_string().contains("refusing unsafe workspace"));
        assert!(!is_safe_workspace(Path::new(r"C:\Users")));
        assert!(!is_safe_workspace(Path::new(r"C:\Windows")));
    }

    #[test]
    fn is_path_within_does_not_match_prefix_sibling() {
        assert!(!is_path_within(
            Path::new("/tmp/project-evil/secret"),
            Path::new("/tmp/project")
        ));
        assert!(is_path_within(
            Path::new("/tmp/project/src/lib.rs"),
            Path::new("/tmp/project")
        ));
    }
}
