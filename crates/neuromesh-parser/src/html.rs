use crate::types::{AstAnalysisResult, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct HtmlParser;

impl HtmlParser {
    pub fn parse(path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("markup");

        extract_element_ids(&mut result, path, content);
        extract_svg_uses(&mut result, filename, content);
        extract_script_functions(&mut result, content);
        extract_embedded_classes(&mut result, content);

        result
    }
}

fn extract_element_ids(result: &mut AstAnalysisResult, path: &Path, content: &str) {
    static HTML_ID_RE: OnceLock<Regex> = OnceLock::new();
    static SVG_ID_RE: OnceLock<Regex> = OnceLock::new();
    let is_svg = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
    let id_re = if is_svg {
        SVG_ID_RE.get_or_init(|| {
            Regex::new(r#"(?is)<([a-z][a-z0-9:-]*)\b[^>]*\bid=["']([^"']+)["']"#).unwrap()
        })
    } else {
        HTML_ID_RE.get_or_init(|| {
            Regex::new(r#"(?i)<(?:section|div|form|main|header|footer|aside|table|button|nav|article)\s+[^>]*id=["']([^"']+)["']"#).unwrap()
        })
    };
    for cap in id_re.captures_iter(content) {
        let (tag, id_name) = if is_svg {
            (
                cap.get(1).unwrap().as_str().to_ascii_lowercase(),
                cap.get(2).unwrap().as_str(),
            )
        } else {
            ("div".to_string(), cap.get(1).unwrap().as_str())
        };
        if id_name.is_empty() || result.symbols.iter().any(|s| s.name == id_name) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        result.symbols.push(ParsedSymbol::new(
            id_name.to_string(),
            NodeType::Component,
            Some(format!("<{tag} id=\"{id_name}\">")),
            line..(line + 1),
            true,
        ));
    }
}

fn extract_svg_uses(result: &mut AstAnalysisResult, filename: &str, content: &str) {
    static USE_RE: OnceLock<Regex> = OnceLock::new();
    let use_re = USE_RE.get_or_init(|| {
        Regex::new(r#"(?is)<use\b[^>]*(?:href|xlink:href)=["']#([^"']+)["']"#).unwrap()
    });
    for cap in use_re.captures_iter(content) {
        let target = cap.get(1).unwrap().as_str();
        if target.is_empty() {
            continue;
        }
        result.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: target.to_string(),
            relationship: EdgeType::References,
            target_file_hint: None,
            receiver_hint: None,
        });
    }
}

fn extract_script_functions(result: &mut AstAnalysisResult, content: &str) {
    static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    static FN_RE: OnceLock<Regex> = OnceLock::new();
    let script_re =
        SCRIPT_RE.get_or_init(|| Regex::new(r"(?is)<script\b[^>]*>(.*?)</script>").unwrap());
    let fn_re = FN_RE.get_or_init(|| {
        Regex::new(r"(?m)(?:function\s+([A-Za-z0-9_$]+)|(?:const|let|var)\s+([A-Za-z0-9_$]+)\s*=\s*(?:function|\([^)]*\)\s*=>))")
            .unwrap()
    });

    for script_match in script_re.captures_iter(content) {
        let script_body = &script_match[1];
        let script_start = script_match.get(1).map(|m| m.start()).unwrap_or(0);
        for fn_cap in fn_re.captures_iter(script_body) {
            let fn_name = fn_cap
                .get(1)
                .or_else(|| fn_cap.get(2))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if fn_name.is_empty() {
                continue;
            }
            let line = line_of(
                content,
                script_start + fn_cap.get(0).map(|m| m.start()).unwrap_or(0),
            );
            result.symbols.push(ParsedSymbol::new(
                fn_name.clone(),
                NodeType::Function,
                Some(format!("function {fn_name}()")),
                line..(line + 1),
                true,
            ));
        }
    }
}

fn extract_embedded_classes(result: &mut AstAnalysisResult, content: &str) {
    static STYLE_RE: OnceLock<Regex> = OnceLock::new();
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    let style_re =
        STYLE_RE.get_or_init(|| Regex::new(r"(?is)<style\b[^>]*>(.*?)</style>").unwrap());
    let class_re = CLASS_RE.get_or_init(|| Regex::new(r"(?m)^\s*\.([A-Za-z0-9_-]+)\s*\{").unwrap());

    for style_match in style_re.captures_iter(content) {
        let style_body = &style_match[1];
        let style_start = style_match.get(1).map(|m| m.start()).unwrap_or(0);
        for class_cap in class_re.captures_iter(style_body) {
            let class_name = class_cap[1].to_string();
            result.design_tokens.push(format!(".{class_name}"));
            let line = line_of(
                content,
                style_start + class_cap.get(0).map(|m| m.start()).unwrap_or(0),
            );
            result.symbols.push(ParsedSymbol::new(
                class_name.clone(),
                NodeType::StyleToken,
                Some(format!(".{class_name} {{")),
                line..(line + 1),
                false,
            ));
        }
    }
}

fn line_of(content: &str, byte: usize) -> usize {
    content
        .get(..byte)
        .map(|head| head.bytes().filter(|b| *b == b'\n').count() + 1)
        .unwrap_or(1)
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
        assert!(res.symbols.iter().any(|s| s.name == "main-header"));
        assert!(res.symbols.iter().any(|s| s.name == "renderCatalog"));
        assert!(res.symbols.iter().any(|s| s.name == "toggleCart"));
        assert!(res.symbols.iter().any(|s| s.name == "product-card"));
    }

    #[test]
    fn svg_extracts_symbol_id_and_use_href() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
  <symbol id="smsInbox" viewBox="0 0 24 24"><path d="M2 4h20"/></symbol>
  <use href="#smsInbox" x="0" y="0"/>
</svg>"##;
        let res = HtmlParser::parse(Path::new("assets/sms-inbox.svg"), svg);
        assert!(
            res.symbols.iter().any(|s| s.name == "smsInbox"),
            "symbols = {:?}",
            res.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(res
            .relationships
            .iter()
            .any(|r| r.target_symbol == "smsInbox"));
    }
}
