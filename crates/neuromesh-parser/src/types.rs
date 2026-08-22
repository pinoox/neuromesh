use neuromesh_core::{EdgeType, NodeType};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSymbol {
    pub name: String,
    pub symbol_type: NodeType,
    pub signature: Option<String>,
    pub line_range: Range<usize>,
    pub docstring: Option<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedImport {
    pub source_path: String,
    pub imported_symbols: Vec<String>,
    pub is_default: bool,
    pub is_namespace: bool,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRelationship {
    pub source_symbol: String,
    pub target_symbol: String,
    pub relationship: EdgeType,
    pub target_file_hint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AstAnalysisResult {
    pub symbols: Vec<ParsedSymbol>,
    pub imports: Vec<ParsedImport>,
    pub exports: Vec<String>,
    pub relationships: Vec<ParsedRelationship>,
    pub design_tokens: Vec<String>,
}
