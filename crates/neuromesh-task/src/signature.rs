use neuromesh_core::{TaskIntent, TaskRisk, TaskSignature};
use regex::Regex;
use uuid::Uuid;

pub struct TaskSignatureExtractor;

impl TaskSignatureExtractor {
    pub fn extract(prompt: &str) -> TaskSignature {
        let lower = prompt.to_lowercase();

        // 1. Detect Intent
        let intent = if lower.contains("build")
            || lower.contains("create")
            || lower.contains("scaffold")
            || lower.contains("generate")
            || lower.contains("implement")
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
        } else if lower.contains("explain") || lower.contains("what is") || lower.contains("how does") {
            TaskIntent::Explain
        } else if lower.contains("make") || lower.contains("update") || lower.contains("change") || lower.contains("add") {
            TaskIntent::Modify
        } else {
            TaskIntent::Query
        };

        // 2. Detect Technology & Framework
        let technology = if lower.contains("vue 3") || lower.contains("vue3") || lower.contains("vue") {
            "Vue".to_string()
        } else if lower.contains("react") || lower.contains("next") {
            "React".to_string()
        } else if lower.contains("rust") || lower.contains("cargo") {
            "Rust".to_string()
        } else if lower.contains("python") || lower.contains("django") || lower.contains("fastapi") {
            "Python".to_string()
        } else if lower.contains("typescript") || lower.contains("ts") {
            "TypeScript".to_string()
        } else if lower.contains("go") || lower.contains("golang") {
            "Go".to_string()
        } else {
            "Fullstack".to_string()
        };

        // 3. Detect Styling
        let style = if lower.contains("scss") || lower.contains("sass") {
            Some("SCSS".to_string())
        } else if lower.contains("tailwind") {
            Some("Tailwind".to_string())
        } else if lower.contains("css") {
            Some("CSS".to_string())
        } else {
            None
        };

        // 4. Detect Domain
        let domain = if lower.contains("ecommerce") || lower.contains("store") || lower.contains("shop") {
            "ecommerce".to_string()
        } else if lower.contains("frontend") || lower.contains("ui") || lower.contains("responsive") {
            "frontend".to_string()
        } else if lower.contains("backend") || lower.contains("api") || lower.contains("database") {
            "backend".to_string()
        } else {
            "general".to_string()
        };

        // 5. Detect Entity
        let entity = Self::extract_entity(&lower);

        // 6. Detect Goal
        let goal = if lower.contains("responsive") {
            "responsive".to_string()
        } else if lower.contains("componentized") {
            "componentized".to_string()
        } else if lower.contains("production ready") {
            "production ready".to_string()
        } else if lower.contains("fix") {
            "bug fix".to_string()
        } else {
            "feature delivery".to_string()
        };

        // 7. Detect Risk
        let risk = if lower.contains("auth")
            || lower.contains("security")
            || lower.contains("payment")
            || lower.contains("stripe")
            || lower.contains("migration")
            || lower.contains("drop table")
        {
            TaskRisk::Critical
        } else if lower.contains("refactor") || lower.contains("delete") || lower.contains("architecture") {
            TaskRisk::High
        } else if lower.contains("state") || lower.contains("api") || lower.contains("database") {
            TaskRisk::Medium
        } else {
            TaskRisk::Low
        };

        // 8. Related Concepts
        let mut related_concepts = Vec::new();
        if lower.contains("responsive") || lower.contains("mobile") || lower.contains("breakpoint") {
            related_concepts.push("layout".to_string());
            related_concepts.push("breakpoints".to_string());
            related_concepts.push("responsive".to_string());
        }
        if lower.contains("state") || lower.contains("pinia") || lower.contains("store") || lower.contains("cart") {
            related_concepts.push("state".to_string());
            related_concepts.push("reactivity".to_string());
        }
        if lower.contains("scss") || lower.contains("color") || lower.contains("typography") || lower.contains("spacing") {
            related_concepts.push("design tokens".to_string());
            related_concepts.push("variables".to_string());
        }
        if lower.contains("ecommerce") || lower.contains("product") {
            related_concepts.push("catalog".to_string());
            related_concepts.push("pricing".to_string());
        }

        let confidence = if !entity.is_empty() && !technology.is_empty() {
            0.92
        } else {
            0.75
        };

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
            confidence,
            raw_prompt: prompt.to_string(),
        }
    }

    fn extract_entity(lower: &str) -> String {
        let entities = [
            ("cart", "Cart"),
            ("product card", "ProductCard"),
            ("product grid", "ProductGrid"),
            ("product", "Product"),
            ("navigation", "Navigation"),
            ("header", "Header"),
            ("footer", "Footer"),
            ("search", "Search"),
            ("filter", "Filters"),
            ("checkout", "Checkout"),
            ("ecommerce template", "EcommerceTemplate"),
            ("store", "Store"),
            ("auth", "Auth"),
            ("user", "User"),
        ];

        for (pattern, name) in entities {
            if lower.contains(pattern) {
                return name.to_string();
            }
        }

        // Regex fallback for capitalized words
        let word_regex = Regex::new(r"\b([A-Z][a-zA-Z0-9]+)\b").unwrap();
        if let Some(cap) = word_regex.captures(lower) {
            if let Some(m) = cap.get(1) {
                return m.as_str().to_string();
            }
        }

        "Workspace".to_string()
    }
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
        assert!(sig.related_concepts.contains(&"layout".to_string()));
        assert!(sig.related_concepts.contains(&"breakpoints".to_string()));
    }
}
