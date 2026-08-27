use crate::activator::SeedSink;
use neuromesh_core::{TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashSet;

const STYLE_KEYWORDS: &[&str] = &[
    "scss",
    "sass",
    "stylesheet",
    "style sheet",
    "design token",
    "tokens and mixins",
    "tokens/mixins",
    "mixins",
    "mixin",
    "hover-lift",
    "focus-within",
    "price-card",
    "_tokens.",
    "_mixins.",
];

pub fn is_style_task(signature: &TaskSignature) -> bool {
    if signature.style.is_some() {
        return true;
    }
    let lower = signature.raw_prompt.to_lowercase();
    STYLE_KEYWORDS.iter().any(|k| lower.contains(k))
}

pub fn is_style_path(path: &std::path::Path) -> bool {
    let p = path.to_string_lossy().replace('\\', "/").to_lowercase();
    p.contains("/styles/")
        || p.ends_with(".scss")
        || p.ends_with(".sass")
        || p.ends_with(".less")
        || p.contains("_tokens.")
        || p.contains("_mixins.")
        || p.contains("tokens.scss")
        || p.contains("mixins.scss")
}

pub(crate) fn inject_style_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_>,
) {
    if !is_style_task(signature) {
        return;
    }

    let style_ext = signature.style.as_deref().map(|s| s.to_ascii_lowercase());
    for hint in style_file_hints(&style_ext) {
        if let Some(id) = graph.resolve_file_hint(hint) {
            sink.push(graph, prompt, hint.to_string(), 0.95, "style_hint");
            let _ = id;
        }
    }

    let lower = signature.raw_prompt.to_lowercase();
    if lower.contains("productcard")
        || lower.contains("product card")
        || lower.contains("price-card")
    {
        sink.push(graph, prompt, "ProductCard".into(), 0.9, "style_component");
    }
}

fn style_file_hints(style: &Option<String>) -> Vec<&'static str> {
    match style.as_deref() {
        Some("scss") | Some("sass") => vec![
            "src/styles/_tokens.scss",
            "src/styles/tokens.scss",
            "src/styles/_mixins.scss",
            "src/styles/mixins.scss",
            "styles/_tokens.scss",
            "styles/_mixins.scss",
        ],
        Some("less") => vec!["styles/tokens.less", "src/styles/tokens.less"],
        Some("css") => vec!["styles/tokens.css", "src/styles/tokens.css"],
        _ => vec![
            "src/styles/_tokens.scss",
            "src/styles/_mixins.scss",
            "styles/_tokens.scss",
            "styles/_mixins.scss",
        ],
    }
}

pub(crate) fn inject_view_component_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_>,
) {
    let lower = signature.raw_prompt.to_lowercase();
    if !lower.contains("checkout")
        && !lower.contains("cartview")
        && !lower.contains("productcard")
        && !lower.contains("product card")
    {
        return;
    }
    let mut candidates: HashSet<String> = HashSet::new();
    for ident in &signature.identifiers {
        if ident.ends_with("View") || ident.ends_with("Component") {
            candidates.insert(ident.clone());
        }
    }
    for word in ["checkout", "cart", "product", "home", "header"] {
        if lower.contains(word) {
            candidates.insert(format!("{}View", pascal_case(word)));
        }
    }
    for name in candidates {
        if name.len() < 5 {
            continue;
        }
        sink.push(graph, prompt, name, 0.82, "view_component");
    }
}

pub fn style_noise_penalty(path: &std::path::Path, signature: &TaskSignature) -> f32 {
    if !is_style_task(signature) {
        return 0.0;
    }
    let p = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if is_style_path(path) {
        return 0.0;
    }
    if p.contains("promo")
        || p.contains("cartdrawer")
        || p.contains("cartview")
        || p.contains("/stores/cart")
    {
        return 28.0;
    }
    0.0
}

fn pascal_case(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| c.is_alphanumeric()).collect();
    if clean.is_empty() {
        return String::new();
    }
    let mut chars = clean.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    let rest: String = chars.flat_map(|c| c.to_lowercase()).collect();
    format!("{first}{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_style_task_from_tokens_keyword() {
        let sig = TaskSignature {
            id: "t".into(),
            intent: neuromesh_core::TaskIntent::Modify,
            domain: "frontend".into(),
            technology: "Vue".into(),
            style: None,
            entity: "ProductCard".into(),
            goal: "style".into(),
            risk: neuromesh_core::TaskRisk::Low,
            related_concepts: vec![],
            identifiers: vec!["ProductCard".into()],
            file_hints: vec![],
            confidence: 0.9,
            raw_prompt: "Apply hover-lift using SCSS tokens and mixins on ProductCard".into(),
        };
        assert!(is_style_task(&sig));
    }

    #[test]
    fn pascal_case_handles_checkout() {
        assert_eq!(pascal_case("checkout"), "Checkout");
    }
}
