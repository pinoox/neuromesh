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
}

impl ProjectWalker {
    pub fn new(root_path: PathBuf, project_id: ProjectId) -> Self {
        Self {
            root_path,
            project_id,
            max_file_size: 2 * 1024 * 1024, // 2MB max text file
        }
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
        }

        Ok(results)
    }
}
