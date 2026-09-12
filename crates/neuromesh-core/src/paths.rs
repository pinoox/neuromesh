use crate::{NeuroMeshError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Where NeuroMesh keeps state that is not the user's repo.
/// Default `~/.neuromesh`. Override with `NEUROMESH_HOME`.
pub fn neuromesh_home() -> PathBuf {
    if let Ok(raw) = std::env::var("NEUROMESH_HOME") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".neuromesh")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStore {
    /// Graph, memory, and per-project config live under `~/.neuromesh/projects/`.
    /// A workspace `.neuromesh` directory is not read or written.
    #[default]
    Managed,
    /// Every workspace uses `<workspace>/.neuromesh` (legacy).
    Local,
}

impl ProjectStore {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "managed" | "global" | "home" => Ok(Self::Managed),
            "local" | "project" | "workspace" => Ok(Self::Local),
            other => Err(NeuroMeshError::Config(format!(
                "invalid store mode: {other} (use managed or local)"
            ))),
        }
    }
}

impl std::fmt::Display for ProjectStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Managed => write!(f, "managed"),
            Self::Local => write!(f, "local"),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct HomeStorePolicy {
    #[serde(default)]
    project_store: ProjectStore,
    #[serde(default)]
    trust_local: Vec<String>,
}

fn home_store_policy() -> HomeStorePolicy {
    let mut policy = HomeStorePolicy::default();
    let path = neuromesh_home().join("config.json");
    if path.exists() {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<HomeStorePolicy>(&raw) {
                policy = parsed;
            }
        }
    }
    if let Ok(raw) = std::env::var("NEUROMESH_STORE") {
        if let Ok(store) = ProjectStore::parse(&raw) {
            policy.project_store = store;
        }
    }
    policy
}

/// Strip Windows extended-length (verbatim) path prefix (`\\?\` or `\\?\UNC\`),
/// Win32 device prefixes (`\\.\`), and NT namespaces (`\??\`),
/// returning a standard clean path for display, comparison, and cross-platform consistency.
/// Runs unconditionally across all operating systems.
pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let stripped = if s.len() >= 8
        && (s[..8].eq_ignore_ascii_case(r"\\?\UNC\")
            || s[..8].eq_ignore_ascii_case(r"\\.\UNC\")
            || s[..8].eq_ignore_ascii_case(r"\??\UNC\"))
    {
        format!(r"\\{}", &s[8..])
    } else if s.len() >= 8
        && (s[..8].eq_ignore_ascii_case("//?/UNC/")
            || s[..8].eq_ignore_ascii_case("//./UNC/")
            || s[..8].eq_ignore_ascii_case("/??/UNC/"))
    {
        format!("//{}", &s[8..])
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else if let Some(rest) = s.strip_prefix("//?/") {
        rest.to_string()
    } else if let Some(rest) = s.strip_prefix(r"\??\") {
        rest.to_string()
    } else if let Some(rest) = s.strip_prefix("/??/") {
        rest.to_string()
    } else if let Some(rest) = s.strip_prefix(r"\\.\") {
        rest.to_string()
    } else if let Some(rest) = s.strip_prefix("//./") {
        rest.to_string()
    } else {
        s.into_owned()
    };

    // Normalize drive letter to uppercase for cross-platform consistency (e.g. c:\ -> C:\ or c:/ -> C:/)
    let bytes = stripped.as_bytes();
    if bytes.len() >= 2
        && bytes[0].is_ascii_lowercase()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'\\' || bytes[2] == b'/')
    {
        let mut chars: Vec<char> = stripped.chars().collect();
        chars[0] = chars[0].to_ascii_uppercase();
        return PathBuf::from(chars.into_iter().collect::<String>());
    }

    PathBuf::from(stripped)
}

/// Canonicalize `path` and strip any Windows verbatim prefix (`\\?\`).
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    path.canonicalize().map(|p| strip_verbatim_prefix(&p))
}

/// Check if two path components are equal according to the current platform's filesystem semantics.
/// Windows and macOS are case-insensitive by default; Linux/Unix is case-sensitive.
pub fn components_equal(a: &Component, b: &Component) -> bool {
    #[cfg(any(windows, target_os = "macos"))]
    {
        a.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        a == b
    }
}

/// Compare two paths component-by-component without canonicalization.
pub fn components_equal_path(a: &Path, b: &Path) -> bool {
    let mut a_comps = a.components();
    let mut b_comps = b.components();
    loop {
        match (a_comps.next(), b_comps.next()) {
            (None, None) => return true,
            (Some(ca), Some(cb)) => {
                if !components_equal(&ca, &cb) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// True when two paths refer to the same location across OSes.
/// Handles case-insensitivity (Windows/macOS), separators, verbatim prefixes, and symlinks.
pub fn paths_equal(a: &Path, b: &Path) -> bool {
    let a_clean = strip_verbatim_prefix(a);
    let b_clean = strip_verbatim_prefix(b);

    if components_equal_path(&a_clean, &b_clean) {
        return true;
    }

    let a_canon = canonicalize(&a_clean).unwrap_or_else(|_| a_clean);
    let b_canon = canonicalize(&b_clean).unwrap_or_else(|_| b_clean);

    components_equal_path(&a_canon, &b_canon)
}

/// Check if child components start with root components.
fn component_starts_with(child: &Path, root: &Path) -> bool {
    let mut child_comps = child.components();
    for root_comp in root.components() {
        let Some(child_comp) = child_comps.next() else {
            return false;
        };
        if !components_equal(&child_comp, &root_comp) {
            return false;
        }
    }
    true
}

/// True when `child` lies inside `root` or equals `root`.
/// Handles case-insensitivity on Windows/macOS, slashes, verbatim prefixes, and symlinks (e.g. /tmp on macOS).
pub fn is_path_within(child: &Path, root: &Path) -> bool {
    let child_clean = strip_verbatim_prefix(child);
    let root_clean = strip_verbatim_prefix(root);

    // 1. Direct component check
    if component_starts_with(&child_clean, &root_clean) {
        return true;
    }

    // 2. Canonical check (resolving symlinks like /tmp -> /private/tmp on macOS)
    let child_canon = canonicalize(&child_clean).unwrap_or_else(|_| {
        if let Some(parent) = child_clean.parent() {
            if let Ok(canon_parent) = canonicalize(parent) {
                if let Some(name) = child_clean.file_name() {
                    return canon_parent.join(name);
                }
            }
        }
        child_clean.clone()
    });
    let root_canon = canonicalize(&root_clean).unwrap_or_else(|_| root_clean.clone());

    component_starts_with(&child_canon, &root_canon)
}

/// Strip `root` from `child`, returning the relative path if `child` is within `root`.
pub fn strip_prefix_within(child: &Path, root: &Path) -> Option<PathBuf> {
    let child_clean = strip_verbatim_prefix(child);
    let root_clean = strip_verbatim_prefix(root);

    if let Some(rel) = component_strip_prefix(&child_clean, &root_clean) {
        return Some(rel);
    }

    let child_canon = canonicalize(&child_clean).unwrap_or_else(|_| child_clean.clone());
    let root_canon = canonicalize(&root_clean).unwrap_or_else(|_| root_clean.clone());

    component_strip_prefix(&child_canon, &root_canon)
}

fn component_strip_prefix(child: &Path, root: &Path) -> Option<PathBuf> {
    let mut child_comps = child.components();
    for root_comp in root.components() {
        let child_comp = child_comps.next()?;
        if !components_equal(&child_comp, &root_comp) {
            return None;
        }
    }
    let mut rel = PathBuf::new();
    for rem in child_comps {
        rel.push(rem.as_os_str());
    }
    Some(rel)
}

/// Canonical lowercase `/`-separated path used as a stable project key.
pub fn normalize_workspace(path: &Path) -> String {
    let canon = canonicalize(path).unwrap_or_else(|_| strip_verbatim_prefix(path));
    let mut s = canon.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    let trimmed = if s.len() > 1 && s.ends_with('/') {
        s.trim_end_matches('/').to_string()
    } else {
        s
    };
    trimmed.to_lowercase()
}

fn workspace_is_trusted(policy: &HomeStorePolicy, workspace: &Path) -> bool {
    if policy.project_store == ProjectStore::Local {
        return true;
    }
    let n = normalize_workspace(workspace);
    policy.trust_local.iter().any(|entry| {
        paths_equal(Path::new(entry), workspace)
            || entry.replace('\\', "/").eq_ignore_ascii_case(&n)
    })
}

/// True when this workspace is allowed to use `<workspace>/.neuromesh`.
pub fn uses_local_dotdir(workspace: &Path) -> bool {
    workspace_is_trusted(&home_store_policy(), workspace)
}

fn project_slot_name(workspace: &Path) -> String {
    let key = normalize_workspace(workspace);
    let hash = Sha256::digest(key.as_bytes());
    let hex = hash
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let slug = workspace
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(32)
        .collect::<String>();
    let slug = if slug.is_empty() {
        "project".to_string()
    } else {
        slug
    };
    format!("{slug}-{hex}")
}

/// Resolved data directory for a workspace (does not create it).
pub fn project_data_dir(workspace: &Path) -> PathBuf {
    let policy = home_store_policy();
    if workspace_is_trusted(&policy, workspace) {
        return workspace.join(".neuromesh");
    }
    neuromesh_home()
        .join("projects")
        .join(project_slot_name(workspace))
}

fn copy_if_missing(from: &Path, to: &Path) {
    if from.exists() && !to.exists() {
        if let Some(parent) = to.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::copy(from, to);
    }
}

fn write_workspace_meta(dir: &Path, workspace: &Path) {
    let meta = dir.join("workspace.json");
    if meta.exists() {
        return;
    }
    let name = workspace
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let body = serde_json::json!({
        "path": normalize_workspace(workspace),
        "name": name,
    });
    if let Ok(raw) = serde_json::to_string_pretty(&body) {
        let _ = fs::write(meta, raw);
    }
}

fn migrate_legacy_dotdir(workspace: &Path, dest: &Path) {
    if uses_local_dotdir(workspace) {
        return;
    }
    let src = workspace.join(".neuromesh");
    if !src.is_dir() {
        return;
    }
    for name in [
        "graph.bin",
        "graph.json",
        "embeddings.bin",
        "neuromesh.json",
        "config.json",
    ] {
        copy_if_missing(&src.join(name), &dest.join(name));
    }
}

/// Create the data dir, write `workspace.json`, and copy leftover in-repo
/// `.neuromesh` files into the managed slot once (without trusting that folder).
pub fn ensure_project_data_dir(workspace: &Path) -> Result<PathBuf> {
    let dir = project_data_dir(workspace);
    fs::create_dir_all(&dir)?;
    write_workspace_meta(&dir, workspace);
    migrate_legacy_dotdir(workspace, &dir);
    Ok(dir)
}

pub fn graph_path(workspace: &Path) -> PathBuf {
    project_data_dir(workspace).join("graph.bin")
}

pub fn embeddings_path(workspace: &Path) -> PathBuf {
    project_data_dir(workspace).join("embeddings.bin")
}

pub fn memory_db_path(workspace: &Path) -> PathBuf {
    let dir = ensure_project_data_dir(workspace).unwrap_or_else(|_| project_data_dir(workspace));
    dir.join("neuromesh.json")
}

pub fn project_config_path(workspace: &Path) -> PathBuf {
    project_data_dir(workspace).join("config.json")
}

pub fn leftover_workspace_dotdir(workspace: &Path) -> Option<PathBuf> {
    if uses_local_dotdir(workspace) {
        return None;
    }
    let local = workspace.join(".neuromesh");
    local.is_dir().then_some(local)
}

pub fn copy_store_files(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for name in [
        "graph.bin",
        "graph.json",
        "embeddings.bin",
        "neuromesh.json",
        "config.json",
    ] {
        copy_if_missing(&from.join(name), &to.join(name));
    }
    Ok(())
}

/// Persist store policy in `~/.neuromesh/config.json` (merge, no secrets copied).
pub fn save_store_policy(store: ProjectStore, trust_local: Vec<String>) -> Result<PathBuf> {
    let home = neuromesh_home();
    fs::create_dir_all(&home)?;
    let path = home.join("config.json");
    let mut value = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "project_store".into(),
            serde_json::Value::String(store.to_string()),
        );
        obj.insert("trust_local".into(), serde_json::to_value(&trust_local)?);
    }
    fs::write(&path, serde_json::to_string_pretty(&value)?)?;
    Ok(path)
}

pub fn current_trust_list() -> Vec<String> {
    home_store_policy().trust_local
}

pub fn current_project_store() -> ProjectStore {
    home_store_policy().project_store
}

pub fn trust_workspace_local(workspace: &Path) -> Result<PathBuf> {
    let key = normalize_workspace(workspace);
    let store = current_project_store();
    let mut trust = current_trust_list();
    if store != ProjectStore::Local && !trust.iter().any(|e| paths_equal(Path::new(e), Path::new(&key))) {
        trust.push(key);
    }
    save_store_policy(store, trust)?;
    let local = workspace.join(".neuromesh");
    let managed = neuromesh_home()
        .join("projects")
        .join(project_slot_name(workspace));
    if managed.is_dir() {
        copy_store_files(&managed, &local)?;
    } else {
        fs::create_dir_all(&local)?;
    }
    write_workspace_meta(&local, workspace);
    Ok(local)
}

pub fn untrust_workspace_local(workspace: &Path) -> Result<PathBuf> {
    let key = normalize_workspace(workspace);
    let trust: Vec<String> = current_trust_list()
        .into_iter()
        .filter(|e| !paths_equal(Path::new(e), Path::new(&key)) && !paths_equal(Path::new(e), workspace))
        .collect();
    save_store_policy(ProjectStore::Managed, trust)?;
    let managed = ensure_project_data_dir(workspace)?;
    Ok(managed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_slot_is_outside_the_repo() {
        let home = PathBuf::from("/tmp/nm-home-test");
        let ws = PathBuf::from("/tmp/repos/my-app");
        let slot = home.join("projects").join(project_slot_name(&ws));
        assert!(slot.starts_with(home.join("projects")));
        assert!(!slot.starts_with(&ws));
        assert!(slot
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("my-app-"));
    }

    #[test]
    fn local_store_trusts_every_workspace() {
        let ws = PathBuf::from("/tmp/repos/app");
        let policy = HomeStorePolicy {
            project_store: ProjectStore::Local,
            trust_local: vec![],
        };
        assert!(workspace_is_trusted(&policy, &ws));
    }

    #[test]
    fn trust_list_matches_normalized_path() {
        let ws = PathBuf::from(".");
        let n = normalize_workspace(&ws);
        let policy = HomeStorePolicy {
            project_store: ProjectStore::Managed,
            trust_local: vec![n],
        };
        assert!(workspace_is_trusted(&policy, &ws));
    }

    #[test]
    fn parse_store_aliases() {
        assert_eq!(
            ProjectStore::parse("managed").unwrap(),
            ProjectStore::Managed
        );
        assert_eq!(ProjectStore::parse("local").unwrap(), ProjectStore::Local);
        assert_eq!(
            ProjectStore::parse("global").unwrap(),
            ProjectStore::Managed
        );
        assert!(ProjectStore::parse("nope").is_err());
    }

    #[test]
    fn strip_verbatim_prefix_cleans_paths() {
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\C:\foo\bar")),
            PathBuf::from(r"C:\foo\bar")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\c:\foo\bar")),
            PathBuf::from(r"C:\foo\bar")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\UNC\server\share\file")),
            PathBuf::from(r"\\server\share\file")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\unc\server\share\file")),
            PathBuf::from(r"\\server\share\file")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"//?/unc/server/share/file")),
            PathBuf::from("//server/share/file")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\.\C:\foo\bar")),
            PathBuf::from(r"C:\foo\bar")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\??\C:\foo\bar")),
            PathBuf::from(r"C:\foo\bar")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"//?/c:/foo/bar")),
            PathBuf::from("C:/foo/bar")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new("c:/foo/bar")),
            PathBuf::from("C:/foo/bar")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new("/normal/path")),
            PathBuf::from("/normal/path")
        );
    }

    #[test]
    fn normalize_workspace_trims_trailing_slash_and_lowercases() {
        let p1 = Path::new("/projects/app");
        let p2 = Path::new("/projects/app/");
        assert_eq!(normalize_workspace(p1), normalize_workspace(p2));
    }

    #[test]
    fn test_cross_platform_path_matching() {
        assert!(is_path_within(
            Path::new("/tmp/project/src/lib.rs"),
            Path::new("/tmp/project")
        ));
        assert!(is_path_within(
            Path::new("/tmp/project"),
            Path::new("/tmp/project")
        ));
        assert!(!is_path_within(
            Path::new("/tmp/project-evil/secret"),
            Path::new("/tmp/project")
        ));
        assert_eq!(
            strip_prefix_within(
                Path::new("/tmp/project/src/main.rs"),
                Path::new("/tmp/project")
            ),
            Some(PathBuf::from("src/main.rs"))
        );

        #[cfg(windows)]
        {
            assert!(is_path_within(
                Path::new(r"C:\Projects\app\src\main.rs"),
                Path::new(r"c:\projects\app")
            ));
            assert!(paths_equal(
                Path::new(r"c:\projects\app"),
                Path::new(r"C:\projects\app")
            ));
            assert_eq!(
                strip_prefix_within(
                    Path::new(r"C:\Projects\app\src\main.rs"),
                    Path::new(r"c:\projects\app")
                ),
                Some(PathBuf::from(r"src\main.rs"))
            );
        }
    }
}
