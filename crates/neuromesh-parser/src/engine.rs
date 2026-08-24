use crate::overlay;
use crate::registry::LanguageSpec;
use crate::types::AstAnalysisResult;
use neuromesh_index::SourceLanguage;
use std::path::Path;

pub struct CodeIntelligenceEngine;

impl CodeIntelligenceEngine {
    pub fn analyze(path: &Path, content: &str, language: SourceLanguage) -> AstAnalysisResult {
        let mut ast = LanguageSpec::get(language).extract(path, content);
        overlay::apply(path, content, language, &mut ast);
        ast
    }
}
