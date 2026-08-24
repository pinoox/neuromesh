use crate::generic::GenericParser;
use crate::html::HtmlParser;
use crate::python_lang::PythonParser;
use crate::query_extract::{self, Grammar, QueryOptions, RUST_QUERIES, TYPESCRIPT_QUERIES};
use crate::rust_lang::RustParser;
use crate::scss::ScssParser;
use crate::types::AstAnalysisResult;
use crate::typescript::TypeScriptParser;
use crate::vue::VueParser;
use neuromesh_index::SourceLanguage;
use std::path::Path;

/// Data-driven extractor: tree-sitter queries when a grammar exists, regex otherwise.
#[derive(Clone, Copy)]
pub struct LanguageSpec {
    pub language: SourceLanguage,
    grammar: Option<Grammar>,
    queries: Option<&'static str>,
    options: QueryOptions,
    fallback: Fallback,
}

#[derive(Clone, Copy)]
enum Fallback {
    Rust,
    TypeScript,
    Python,
    Vue,
    Generic,
    Html,
    Scss,
    None,
}

impl LanguageSpec {
    pub fn get(language: SourceLanguage) -> Self {
        match language {
            SourceLanguage::Rust => Self {
                language,
                grammar: Some(Grammar::Rust),
                queries: Some(RUST_QUERIES),
                options: QueryOptions {
                    rust_use: true,
                    skip_cfg_test: true,
                    ts_import: false,
                },
                fallback: Fallback::Rust,
            },
            SourceLanguage::TypeScript | SourceLanguage::JavaScript => Self {
                language,
                grammar: Some(Grammar::TypeScript),
                queries: Some(TYPESCRIPT_QUERIES),
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: true,
                },
                fallback: Fallback::TypeScript,
            },
            SourceLanguage::Python => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: false,
                },
                fallback: Fallback::Python,
            },
            SourceLanguage::Vue => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: false,
                },
                fallback: Fallback::Vue,
            },
            SourceLanguage::SCSS | SourceLanguage::CSS => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: false,
                },
                fallback: Fallback::Scss,
            },
            SourceLanguage::HTML => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: false,
                },
                fallback: Fallback::Html,
            },
            SourceLanguage::Go
            | SourceLanguage::PHP
            | SourceLanguage::Java
            | SourceLanguage::Kotlin
            | SourceLanguage::CSharp
            | SourceLanguage::C
            | SourceLanguage::Cpp => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: false,
                },
                fallback: Fallback::Generic,
            },
            SourceLanguage::JSON
            | SourceLanguage::YAML
            | SourceLanguage::Markdown
            | SourceLanguage::SQL
            | SourceLanguage::Unknown => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions {
                    rust_use: false,
                    skip_cfg_test: false,
                    ts_import: false,
                },
                fallback: Fallback::None,
            },
        }
    }

    pub fn extract(self, path: &Path, content: &str) -> AstAnalysisResult {
        if let (Some(grammar), Some(queries)) = (self.grammar, self.queries) {
            if let Some(ast) = query_extract::parse(path, content, grammar, queries, self.options) {
                return ast;
            }
        }
        self.fallback.parse(path, content)
    }
}

impl Fallback {
    fn parse(self, path: &Path, content: &str) -> AstAnalysisResult {
        match self {
            Fallback::Rust => RustParser::parse(path, content),
            Fallback::TypeScript => TypeScriptParser::parse(path, content),
            Fallback::Python => PythonParser::parse(path, content),
            Fallback::Vue => VueParser::parse(path, content),
            Fallback::Generic => GenericParser::parse(path, content),
            Fallback::Html => HtmlParser::parse(path, content),
            Fallback::Scss => ScssParser::parse(path, content),
            Fallback::None => AstAnalysisResult::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LanguageSpec;
    use neuromesh_index::SourceLanguage;

    #[test]
    fn rust_and_typescript_use_tree_sitter_queries() {
        let rust = LanguageSpec::get(SourceLanguage::Rust);
        assert!(rust.grammar.is_some() && rust.queries.is_some());
        let ts = LanguageSpec::get(SourceLanguage::TypeScript);
        assert!(ts.grammar.is_some() && ts.queries.is_some());
        let kotlin = LanguageSpec::get(SourceLanguage::Kotlin);
        assert!(kotlin.grammar.is_none());
    }
}
