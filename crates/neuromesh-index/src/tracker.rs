use chrono::{DateTime, Utc};
use neuromesh_core::{ProjectId, TokenCounter};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    Vue,
    TypeScript,
    JavaScript,
    SCSS,
    CSS,
    Less,
    Rust,
    Python,
    Go,
    PHP,
    Java,
    Kotlin,
    CSharp,
    Swift,
    Dart,
    Ruby,
    C,
    Cpp,
    JSON,
    YAML,
    Markdown,
    HTML,
    Svg,
    Twig,
    Svelte,
    Astro,
    SQL,
    Unknown,
}

impl SourceLanguage {
    pub fn from_path(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_lowercase();

        if filename == "cargo.toml" || filename == "cargo.lock" {
            return SourceLanguage::Rust;
        }
        if filename == "package.json" || filename == "tsconfig.json" {
            return SourceLanguage::JSON;
        }

        match extension.as_str() {
            "vue" => SourceLanguage::Vue,
            "ts" | "tsx" | "mts" | "cts" => SourceLanguage::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => SourceLanguage::JavaScript,
            "scss" | "sass" => SourceLanguage::SCSS,
            "css" => SourceLanguage::CSS,
            "less" => SourceLanguage::Less,
            "rs" => SourceLanguage::Rust,
            "py" | "pyw" => SourceLanguage::Python,
            "go" => SourceLanguage::Go,
            "php" => SourceLanguage::PHP,
            "java" => SourceLanguage::Java,
            "kt" | "kts" => SourceLanguage::Kotlin,
            "cs" => SourceLanguage::CSharp,
            "swift" => SourceLanguage::Swift,
            "dart" => SourceLanguage::Dart,
            "rb" | "rake" => SourceLanguage::Ruby,
            "c" | "h" => SourceLanguage::C,
            "cpp" | "hpp" | "cc" | "cxx" => SourceLanguage::Cpp,
            "json" => SourceLanguage::JSON,
            "yaml" | "yml" => SourceLanguage::YAML,
            "md" | "markdown" => SourceLanguage::Markdown,
            "html" | "htm" | "cshtml" | "razor" => SourceLanguage::HTML,
            "svg" => SourceLanguage::Svg,
            "twig" => SourceLanguage::Twig,
            "svelte" => SourceLanguage::Svelte,
            "astro" => SourceLanguage::Astro,
            "sql" => SourceLanguage::SQL,
            _ => SourceLanguage::Unknown,
        }
    }

    pub fn is_code(&self) -> bool {
        !matches!(self, SourceLanguage::Unknown | SourceLanguage::Markdown)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vue => "vue",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::SCSS => "scss",
            Self::CSS => "css",
            Self::Less => "less",
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Go => "go",
            Self::PHP => "php",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::CSharp => "csharp",
            Self::Swift => "swift",
            Self::Dart => "dart",
            Self::Ruby => "ruby",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::JSON => "json",
            Self::YAML => "yaml",
            Self::Markdown => "markdown",
            Self::HTML => "html",
            Self::Svg => "svg",
            Self::Twig => "twig",
            Self::Svelte => "svelte",
            Self::Astro => "astro",
            Self::SQL => "sql",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub project_id: ProjectId,
    pub relative_path: PathBuf,
    pub full_path: PathBuf,
    pub blake3_hash: String,
    pub byte_size: u64,
    pub token_count: usize,
    pub language: SourceLanguage,
    pub last_modified: DateTime<Utc>,
}

impl IndexedFile {
    pub fn new(
        project_id: ProjectId,
        relative_path: PathBuf,
        full_path: PathBuf,
        content: &str,
        hash: String,
        byte_size: u64,
        last_modified: DateTime<Utc>,
    ) -> Self {
        let language = SourceLanguage::from_path(&relative_path);
        let token_count = TokenCounter::count_tokens(content);

        Self {
            project_id,
            relative_path,
            full_path,
            blake3_hash: hash,
            byte_size,
            token_count,
            language,
            last_modified,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SourceLanguage;
    use std::path::Path;

    #[test]
    fn kotlin_extensions_are_source() {
        assert_eq!(
            SourceLanguage::from_path(Path::new("app/src/main/java/SmsStore.kt")),
            SourceLanguage::Kotlin
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("build.gradle.kts")),
            SourceLanguage::Kotlin
        );
        assert_eq!(SourceLanguage::Kotlin.as_str(), "kotlin");
        assert!(SourceLanguage::Kotlin.is_code());
        assert_eq!(
            SourceLanguage::from_path(Path::new("SmsStore.swift")),
            SourceLanguage::Swift
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("lib/sms_store.dart")),
            SourceLanguage::Dart
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("app/sms_store.rb")),
            SourceLanguage::Ruby
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("theme/home.twig")),
            SourceLanguage::Twig
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("src/SmsCard.svelte")),
            SourceLanguage::Svelte
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("src/pages/sms.astro")),
            SourceLanguage::Astro
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("Pages/Sms.cshtml")),
            SourceLanguage::HTML
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("Inbox.razor")),
            SourceLanguage::HTML
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("styles/sms.less")),
            SourceLanguage::Less
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("assets/sms-inbox.svg")),
            SourceLanguage::Svg
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("theme/inbox.html")),
            SourceLanguage::HTML
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("theme/badge.css")),
            SourceLanguage::CSS
        );
        assert!(SourceLanguage::Less.is_code());
        assert!(SourceLanguage::Svg.is_code());
        assert!(SourceLanguage::Twig.is_code());
        assert!(SourceLanguage::Svelte.is_code());
        assert!(SourceLanguage::Astro.is_code());
    }
}
