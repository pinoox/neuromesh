use crate::hasher::ContentHasher;
use crate::tracker::{IndexedFile, SourceLanguage};
use chrono::{DateTime, Utc};
use neuromesh_core::{ProjectId, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Indexed files plus counts of skipped unknown extensions (non-binary).
#[derive(Debug, Default)]
pub struct ScanReport {
    pub files: Vec<(IndexedFile, String)>,
    pub skipped_by_extension: BTreeMap<String, usize>,
}

impl ScanReport {
    pub fn skipped_count(&self) -> usize {
        self.skipped_by_extension.values().copied().sum()
    }

    pub fn skipped_summary(&self) -> String {
        let mut parts: Vec<(&String, &usize)> = self.skipped_by_extension.iter().collect();
        parts.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        parts
            .into_iter()
            .take(6)
            .map(|(ext, n)| format!(".{ext}: {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

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
                || current.join("settings.gradle.kts").exists()
                || current.join("settings.gradle").exists()
                || current.join("pubspec.yaml").exists()
                || current.join("Package.swift").exists()
                || current.join("Gemfile").exists()
                || current.join("composer.json").exists()
                || current.join("app.php").exists()
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
                || s_lower == ".dart_tool"
                || s_lower == "pods"
                || s_lower == ".build"
                || s_lower == ".svelte-kit"
            {
                return true;
            }
        }
        false
    }

    pub fn scan(&self) -> Result<Vec<(IndexedFile, String)>> {
        Ok(self.scan_report()?.files)
    }

    pub fn scan_report(&self) -> Result<ScanReport> {
        let mut report = ScanReport::default();

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
                if let Some(ext) = reportable_unknown_extension(&relative_path) {
                    *report.skipped_by_extension.entry(ext).or_insert(0) += 1;
                }
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

            report.files.push((indexed_file, content));
            if report.files.len() >= self.max_files {
                break;
            }
        }

        Ok(report)
    }
}

fn reportable_unknown_extension(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;
    if ext.is_empty() || is_noise_extension(&ext) {
        return None;
    }
    Some(ext)
}

fn is_noise_extension(ext: &str) -> bool {
    matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "bmp"
            | "tif"
            | "tiff"
            | "svg"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "eot"
            | "mp3"
            | "mp4"
            | "wav"
            | "ogg"
            | "webm"
            | "zip"
            | "jar"
            | "aar"
            | "apk"
            | "so"
            | "dll"
            | "exe"
            | "o"
            | "a"
            | "lib"
            | "class"
            | "dex"
            | "bin"
            | "dat"
            | "pdf"
            | "7z"
            | "rar"
            | "gz"
            | "tgz"
            | "bz2"
            | "xz"
            | "wasm"
            | "pdb"
            | "dylib"
            | "pyc"
            | "pyo"
            | "rlib"
            | "rmeta"
    )
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

    #[test]
    fn skipped_summary_lists_unknown_code_extensions() {
        let mut report = super::ScanReport::default();
        report.skipped_by_extension.insert("swift".into(), 8);
        report.skipped_by_extension.insert("proto".into(), 4);
        assert_eq!(report.skipped_count(), 12);
        assert_eq!(report.skipped_summary(), ".swift: 8, .proto: 4");
        assert_eq!(
            super::reportable_unknown_extension(Path::new("Foo.swift")).as_deref(),
            Some("swift")
        );
        assert_eq!(
            super::reportable_unknown_extension(Path::new("icon.png")),
            None
        );
        assert_eq!(
            crate::tracker::SourceLanguage::from_path(Path::new("SmsStore.kt")),
            crate::tracker::SourceLanguage::Kotlin
        );
    }
}
