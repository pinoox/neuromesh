use crate::registry::LanguageSpec;
use crate::types::AstAnalysisResult;
use neuromesh_index::SourceLanguage;
use std::path::Path;

pub struct CodeIntelligenceEngine;

impl CodeIntelligenceEngine {
    pub fn analyze(path: &Path, content: &str, language: SourceLanguage) -> AstAnalysisResult {
        LanguageSpec::get(language).extract(path, content)
    }
}
