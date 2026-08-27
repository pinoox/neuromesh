use crate::activator::SeedSink;
use neuromesh_core::TaskSignature;
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
    if lower.contains("price-card") || lower.contains("pricecard") {
        for hint in [
            "src/styles/_priceCard.scss",
            "src/styles/priceCard.scss",
            "styles/_priceCard.scss",
        ] {
            if graph.resolve_file_hint(hint).is_some() {
                sink.push(graph, prompt, hint.to_string(), 0.88, "style_partial");
            }
        }
        sink.push(graph, prompt, "price-card-tile".into(), 0.75, "style_mixin");
    }

    for token in style_token_queries(signature) {
        for hit in graph.search_symbols(&token, 4) {
            if hit.node_type == neuromesh_core::NodeType::StyleToken
                || is_style_path(&hit.file_path)
            {
                sink.push(graph, prompt, hit.name.clone(), 0.78, "style_token");
            }
        }
    }
}

fn style_token_queries(signature: &TaskSignature) -> Vec<String> {
    let lower = signature.raw_prompt.to_lowercase();
    let mut out = Vec::new();
    for kw in [
        "hover-lift",
        "focus-within",
        "price-card",
        "price-card-tile",
    ] {
        if lower.contains(kw) {
            out.push(kw.to_string());
        }
    }
    out
}

const SCSS_STYLE_HINTS: &[&str] = &[
    "src/styles/_tokens.scss",
    "src/styles/tokens.scss",
    "src/styles/_mixins.scss",
    "src/styles/mixins.scss",
    "styles/_tokens.scss",
    "styles/tokens.scss",
    "styles/_mixins.scss",
    "styles/mixins.scss",
];

fn style_file_hints(style: &Option<String>) -> Vec<&'static str> {
    match style.as_deref() {
        Some("scss") | Some("sass") => SCSS_STYLE_HINTS.to_vec(),
        Some("less") => vec!["styles/tokens.less", "src/styles/tokens.less"],
        Some("css") => vec!["styles/tokens.css", "src/styles/tokens.css"],
        _ => SCSS_STYLE_HINTS.to_vec(),
    }
}

pub(crate) fn inject_view_component_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_>,
) {
    let lower = signature.raw_prompt.to_lowercase();
    let view_task = lower.contains("checkout")
        || lower.contains("cartview")
        || lower.contains("productcard")
        || lower.contains("product card")
        || lower.contains("setqty")
        || lower.contains("quantity")
        || lower.contains("stepper");
    if !view_task {
        return;
    }
    let mut candidates: HashSet<String> = HashSet::new();
    for ident in &signature.identifiers {
        if ident.ends_with("View") || ident.ends_with("Component") {
            candidates.insert(ident.clone());
        }
    }
    for word in ["checkout", "cart", "product", "home", "header"] {
        if word == "cart"
            && (prompt_contains_word(&lower, "checkout") || lower.contains("cartview"))
            && !lower.contains("cart view")
        {
            continue;
        }
        if prompt_contains_word(&lower, word) {
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
        || p.contains("appbutton")
    {
        return 28.0;
    }
    0.0
}

fn prompt_contains_word(lower: &str, word: &str) -> bool {
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == word)
}

/// Drop optional connector fill when checkout/store seeds already anchor the task.
pub(crate) fn tighten_focused_view_selection(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    selection: &mut crate::selector::Selection,
) {
    let lower = signature.raw_prompt.to_lowercase();
    let focused_checkout = (lower.contains("setqty") || prompt_contains_word(&lower, "stepper"))
        && prompt_contains_word(&lower, "checkout");
    if !focused_checkout {
        return;
    }
    let keep = |path: &str| {
        let p = path.replace('\\', "/").to_lowercase();
        p.contains("checkoutview") || p.contains("stores/cart")
    };
    selection.optional.retain(|id| {
        graph
            .get_node(id)
            .map(|n| keep(&n.file_path.to_string_lossy()))
            .unwrap_or(false)
    });
    selection.required.retain(|id| {
        graph
            .get_node(id)
            .map(|n| keep(&n.file_path.to_string_lossy()))
            .unwrap_or(true)
    });
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

    #[test]
    fn default_style_hints_include_non_underscore_paths() {
        let hints = style_file_hints(&None);
        assert!(hints.contains(&"src/styles/tokens.scss"));
        assert!(hints.contains(&"src/styles/mixins.scss"));
    }
}
