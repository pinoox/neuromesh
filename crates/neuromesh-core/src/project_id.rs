//! Stable, deterministic project identity.
//!
//! Before this module a [`ProjectId`] was built from the *directory name*
//! (`ProjectId::new(path.file_name())`), so two unrelated checkouts both called
//! `app`, `api`, or `backend` shared one identity. That made `ContextNode::project_id`
//! useless as an isolation signal and made "is this the same project?" checks
//! silently wrong.
//!
//! Here a `ProjectId` is a **pure function of the canonical path of the project
//! root** — the nearest enclosing git repository, or the workspace itself when the
//! directory is not in git. It is deterministic, allocation-cheap, and never
//! writes to the user's repository.
//!
//! Two different checkouts of the same repository are deliberately *different*
//! projects: their graphs live in different slots and must never be merged.

use crate::paths::normalize_workspace;
use crate::types::ProjectId;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Hex characters kept from the digest. 16 hex chars = 64 bits, which is far
/// beyond collision range for the number of projects one machine ever indexes.
const ID_HEX_LEN: usize = 16;

/// How far up the tree we look for a repository marker before giving up.
const MAX_ANCESTOR_WALK: usize = 64;

/// Nearest enclosing git repository root, or `start` itself when there is none.
///
/// Walks up looking for `.git` — a directory in a normal clone, a *file* in a
/// worktree or submodule, so both are accepted. Does not shell out to `git`, so
/// it works with no git binary on PATH.
pub fn project_root(start: &Path) -> PathBuf {
    let canonical = crate::paths::canonicalize(start)
        .unwrap_or_else(|_| crate::paths::strip_verbatim_prefix(start));
    let mut current = canonical.as_path();
    for _ in 0..MAX_ANCESTOR_WALK {
        if current.join(".git").exists() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    canonical
}

/// Deterministic identity for the project that owns `workspace`.
///
/// The digest is taken over [`normalize_workspace`] of the project root, which
/// is the same normalization the managed-store slot name uses, so an id and its
/// storage slot always agree about what "the same project" means.
pub fn stable_project_id(workspace: &Path) -> ProjectId {
    ProjectId::new(project_id_hex(&project_root(workspace)))
}

/// Identity for a path that is already known to be a project root — skips the
/// ancestor walk. Use when the caller resolved the root itself.
pub fn stable_project_id_for_root(root: &Path) -> ProjectId {
    ProjectId::new(project_id_hex(root))
}

fn project_id_hex(root: &Path) -> String {
    let key = normalize_workspace(root);
    let digest = Sha256::digest(key.as_bytes());
    digest
        .iter()
        .take(ID_HEX_LEN / 2)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// True when `path` lies inside `root`.
///
/// Both sides are slash-normalized and lowercased so Windows drive letters and
/// separators compare correctly. A path equal to `root` counts as inside.
/// Relative paths never match an absolute root — callers holding the relative
/// `ContextNode::file_path` should compare project ids instead.
pub fn path_is_within(path: &Path, root: &Path) -> bool {
    crate::paths::is_path_within(path, root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nm-pid-{tag}-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn same_path_yields_same_id() {
        let dir = temp_dir("same");
        assert_eq!(stable_project_id(&dir), stable_project_id(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn directory_name_collisions_get_distinct_ids() {
        // The bug this module exists to fix: two projects both named `app`.
        let base = temp_dir("collide");
        let one = base.join("one").join("app");
        let two = base.join("two").join("app");
        let _ = fs::create_dir_all(&one);
        let _ = fs::create_dir_all(&two);
        assert_ne!(
            stable_project_id(&one),
            stable_project_id(&two),
            "same directory name must not mean same project"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn id_is_stable_hex_of_expected_width() {
        let dir = temp_dir("width");
        let id = stable_project_id(&dir).0;
        assert_eq!(id.len(), ID_HEX_LEN);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn subdirectory_resolves_to_the_git_root() {
        let base = temp_dir("gitroot");
        let root = base.join("repo");
        let nested = root.join("crates").join("thing").join("src");
        let _ = fs::create_dir_all(&nested);
        let _ = fs::create_dir_all(root.join(".git"));

        assert_eq!(project_root(&nested), project_root(&root));
        assert_eq!(stable_project_id(&nested), stable_project_id(&root));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn worktree_git_file_counts_as_a_root() {
        let base = temp_dir("worktree");
        let root = base.join("wt");
        let nested = root.join("src");
        let _ = fs::create_dir_all(&nested);
        let _ = fs::write(root.join(".git"), "gitdir: /elsewhere/.git/worktrees/wt");

        assert_eq!(project_root(&nested), project_root(&root));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn without_git_the_workspace_is_its_own_root() {
        let base = temp_dir("nogit");
        let plain = base.join("plain");
        let _ = fs::create_dir_all(&plain);
        assert_eq!(project_root(&plain), project_root(&plain));
        assert_ne!(stable_project_id(&plain), stable_project_id(&base));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn path_within_root_checks() {
        let base = temp_dir("within");
        let root = base.join("root");
        let inside = root.join("src").join("main.rs");
        let sibling = base.join("root-other").join("main.rs");
        let _ = fs::create_dir_all(inside.parent().unwrap());
        let _ = fs::create_dir_all(sibling.parent().unwrap());

        assert!(path_is_within(&inside, &root));
        assert!(path_is_within(&root, &root));
        // `root-other` shares a textual prefix with `root` but is not inside it.
        assert!(!path_is_within(&sibling, &root));
        assert!(!path_is_within(Path::new("src/main.rs"), &root));
        let _ = fs::remove_dir_all(&base);
    }
}
