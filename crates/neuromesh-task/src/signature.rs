use neuromesh_core::{TaskIntent, TaskRisk, TaskSignature};
use neuromesh_parser::{
    extract_embedded_code_tokens, extract_prompt_anchors, is_imperative_verb, normalize_unicode,
};
use uuid::Uuid;

pub struct TaskSignatureExtractor;

impl TaskSignatureExtractor {
    pub fn extract(prompt: &str) -> TaskSignature {
        let prompt = normalize_unicode(prompt);
        let lower = prompt.to_lowercase();
        let anchors = extract_prompt_anchors(&prompt);
        let embedded = extract_embedded_code_tokens(&prompt);

        let intent = if lower.contains("build")
            || lower.contains("create")
            || lower.contains("scaffold")
            || lower.contains("generate")
            || lower.contains("implement")
            || lower.contains("design")
            || lower.contains("plan")
            || lower.contains("architect")
        {
            TaskIntent::Create
        } else if lower.contains("fix")
            || lower.contains("bug")
            || lower.contains("error")
            || lower.contains("issue")
            || lower.contains("repair")
        {
            TaskIntent::Fix
        } else if lower.contains("refactor")
            || lower.contains("clean up")
            || lower.contains("restructure")
        {
            TaskIntent::Refactor
        } else if lower.contains("optimize")
            || lower.contains("speed up")
            || lower.contains("performance")
        {
            TaskIntent::Optimize
        } else if lower.contains("test") || lower.contains("spec") || lower.contains("typecheck") {
            TaskIntent::Test
        } else if lower.contains("explain")
            || lower.contains("what is")
            || lower.contains("how does")
            || lower.contains("how do")
        {
            TaskIntent::Explain
        } else if lower.contains("make")
            || lower.contains("update")
            || lower.contains("change")
            || lower.contains("add")
        {
            TaskIntent::Modify
        } else {
            TaskIntent::Query
        };

        let technology = if lower.contains("vue 3")
            || lower.contains("vue3")
            || lower.contains("vue")
        {
            "Vue".to_string()
        } else if lower.contains("react")
            || lower.contains("next.js")
            || lower.contains("nextjs")
            || lower.contains("next/")
        {
            "React".to_string()
        } else if lower.contains("rust") || lower.contains("cargo") || lower.contains("neuromesh") {
            "Rust".to_string()
        } else if lower.contains("python") || lower.contains("django") || lower.contains("fastapi")
        {
            "Python".to_string()
        } else if lower.contains("typescript") || lower.contains(".ts") {
            "TypeScript".to_string()
        } else if lower.contains("golang") || lower.contains(" go ") {
            "Go".to_string()
        } else if lower.contains("pinoox") || lower.contains("pinx") || lower.contains("pincore") {
            "Pinoox".to_string()
        } else if lower.contains("laravel") || lower.contains("eloquent") {
            "Laravel".to_string()
        } else if lower.contains("primevue") || lower.contains("prime vue") {
            "Vue".to_string()
        } else if lower.contains("javascript")
            || lower.contains(".cjs")
            || lower.contains(".mjs")
            || lower.contains(".jsx")
        {
            "JavaScript".to_string()
        } else if lower.contains(".sql")
            || lower.contains("create table")
            || lower.contains("postgres")
            || lower.contains("mysql")
        {
            "SQL".to_string()
        } else {
            "Fullstack".to_string()
        };

        let style = if lower.contains("scss") || lower.contains("sass") {
            Some("SCSS".to_string())
        } else if lower.contains("less") {
            Some("Less".to_string())
        } else if lower.contains("tailwind") {
            Some("Tailwind".to_string())
        } else if lower.contains("css") {
            Some("CSS".to_string())
        } else {
            None
        };

        let domain = if lower.contains("ecommerce")
            || lower.contains("store")
            || lower.contains("shop")
        {
            "ecommerce".to_string()
        } else if lower.contains("frontend") || lower.contains("ui") || lower.contains("responsive")
        {
            "frontend".to_string()
        } else if lower.contains("backend") || lower.contains("api") || lower.contains("database") {
            "backend".to_string()
        } else {
            "general".to_string()
        };

        let entity = anchors
            .identifiers
            .iter()
            .find(|id| !is_imperative_verb(id))
            .cloned()
            .or_else(|| fallback_entity(&lower))
            .unwrap_or_else(|| "Workspace".to_string());

        let goal = if lower.contains("responsive") {
            "responsive".to_string()
        } else if lower.contains("explain")
            || lower.contains("how does")
            || lower.contains("what is")
        {
            "explain".to_string()
        } else if lower.contains("fix") {
            "bug fix".to_string()
        } else {
            "feature delivery".to_string()
        };

        let risk = if lower.contains("auth")
            || lower.contains("security")
            || lower.contains("payment")
            || lower.contains("stripe")
            || lower.contains("migration")
            || lower.contains("drop table")
        {
            TaskRisk::Critical
        } else if lower.contains("refactor")
            || lower.contains("delete")
            || lower.contains("architecture")
        {
            TaskRisk::High
        } else if lower.contains("state") || lower.contains("api") || lower.contains("database") {
            TaskRisk::Medium
        } else {
            TaskRisk::Low
        };

        let mut related_concepts = anchors.identifiers.clone();
        if lower.contains("responsive") || lower.contains("mobile") || lower.contains("breakpoint")
        {
            related_concepts.push("layout".to_string());
            related_concepts.push("breakpoints".to_string());
        }
        if lower.contains("state") || lower.contains("pinia") || lower.contains("store") {
            related_concepts.push("state".to_string());
        }
        if (lower.contains("token") || lower.contains("mixin") || lower.contains("stylesheet"))
            && (lower.contains("scss")
                || lower.contains("sass")
                || lower.contains("stylesheet")
                || lower.contains("mixin"))
        {
            related_concepts.push("tokens".to_string());
            related_concepts.push("mixins".to_string());
        }
        if lower.contains("checkout") {
            related_concepts.push("CheckoutView".to_string());
        }

        related_concepts.sort();
        related_concepts.dedup();

        let confidence = if !anchors.identifiers.is_empty() || !anchors.file_hints.is_empty() {
            0.94
        } else if entity != "Workspace" {
            0.82
        } else {
            0.62
        };

        let mut identifiers = anchors.identifiers;
        for token in embedded {
            if !identifiers.iter().any(|id| id.eq_ignore_ascii_case(&token)) {
                identifiers.push(token);
            }
        }

        TaskSignature {
            id: Uuid::new_v4().to_string(),
            intent,
            domain,
            technology,
            style,
            entity,
            goal,
            risk,
            related_concepts,
            identifiers,
            file_hints: anchors.file_hints,
            client_keywords: Vec::new(),
            client_expansion: Vec::new(),
            client_path_hints: Vec::new(),
            client_entity_types: Vec::new(),
            client_intent: None,
            engine_override: None,
            confidence,
            raw_prompt: prompt.to_string(),
        }
    }
}

fn fallback_entity(lower: &str) -> Option<String> {
    let entities = [
        ("cart", "Cart"),
        ("product card", "ProductCard"),
        ("product grid", "ProductGrid"),
        ("product", "Product"),
        ("navigation", "Navigation"),
        ("header", "Header"),
        ("checkout", "Checkout"),
        ("auth", "Auth"),
    ];
    for (pattern, name) in entities {
        if lower.contains(pattern) {
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_task_signature_responsive_cart() {
        let prompt = "Make the shopping cart responsive.";
        let sig = TaskSignatureExtractor::extract(prompt);

        assert_eq!(sig.intent, TaskIntent::Modify);
        assert_eq!(sig.entity, "Cart");
        assert_eq!(sig.goal, "responsive");
        assert_eq!(sig.risk, TaskRisk::Low);
        assert!(sig.related_concepts.contains(&"breakpoints".to_string()));
    }

    #[test]
    fn extracts_real_code_identifiers() {
        let sig = TaskSignatureExtractor::extract(
            "How does neuromesh_get_context extract task intent in tools.rs?",
        );
        assert!(sig
            .identifiers
            .iter()
            .any(|id| id == "neuromesh_get_context"));
        assert_eq!(sig.intent, TaskIntent::Explain);
        assert!(sig.file_hints.iter().any(|p| p.contains("tools.rs")));
    }

    #[test]
    fn design_prompt_is_create_intent() {
        let sig = TaskSignatureExtractor::extract(
            "Design the product catalog domain for products and categories.",
        );
        assert_eq!(sig.intent, TaskIntent::Create);
        assert!(
            !sig.identifiers.iter().any(|id| id == "Design"),
            "identifiers = {:?}",
            sig.identifiers
        );
    }

    #[test]
    fn next_callback_is_not_react_technology() {
        let sig =
            TaskSignatureExtractor::extract("Explain the middleware pipeline and how next() works");
        assert_ne!(sig.technology, "React");
    }
}
