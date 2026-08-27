use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct VueParser;

impl VueParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("AnonymousComponent");

        // 1. Register the component itself
        result.symbols.push(ParsedSymbol::new(
            filename,
            NodeType::Component,
            Some(format!("<{} />", filename)),
            1..content.lines().count() + 1,
            true,
        ));

        // 2. Extract imports from <script> blocks
        let import_regex = Regex::new(
            r#"(?m)^\s*import\s+(?:(?:(?:type\s+)?(\w+)|(?:\{\s*([^}]+)\s*\}))\s+from\s+)?['"]([^'"]+)['"];?"#,
        )
        .unwrap();

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(caps) = import_regex.captures(line) {
                let default_import = caps.get(1).map(|m| m.as_str().trim());
                let named_imports = caps.get(2).map(|m| m.as_str());
                let source_path = caps.get(3).map(|m| m.as_str()).unwrap_or("");

                let mut imported_symbols = Vec::new();
                if let Some(def) = default_import {
                    if !def.is_empty() {
                        imported_symbols.push(def.to_string());
                    }
                }
                if let Some(named) = named_imports {
                    for sym in named.split(',') {
                        let clean = sym.split_whitespace().next_back().unwrap_or("").trim();
                        if !clean.is_empty() {
                            imported_symbols.push(clean.to_string());
                        }
                    }
                }

                result.imports.push(ParsedImport {
                    source_path: source_path.to_string(),
                    imported_symbols: imported_symbols.clone(),
                    is_default: default_import.is_some(),
                    is_namespace: false,
                    line_number: line_idx + 1,
                });

                for sym in imported_symbols {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: sym,
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(vue_import_hint(source_path)),
                        receiver_hint: None,
                    });
                }
            }
        }

        // 3. Extract Pinia stores and composables
        let store_regex = Regex::new(r"const\s+\w+\s*=\s*(use\w+Store)\s*\(").unwrap();
        let composable_regex = Regex::new(r"const\s+[^=]+=\s*(use[A-Z]\w+)\s*\(").unwrap();

        for cap in store_regex.captures_iter(content) {
            if let Some(store_hook) = cap.get(1) {
                let hook = store_hook.as_str();
                result.relationships.push(ParsedRelationship {
                    source_symbol: filename.to_string(),
                    target_symbol: hook.to_string(),
                    relationship: EdgeType::DependsOn,
                    target_file_hint: Some(pinia_store_hint(hook)),
                    receiver_hint: None,
                });
            }
        }

        for cap in composable_regex.captures_iter(content) {
            if let Some(composable) = cap.get(1) {
                let name = composable.as_str();
                if !name.ends_with("Store") {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: name.to_string(),
                        relationship: EdgeType::Calls,
                        target_file_hint: None,
                        receiver_hint: None,
                    });
                }
            }
        }

        // 4. Extract Template Component Usages (PascalCase + kebab-case PrimeVue)
        let tag_regex =
            Regex::new(r"<([A-Z][a-zA-Z0-9]+|[a-z][a-z0-9]*(?:-[a-z0-9]+)+)[\s/>]").unwrap();
        for cap in tag_regex.captures_iter(content) {
            if let Some(tag) = cap.get(1) {
                let child_component = vue_tag_to_component(tag.as_str());
                if child_component != filename && !is_native_html_tag(tag.as_str()) {
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: child_component.clone(),
                        relationship: EdgeType::Contains,
                        target_file_hint: Some(format!("{child_component}.vue")),
                        receiver_hint: None,
                    });
                    if tag.as_str().contains('-')
                        && !result.symbols.iter().any(|s| s.name == child_component)
                    {
                        result.symbols.push(ParsedSymbol::new(
                            &child_component,
                            NodeType::Component,
                            Some(format!("<{} />", tag.as_str())),
                            1..2,
                            false,
                        ));
                    }
                }
            }
        }

        // 4b. Template handlers (@click="ui.goCart()") -> store action calls
        let mut store_aliases: HashMap<String, String> = HashMap::new();
        let alias_store_regex = Regex::new(r"const\s+(\w+)\s*=\s*(use\w+Store)\s*\(").unwrap();
        for cap in alias_store_regex.captures_iter(content) {
            if let (Some(alias), Some(hook)) = (cap.get(1), cap.get(2)) {
                store_aliases.insert(alias.as_str().to_string(), pinia_store_hint(hook.as_str()));
            }
        }
        let handler_regex =
            Regex::new(r#"(?:@[A-Za-z][\w:.$-]*|v-on:[A-Za-z][\w:.$-]*)\s*=\s*["']([^"']+)["']"#)
                .unwrap();
        let action_call_regex = Regex::new(r"(\w+)\.(\w+)\s*\(").unwrap();
        let action_ref_regex = Regex::new(r"^(\w+)\.(\w+)$").unwrap();
        let push_store_action = |alias: &str, action: &str, result: &mut AstAnalysisResult| {
            if action.is_empty() || !store_aliases.contains_key(alias) {
                return;
            }
            result.relationships.push(ParsedRelationship {
                source_symbol: filename.to_string(),
                target_symbol: action.to_string(),
                relationship: EdgeType::Calls,
                target_file_hint: store_aliases.get(alias).cloned(),
                receiver_hint: Some(alias.to_string()),
            });
        };
        for cap in handler_regex.captures_iter(content) {
            let Some(expr) = cap.get(1) else {
                continue;
            };
            let expr = expr.as_str().trim();
            if let Some(am) = action_call_regex.captures(expr) {
                push_store_action(
                    am.get(1).map(|m| m.as_str()).unwrap_or(""),
                    am.get(2).map(|m| m.as_str()).unwrap_or(""),
                    &mut result,
                );
            } else if let Some(am) = action_ref_regex.captures(expr) {
                push_store_action(
                    am.get(1).map(|m| m.as_str()).unwrap_or(""),
                    am.get(2).map(|m| m.as_str()).unwrap_or(""),
                    &mut result,
                );
            }
        }

        if let Some(script) = extract_script_block(content) {
            for am in action_call_regex.captures_iter(script) {
                push_store_action(
                    am.get(1).map(|m| m.as_str()).unwrap_or(""),
                    am.get(2).map(|m| m.as_str()).unwrap_or(""),
                    &mut result,
                );
            }
        }

        // 5. Extract SCSS @use / @import / Design tokens from <style>
        let scss_import_regex = Regex::new(r#"(?m)@(use|import)\s+['"]([^'"]+)['"]"#).unwrap();
        for cap in scss_import_regex.captures_iter(content) {
            if let Some(source) = cap.get(2) {
                result.imports.push(ParsedImport {
                    source_path: source.as_str().to_string(),
                    imported_symbols: vec!["styles".into()],
                    is_default: false,
                    is_namespace: false,
                    line_number: 1,
                });
                result.relationships.push(ParsedRelationship {
                    source_symbol: filename.to_string(),
                    target_symbol: source.as_str().to_string(),
                    relationship: EdgeType::References,
                    target_file_hint: Some(source.as_str().to_string()),
                    receiver_hint: None,
                });
            }
        }

        let token_regex = Regex::new(r"(\$[-a-zA-Z0-9_]+|--[-a-zA-Z0-9_]+)").unwrap();
        for cap in token_regex.captures_iter(content) {
            if let Some(token) = cap.get(1) {
                let token_str = token.as_str().to_string();
                if !result.design_tokens.contains(&token_str) {
                    result.design_tokens.push(token_str);
                }
            }
        }

        result
    }
}

fn vue_import_hint(source_path: &str) -> String {
    let trimmed = source_path.trim();
    if Path::new(trimmed)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| !e.is_empty())
    {
        return trimmed.to_string();
    }
    format!("{trimmed}.ts")
}

fn pinia_store_hint(hook: &str) -> String {
    let stem = hook
        .strip_prefix("use")
        .unwrap_or(hook)
        .strip_suffix("Store")
        .unwrap_or(hook);
    let mut out = String::new();
    for (i, ch) in stem.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        hook.to_string()
    } else {
        format!("stores/{out}")
    }
}

fn extract_script_block(content: &str) -> Option<&str> {
    let open = content.find("<script")?;
    let after_open = &content[open..];
    let start = after_open.find('>')? + 1;
    let body = &after_open[start..];
    let end = body.find("</script>")?;
    Some(&body[..end])
}

fn vue_tag_to_component(tag: &str) -> String {
    if !tag.contains('-') {
        return tag.to_string();
    }
    tag.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn is_native_html_tag(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "article"
            | "aside"
            | "audio"
            | "b"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "canvas"
            | "caption"
            | "code"
            | "col"
            | "colgroup"
            | "data"
            | "datalist"
            | "dd"
            | "details"
            | "dialog"
            | "div"
            | "dl"
            | "dt"
            | "em"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hr"
            | "html"
            | "i"
            | "iframe"
            | "img"
            | "input"
            | "label"
            | "legend"
            | "li"
            | "link"
            | "main"
            | "map"
            | "meta"
            | "nav"
            | "object"
            | "ol"
            | "optgroup"
            | "option"
            | "p"
            | "path"
            | "picture"
            | "pre"
            | "progress"
            | "script"
            | "section"
            | "select"
            | "slot"
            | "small"
            | "source"
            | "span"
            | "strong"
            | "style"
            | "sub"
            | "summary"
            | "sup"
            | "svg"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "time"
            | "title"
            | "tr"
            | "track"
            | "ul"
            | "video"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_click_extracts_store_action_call() {
        let src = r#"<script setup>
const ui = useUiStore()
</script>
<template>
  <button @click="ui.goCart()">Cart</button>
</template>
"#;
        let ast = VueParser::parse(std::path::Path::new("Header.vue"), src);
        assert!(
            ast.relationships.iter().any(|r| {
                r.target_symbol == "goCart"
                    && r.relationship == EdgeType::Calls
                    && r.target_file_hint.as_deref() == Some("stores/ui")
            }),
            "expected goCart call edge, rels={:?}",
            ast.relationships
        );
    }

    #[test]
    fn template_view_binding_extracts_store_action_call() {
        let src = r#"<script setup>
const products = useProductsStore()
</script>
<template>
  <ProductGrid @view="products.openProduct" />
</template>
"#;
        let ast = VueParser::parse(std::path::Path::new("App.vue"), src);
        assert!(
            ast.relationships.iter().any(|r| {
                r.target_symbol == "openProduct"
                    && r.relationship == EdgeType::Calls
                    && r.receiver_hint.as_deref() == Some("products")
            }),
            "expected openProduct call edge, rels={:?}",
            ast.relationships
        );
    }

    #[test]
    fn script_setup_extracts_store_action_call() {
        let src = r#"<script setup>
const ui = useUiStore()
ui.showToast('saved')
</script>
<template><div /></template>
"#;
        let ast = VueParser::parse(std::path::Path::new("CheckoutView.vue"), src);
        assert!(
            ast.relationships
                .iter()
                .any(|r| { r.target_symbol == "showToast" && r.relationship == EdgeType::Calls }),
            "expected showToast call edge, rels={:?}",
            ast.relationships
        );
    }
}
