use crate::generic::GenericParser;
use crate::html::HtmlParser;
use crate::python_lang::PythonParser;
use crate::query_extract::{
    self, Grammar, QueryOptions, CSHARP_QUERIES, DART_QUERIES, GO_QUERIES, JAVA_QUERIES,
    KOTLIN_QUERIES, PHP_QUERIES, PYTHON_QUERIES, RUBY_QUERIES, RUST_QUERIES, SWIFT_QUERIES,
    TYPESCRIPT_QUERIES,
};
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
                options: QueryOptions::rust(),
                fallback: Fallback::Rust,
            },
            SourceLanguage::TypeScript | SourceLanguage::JavaScript => Self {
                language,
                grammar: Some(Grammar::TypeScript),
                queries: Some(TYPESCRIPT_QUERIES),
                options: QueryOptions::typescript(),
                fallback: Fallback::TypeScript,
            },
            SourceLanguage::Python => Self {
                language,
                grammar: Some(Grammar::Python),
                queries: Some(PYTHON_QUERIES),
                options: QueryOptions::python(),
                fallback: Fallback::Python,
            },
            SourceLanguage::Go => Self {
                language,
                grammar: Some(Grammar::Go),
                queries: Some(GO_QUERIES),
                options: QueryOptions::go(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::Java => Self {
                language,
                grammar: Some(Grammar::Java),
                queries: Some(JAVA_QUERIES),
                options: QueryOptions::java(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::Kotlin => Self {
                language,
                grammar: Some(Grammar::Kotlin),
                queries: Some(KOTLIN_QUERIES),
                options: QueryOptions::kotlin(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::PHP => Self {
                language,
                grammar: Some(Grammar::Php),
                queries: Some(PHP_QUERIES),
                options: QueryOptions::php(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::CSharp => Self {
                language,
                grammar: Some(Grammar::CSharp),
                queries: Some(CSHARP_QUERIES),
                options: QueryOptions::csharp(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::Dart => Self {
                language,
                grammar: Some(Grammar::Dart),
                queries: Some(DART_QUERIES),
                options: QueryOptions::dart(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::Swift => Self {
                language,
                grammar: Some(Grammar::Swift),
                queries: Some(SWIFT_QUERIES),
                options: QueryOptions::swift(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::Ruby => Self {
                language,
                grammar: Some(Grammar::Ruby),
                queries: Some(RUBY_QUERIES),
                options: QueryOptions::ruby(),
                fallback: Fallback::Generic,
            },
            SourceLanguage::Vue | SourceLanguage::Svelte => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions::typescript(),
                fallback: Fallback::Vue,
            },
            SourceLanguage::SCSS | SourceLanguage::CSS => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions::typescript(),
                fallback: Fallback::Scss,
            },
            SourceLanguage::HTML | SourceLanguage::Twig => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions::typescript(),
                fallback: Fallback::Html,
            },
            SourceLanguage::C | SourceLanguage::Cpp => Self {
                language,
                grammar: None,
                queries: None,
                options: QueryOptions::java(),
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
                options: QueryOptions::typescript(),
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
    fn wave2_languages_use_tree_sitter_queries() {
        for language in [
            SourceLanguage::Rust,
            SourceLanguage::TypeScript,
            SourceLanguage::Python,
            SourceLanguage::Go,
            SourceLanguage::Java,
            SourceLanguage::Kotlin,
            SourceLanguage::PHP,
            SourceLanguage::CSharp,
            SourceLanguage::Dart,
            SourceLanguage::Swift,
            SourceLanguage::Ruby,
        ] {
            let spec = LanguageSpec::get(language);
            assert!(
                spec.grammar.is_some() && spec.queries.is_some(),
                "{language:?} should have a query grammar"
            );
        }
        let c = LanguageSpec::get(SourceLanguage::C);
        assert!(c.grammar.is_none());
    }
}
