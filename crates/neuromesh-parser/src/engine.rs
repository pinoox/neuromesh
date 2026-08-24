use crate::generic::GenericParser;
use crate::html::HtmlParser;
use crate::python_lang::PythonParser;
use crate::rust_lang::RustParser;
use crate::scss::ScssParser;
use crate::types::AstAnalysisResult;
use crate::typescript::TypeScriptParser;
use crate::vue::VueParser;
use neuromesh_index::SourceLanguage;
use std::path::Path;

pub struct CodeIntelligenceEngine;

impl CodeIntelligenceEngine {
    pub fn analyze(path: &Path, content: &str, language: SourceLanguage) -> AstAnalysisResult {
        match language {
            SourceLanguage::Vue => VueParser::parse(path, content),
            SourceLanguage::TypeScript | SourceLanguage::JavaScript => {
                crate::tree_sitter_lang::parse_typescript(path, content)
                    .unwrap_or_else(|| TypeScriptParser::parse(path, content))
            }
            SourceLanguage::SCSS | SourceLanguage::CSS => ScssParser::parse(path, content),
            SourceLanguage::Rust => crate::tree_sitter_lang::parse_rust(path, content)
                .unwrap_or_else(|| RustParser::parse(path, content)),
            SourceLanguage::Python => PythonParser::parse(path, content),
            SourceLanguage::HTML => HtmlParser::parse(path, content),
            SourceLanguage::Go
            | SourceLanguage::PHP
            | SourceLanguage::Java
            | SourceLanguage::Kotlin
            | SourceLanguage::CSharp
            | SourceLanguage::C
            | SourceLanguage::Cpp => GenericParser::parse(path, content),
            _ => AstAnalysisResult::default(),
        }
    }
}
