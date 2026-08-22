use crate::signature::TaskSignatureExtractor;
use neuromesh_core::{SubtaskNode, SubtaskStatus, TaskGraph};
use std::collections::HashMap;
use uuid::Uuid;

pub struct TaskDecomposer;

impl TaskDecomposer {
    pub fn decompose(prompt: &str) -> TaskGraph {
        let signature = TaskSignatureExtractor::extract(prompt);
        let lower = prompt.to_lowercase();

        let is_large_scaffold = lower.contains("ecommerce")
            || lower.contains("template")
            || lower.contains("complete")
            || lower.contains("production ready")
            || lower.contains("full app");

        let mut subtasks = HashMap::new();
        let mut execution_order = Vec::new();

        if is_large_scaffold {
            let task_specs = vec![
                (
                    "req",
                    "Requirements",
                    "Define core shopping flow, state requirements and asset tokens",
                    vec![],
                    vec!["README.md"],
                ),
                (
                    "arch",
                    "Architecture",
                    "Directory structure, router setup, Pinia store architecture",
                    vec!["req"],
                    vec!["router/index.ts", "stores/"],
                ),
                (
                    "design_tokens",
                    "Design System",
                    "SCSS variables for colors, typography, spacing, breakpoints",
                    vec!["arch"],
                    vec!["styles/_variables.scss", "styles/_breakpoints.scss"],
                ),
                (
                    "comp_header",
                    "Header & Navigation",
                    "Build responsive Header, Search, Navigation bars",
                    vec!["design_tokens"],
                    vec!["Header.vue", "Navigation.vue"],
                ),
                (
                    "comp_product_card",
                    "ProductCard Component",
                    "Build responsive product card with badge, rating and image",
                    vec!["design_tokens"],
                    vec!["ProductCard.vue", "types/product.ts"],
                ),
                (
                    "comp_product_grid",
                    "ProductGrid Component",
                    "Responsive multi-column product layout with CSS grid",
                    vec!["comp_product_card"],
                    vec!["ProductGrid.vue"],
                ),
                (
                    "comp_cart_drawer",
                    "Cart Drawer Component",
                    "Slide-over shopping cart with reactive line items & totals",
                    vec!["comp_product_card"],
                    vec!["CartDrawer.vue", "stores/cartStore.ts"],
                ),
                (
                    "comp_filters",
                    "Filters & Search",
                    "Faceted product filters and dynamic search input",
                    vec!["comp_product_grid"],
                    vec!["Filters.vue", "Search.vue"],
                ),
                (
                    "pages_home",
                    "Home & Category Pages",
                    "Scaffold HomeView and CategoryView pages",
                    vec!["comp_product_grid", "comp_header"],
                    vec!["HomeView.vue", "CategoryView.vue"],
                ),
                (
                    "state_pinia",
                    "State Management",
                    "Pinia Cart & Catalog stores with persistence",
                    vec!["arch"],
                    vec!["stores/cartStore.ts", "stores/productStore.ts"],
                ),
                (
                    "responsive_audit",
                    "Responsive Optimization",
                    "Verify mobile/tablet/desktop breakpoint styling",
                    vec!["pages_home"],
                    vec!["styles/main.scss"],
                ),
                (
                    "qa_verify",
                    "QA & Verification",
                    "TypeScript typecheck, build validation, component tests",
                    vec!["responsive_audit"],
                    vec!["package.json"],
                ),
            ];

            for (id, title, desc, deps, files) in task_specs {
                let node = SubtaskNode {
                    id: id.to_string(),
                    title: title.to_string(),
                    description: desc.to_string(),
                    status: SubtaskStatus::Pending,
                    dependencies: deps.into_iter().map(|s| s.to_string()).collect(),
                    relevant_files: files.into_iter().map(|s| s.to_string()).collect(),
                    relevant_symbols: Vec::new(),
                    context_tokens_used: 0,
                    children: Vec::new(),
                };
                subtasks.insert(id.to_string(), node);
                execution_order.push(id.to_string());
            }
        } else {
            let main_id = "task_main";
            let main_node = SubtaskNode {
                id: main_id.to_string(),
                title: format!("Execute: {}", signature.entity),
                description: prompt.to_string(),
                status: SubtaskStatus::Pending,
                dependencies: Vec::new(),
                relevant_files: vec![format!("{}.vue", signature.entity)],
                relevant_symbols: vec![signature.entity.clone()],
                context_tokens_used: 0,
                children: Vec::new(),
            };
            subtasks.insert(main_id.to_string(), main_node);
            execution_order.push(main_id.to_string());
        }

        TaskGraph {
            id: Uuid::new_v4().to_string(),
            root_task: prompt.to_string(),
            signature,
            subtasks,
            execution_order,
        }
    }
}
