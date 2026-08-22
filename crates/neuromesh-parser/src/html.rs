use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::NodeType;
use regex::Regex;
use std::path::Path;

pub struct HtmlParser;

impl HtmlParser {
    pub fn parse(_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();

        // 1. Extract IDs and Named Containers
        let id_regex = Regex::new(r#"(?i)<(?:section|div|form|main|header|footer|aside|table|button)\s+[^>]*id=["']([^"']+)["']"#).unwrap();
        for cap in id_regex.captures_iter(content) {
            let id_name = cap[1].to_string();
            result.symbols.push(ParsedSymbol::new(
                format!("#{}", id_name),
                NodeType::Component,
                Some(format!("<div id=\"{}\">", id_name)),
                1..2,
                true,
            ));
        }

        // 2. Extract Embedded JavaScript Functions inside <script>
        let script_regex = Regex::new(r"(?s)<script\b[^>]*>(.*?)</script>").unwrap();
        let fn_regex = Regex::new(r"(?m)(?:function\s+([A-Za-z0-9_$]+)|(?:const|let|var)\s+([A-Za-z0-9_$]+)\s*=\s*(?:function|\([^)]*\)\s*=>))").unwrap();

        for script_match in script_regex.captures_iter(content) {
            let script_body = &script_match[1];
            for fn_cap in fn_regex.captures_iter(script_body) {
                let fn_name = fn_cap
                    .get(1)
                    .or_else(|| fn_cap.get(2))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                if !fn_name.is_empty() {
                    result.symbols.push(ParsedSymbol::new(
                        fn_name.clone(),
                        NodeType::Function,
                        Some(format!("function {}()", fn_name)),
                        1..2,
                        true,
                    ));
                }
            }
        }

        // 3. Extract Embedded CSS Class Selectors
        let style_regex = Regex::new(r"(?s)<style\b[^>]*>(.*?)</style>").unwrap();
        let class_regex = Regex::new(r"(?m)^\s*\.([A-Za-z0-9_-]+)\s*\{").unwrap();

        for style_match in style_regex.captures_iter(content) {
            let style_body = &style_match[1];
            for class_cap in class_regex.captures_iter(style_body) {
                let class_name = class_cap[1].to_string();
                let token_name = format!(".{}", class_name);
                result.design_tokens.push(token_name.clone());
                result.symbols.push(ParsedSymbol::new(
                    token_name.clone(),
                    NodeType::StyleToken,
                    Some(format!(".{} {{", class_name)),
                    1..2,
                    false,
                ));
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_html_embedded_scripts_and_styles() {
        let html = r#"
            <!DOCTYPE html>
            <html>
              <head>
                <style>
                  .product-card { padding: 1rem; }
                  .header-nav { display: flex; }
                </style>
              </head>
              <body>
                <header id="main-header"></header>
                <main id="catalog-section"></main>
                <script>
                  function renderCatalog() { console.log('rendering'); }
                  const toggleCart = () => { console.log('cart'); };
                </script>
              </body>
            </html>
        "#;

        let res = HtmlParser::parse(Path::new("shop.html"), html);
        assert!(res.symbols.iter().any(|s| s.name == "#main-header"));
        assert!(res.symbols.iter().any(|s| s.name == "renderCatalog"));
        assert!(res.symbols.iter().any(|s| s.name == "toggleCart"));
        assert!(res.symbols.iter().any(|s| s.name == ".product-card"));
    }
}
