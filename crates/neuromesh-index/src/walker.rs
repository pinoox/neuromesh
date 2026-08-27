use crate::hasher::ContentHasher;
use crate::tracker::{FileFingerprint, IndexedFile, SourceLanguage};
use chrono::{DateTime, Utc};
use neuromesh_core::{ProjectId, Result};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Indexed files plus counts of skipped unknown extensions (non-binary).
#[derive(Debug, Default)]
pub struct ScanReport {
    pub files: Vec<(IndexedFile, String)>,
    /// Every kept relative path (changed and unchanged).
    pub present: Vec<String>,
    pub unchanged: usize,
    pub skipped_by_extension: BTreeMap<String, usize>,
    /// True when more source files existed than `file_cap`.
    pub truncated: bool,
    pub omitted_over_cap: usize,
    pub file_cap: usize,
    /// True when the cap grew to fit production sources (not `--max-files`).
    pub auto_cap: bool,
    pub hard_cap: usize,
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
    /// `None` = auto: index every non-test source, then tests, up to `hard_cap`.
    max_files: Option<usize>,
    soft_cap: usize,
    hard_cap: usize,
}

impl ProjectWalker {
    pub const SOFT_CAP: usize = 6_000;
    pub const HARD_CAP: usize = 50_000;

    pub fn new(root_path: PathBuf, project_id: ProjectId) -> Self {
        Self {
            root_path,
            project_id,
            max_file_size: 2 * 1024 * 1024, // 2MB max text file
            max_files: None,
            soft_cap: Self::SOFT_CAP,
            hard_cap: Self::HARD_CAP,
        }
    }

    pub fn with_max_files(mut self, max_files: usize) -> Self {
        self.max_files = Some(max_files.max(1));
        self
    }

    /// `None` keeps auto-grow. Used by CLI/MCP/API after reading config.
    pub fn with_optional_max_files(self, max_files: Option<usize>) -> Self {
        match max_files {
            Some(n) => self.with_max_files(n),
            None => self,
        }
    }

    #[cfg(test)]
    fn with_auto_caps(mut self, soft: usize, hard: usize) -> Self {
        self.max_files = None;
        self.soft_cap = soft.max(1);
        self.hard_cap = hard.max(self.soft_cap);
        self
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
                || current.join("artisan").exists()
                || current.join("app.php").exists()
                || current.join("bin").join("pinx").exists()
                || current.join("go.mod").exists()
                || current.join("angular.json").exists()
                || current.join("App.csproj").exists()
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

    /// Honor an explicit MCP/CLI directory instead of walking to a parent git root.
    /// A nested fixture without `package.json` must stay that folder.
    pub fn explicit_workspace(start: &Path) -> PathBuf {
        let dir = if start.is_file() {
            start
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| start.to_path_buf())
        } else {
            start.to_path_buf()
        };
        if dir.is_dir() && Self::is_safe_workspace(&dir) {
            return dir.canonicalize().unwrap_or(dir);
        }
        Self::discover_workspace(&dir)
    }

    pub fn is_safe_workspace(path: &Path) -> bool {
        crate::confine::is_safe_workspace(path)
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
                || s_lower == ".playwright-cli"
                || s_lower == ".playwright"
                || s_lower == ".output"
                || s_lower == "~pinx"
                || s_lower == ".pinx"
            {
                return true;
            }
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            name.as_str(),
            "package-lock.json"
                | "yarn.lock"
                | "pnpm-lock.yaml"
                | "bun.lock"
                | "bun.lockb"
                | "composer.lock"
        )
    }

    pub fn scan(&self) -> Result<Vec<(IndexedFile, String)>> {
        Ok(self.scan_report()?.files)
    }

    pub fn scan_report(&self) -> Result<ScanReport> {
        self.scan_report_with(&HashMap::new())
    }

    /// Walk metadata first. Read+hash only files whose size/mtime miss `known`.
    pub fn scan_report_with(&self, known: &HashMap<String, FileFingerprint>) -> Result<ScanReport> {
        let mut report = ScanReport {
            hard_cap: self.hard_cap,
            auto_cap: self.max_files.is_none(),
            ..ScanReport::default()
        };

        let mut candidates: Vec<(PathBuf, PathBuf, u64, DateTime<Utc>)> = Vec::new();

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
            if crate::confine::path_escapes_workspace(&full_path, &self.root_path) {
                continue;
            }
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

            let last_modified: DateTime<Utc> = metadata
                .modified()
                .map(|t| t.into())
                .unwrap_or_else(|_| Utc::now());
            candidates.push((relative_path, full_path, metadata.len(), last_modified));
        }

        candidates.sort_by(|a, b| {
            is_test_like(&a.0)
                .cmp(&is_test_like(&b.0))
                .then_with(|| a.0.cmp(&b.0))
        });

        let production = candidates
            .iter()
            .filter(|(rel, _, _, _)| !is_test_like(rel))
            .count();
        let cap = match self.max_files {
            Some(n) => n.clamp(1, self.hard_cap),
            None => production.max(self.soft_cap).min(self.hard_cap),
        };
        report.file_cap = cap;

        if candidates.len() > cap {
            report.truncated = true;
            report.omitted_over_cap = candidates.len() - cap;
            candidates.truncate(cap);
        }

        for (relative_path, full_path, byte_size, last_modified) in candidates {
            let rel = relative_path.to_string_lossy().replace('\\', "/");
            report.present.push(rel.clone());
            if let Some(fp) = known.get(&rel) {
                if fp.size == byte_size && fp.mtime_unix == last_modified.timestamp() {
                    report.unchanged += 1;
                    continue;
                }
            }

            let content = match fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let hash = ContentHasher::hash_str(&content);
            let indexed_file = IndexedFile::new(
                self.project_id.clone(),
                relative_path,
                full_path,
                &content,
                hash,
                byte_size,
                last_modified,
            );
            report.files.push((indexed_file, content));
        }

        Ok(report)
    }

    /// Read one workspace file for the live watcher.
    pub fn read_indexed(&self, full_path: &Path) -> Option<(IndexedFile, String)> {
        if Self::is_ignored(full_path) {
            return None;
        }
        if crate::confine::path_escapes_workspace(full_path, &self.root_path) {
            return None;
        }
        let relative_path = full_path.strip_prefix(&self.root_path).ok()?.to_path_buf();
        let language = SourceLanguage::from_path(&relative_path);
        if language == SourceLanguage::Unknown {
            return None;
        }
        let metadata = fs::metadata(full_path).ok()?;
        if metadata.len() > self.max_file_size {
            return None;
        }
        let content = fs::read_to_string(full_path).ok()?;
        let last_modified: DateTime<Utc> = metadata
            .modified()
            .map(|t| t.into())
            .unwrap_or_else(|_| Utc::now());
        let hash = ContentHasher::hash_str(&content);
        Some((
            IndexedFile::new(
                self.project_id.clone(),
                relative_path,
                full_path.to_path_buf(),
                &content,
                hash,
                metadata.len(),
                last_modified,
            ),
            content,
        ))
    }
}

/// Test trees are indexed, but after production sources so a file cap
/// does not drop `HttpKernel.php` in favor of `Tests/`.
fn is_test_like(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.eq_ignore_ascii_case("tests") || s.eq_ignore_ascii_case("test")
    })
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
    use std::fs;
    use std::path::{Path, PathBuf};

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
        assert!(ProjectWalker::is_ignored(Path::new(
            "~pinx/export/app.pinx"
        )));
        assert!(ProjectWalker::is_ignored(Path::new(
            "apps/shop/.pinx/identity.json"
        )));
        assert!(
            !ProjectWalker::is_ignored(Path::new("packages/bench/safeparse.ts")),
            "JS bench/ stays indexed so ranking can deprioritize it vs production"
        );
        assert!(ProjectWalker::is_ignored(Path::new("composer.lock")));
        assert!(ProjectWalker::is_ignored(Path::new("package-lock.json")));
        assert!(!ProjectWalker::is_ignored(Path::new("composer.json")));
    }

    #[test]
    fn file_cap_keeps_sources_ahead_of_tests() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("walk_cap_tmp")
            .join(format!("nm-walk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("src/HttpKernel.php"), "<?php class HttpKernel {}").unwrap();
        fs::write(root.join("tests/a.php"), "<?php class AaaTest {}").unwrap();
        fs::write(root.join("tests/b.php"), "<?php class BbbTest {}").unwrap();
        let walker = ProjectWalker::new(root.clone(), neuromesh_core::ProjectId::new("cap"))
            .with_max_files(1);
        let report = walker.scan_report().unwrap();
        assert!(!report.auto_cap);
        assert!(report.truncated);
        assert_eq!(report.omitted_over_cap, 2);
        assert_eq!(report.files.len(), 1);
        assert!(
            report.files[0]
                .0
                .relative_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("src/HttpKernel.php"),
            "cap must keep production sources, got {:?}",
            report.files[0].0.relative_path
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn explicit_workspace_does_not_walk_to_parent_git() {
        let auth = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/mini-auth")
            .canonicalize()
            .expect("mini-auth fixture");
        assert!(auth.ends_with("mini-auth"), "{}", auth.display());
        let explicit = ProjectWalker::explicit_workspace(&auth);
        let walked = ProjectWalker::discover_workspace(&auth);
        assert!(
            explicit.file_name().is_some_and(|n| n == "mini-auth"),
            "explicit must stay on the fixture, got {}",
            explicit.display()
        );
        assert!(
            walked.file_name().is_some_and(|n| n != "mini-auth"),
            "discover still walks to the git root, got {}",
            walked.display()
        );
    }

    #[test]
    fn auto_cap_grows_to_cover_production_sources() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("walk_cap_tmp")
            .join(format!("nm-walk-auto-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("src/A.php"), "<?php class A {}").unwrap();
        fs::write(root.join("src/B.php"), "<?php class B {}").unwrap();
        fs::write(root.join("src/C.php"), "<?php class C {}").unwrap();
        fs::write(root.join("tests/z.php"), "<?php class Z {}").unwrap();
        let walker = ProjectWalker::new(root.clone(), neuromesh_core::ProjectId::new("auto"))
            .with_auto_caps(2, 10);
        let report = walker.scan_report().unwrap();
        assert!(report.auto_cap);
        assert_eq!(report.file_cap, 3, "auto cap should match production count");
        assert_eq!(report.files.len(), 3);
        assert!(report.truncated);
        assert_eq!(report.omitted_over_cap, 1);
        assert!(report
            .files
            .iter()
            .all(|(f, _)| !f.relative_path.to_string_lossy().contains("tests")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn metadata_walk_skips_unchanged_files() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("walk_cap_tmp")
            .join(format!("nm-walk-incr-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.rs"), "pub fn a() {}\n").unwrap();
        fs::write(root.join("src/b.rs"), "pub fn b() {}\n").unwrap();
        let walker = ProjectWalker::new(root.clone(), neuromesh_core::ProjectId::new("incr"));

        let first = walker.scan_report().unwrap();
        assert_eq!(first.files.len(), 2);
        assert_eq!(first.unchanged, 0);
        assert_eq!(first.present.len(), 2);

        let known: std::collections::HashMap<String, crate::tracker::FileFingerprint> = first
            .files
            .iter()
            .map(|(f, _)| {
                (
                    f.relative_path.to_string_lossy().replace('\\', "/"),
                    f.fingerprint(),
                )
            })
            .collect();

        let second = walker.scan_report_with(&known).unwrap();
        assert!(
            second.files.is_empty(),
            "unchanged files must not be read again"
        );
        assert_eq!(second.unchanged, 2);
        assert_eq!(
            second.present.len(),
            2,
            "present still lists every kept file"
        );

        fs::write(root.join("src/a.rs"), "pub fn a() { let _ = 1; }\n").unwrap();
        let third = walker.scan_report_with(&known).unwrap();
        assert_eq!(third.files.len(), 1, "only the edited file is re-read");
        assert!(third.files[0]
            .0
            .relative_path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("src/a.rs"));
        assert_eq!(third.unchanged, 1);
        let _ = fs::remove_dir_all(&root);
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
