use crate::hasher::ContentHasher;
use crate::tracker::{IndexedFile, SourceLanguage};
use chrono::{DateTime, Utc};
use neuromesh_core::{ProjectId, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ProjectWalker {
    root_path: PathBuf,
    project_id: ProjectId,
    max_file_size: u64,
    max_files: usize,
}

impl ProjectWalker {
    pub fn new(root_path: PathBuf, project_id: ProjectId) -> Self {
        Self {
            root_path,
            project_id,
            max_file_size: 2 * 1024 * 1024, // 2MB max text file
            max_files: 6_000,
        }
    }

    /// Walk up from `start` to a git/cargo root, refusing home and drive roots.
    pub fn discover_workspace(start: &Path) -> PathBuf {
        let mut current = start.to_path_buf();
        loop {
            if !Self::is_safe_workspace(&current) {
                break;
            }
            if current.join(".git").exists()
                || current.join("Cargo.toml").exists()
                || current.join("package.json").exists()
                || current.join("pyproject.toml").exists()
            {
                return current;
            }
            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => break,
            }
        }
        start.to_path_buf()
    }

    pub fn is_safe_workspace(path: &Path) -> bool {
        if let Some(home) = dirs::home_dir() {
            if path == home {
                return false;
            }
        }
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        !matches!(
            name.as_str(),
            "" | "users" | "windows" | "program files" | "program files (x86)" | "appdata" | "/"
        )
    }

    pub fn is_ignored(path: &Path) -> bool {
        for component in path.components() {
            let s = component.as_os_str().to_string_lossy();
            let s_lower = s.to_lowercase();
            if s_lower == "node_modules"
                || s_lower == "target"
                || s_lower == ".git"
                || s_lower == ".neuromesh"
                || s_lower == "dist"
                || s_lower == "build"
                || s_lower == ".next"
                || s_lower == ".nuxt"
                || s_lower == "vendor"
                || s_lower == ".venv"
                || s_lower == "venv"
                || s_lower == "__pycache__"
                || s_lower == ".cache"
                || s_lower == "appdata"
                || s_lower == ".cargo"
                || s_lower == ".rustup"
                || s_lower == ".gemini"
                || s_lower == ".npm"
                || s_lower == ".nuget"
                || s_lower == ".gradle"
                || s_lower == ".m2"
                || s_lower == ".local"
                || s_lower == ".vscode"
                || s_lower == ".idea"
                || s_lower == "local settings"
                || s_lower == "application data"
                || s_lower == "benches"
                || s_lower == "examples"
                || s_lower == "testdata"
                || s_lower == "test_data"
                || s_lower == ".tox"
                || s_lower == ".mypy_cache"
                || s_lower == ".pytest_cache"
            {
                return true;
            }
        }
        false
    }

    pub fn scan(&self) -> Result<Vec<(IndexedFile, String)>> {
        let mut results = Vec::new();

        for entry in WalkDir::new(&self.root_path)
            .max_depth(10)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !Self::is_ignored(e.path()))
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let full_path = entry.path().to_path_buf();
            let relative_path = match full_path.strip_prefix(&self.root_path) {
                Ok(p) => p.to_path_buf(),
                Err(_) => full_path.clone(),
            };

            let metadata = match fs::metadata(&full_path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if metadata.len() > self.max_file_size {
                continue;
            }

            let language = SourceLanguage::from_path(&relative_path);
            if language == SourceLanguage::Unknown {
                continue;
            }

            let content = match fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue, // Skip binary or non-utf8 files
            };

            let hash = ContentHasher::hash_str(&content);
            let last_modified: DateTime<Utc> = metadata
                .modified()
                .map(|t| t.into())
                .unwrap_or_else(|_| Utc::now());

            let indexed_file = IndexedFile::new(
                self.project_id.clone(),
                relative_path,
                full_path,
                &content,
                hash,
                metadata.len(),
                last_modified,
            );

            results.push((indexed_file, content));
            if results.len() >= self.max_files {
                break;
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectWalker;
    use std::path::Path;

    #[test]
    fn ignores_benches_examples_testdata_not_tests() {
        assert!(ProjectWalker::is_ignored(Path::new(
            "crates/foo/benches/hot.rs"
        )));
        assert!(ProjectWalker::is_ignored(Path::new("examples/demo.rs")));
        assert!(ProjectWalker::is_ignored(Path::new("testdata/input.rs")));
        assert!(ProjectWalker::is_ignored(Path::new(".pytest_cache/a.py")));
        assert!(!ProjectWalker::is_ignored(Path::new(
            "crates/foo/src/lib.rs"
        )));
        assert!(!ProjectWalker::is_ignored(Path::new(
            "crates/foo/tests/gold.rs"
        )));
    }
}
