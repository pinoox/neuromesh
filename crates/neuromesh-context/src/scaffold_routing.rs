//! Framework entry-point seeds when symbol resolution finds nothing (greenfield / NL tasks).

use crate::activator::SeedSink;
use neuromesh_core::TaskSignature;
use neuromesh_graph::NeuralProjectGraph;

const MAX_SCAFFOLD_SEEDS: usize = 3;

/// Inject up to three resolved file seeds when the graph has no symbol hits yet.
/// Only call when `seed_energies` is still empty after anchors and client keywords.
pub(crate) fn inject_scaffold_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_>,
) -> bool {
    let tech = signature.technology.to_lowercase();
    if tech.contains("laravel") || prompt.to_lowercase().contains("laravel") {
        return inject_laravel_scaffold(graph, prompt, signature, sink);
    }
    false
}

fn inject_laravel_scaffold(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_>,
) -> bool {
    let lower = format!(
        "{} {}",
        signature.raw_prompt.to_lowercase(),
        prompt.to_lowercase()
    );
    let hints = laravel_scaffold_hints(&lower);
    let mut injected = 0usize;
    for (hint, tag) in hints {
        if injected >= MAX_SCAFFOLD_SEEDS {
            break;
        }
        if graph.resolve_file_hint(hint).is_none() {
            continue;
        }
        let before = sink.resolved_count();
        sink.push(
            graph,
            prompt,
            hint.to_string(),
            0.88,
            &format!("scaffold:laravel-{tag}"),
        );
        if sink.resolved_count() > before {
            injected += 1;
        }
    }
    injected > 0
}

fn laravel_scaffold_hints(lower: &str) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();

    if contains_any(
        lower,
        &[
            "route",
            "controller",
            "listing",
            "detail",
            "middleware",
            "handler",
            "webhook",
            "checkout",
            "cart",
        ],
    ) {
        push_hint(&mut out, "routes/web.php", "routes");
        push_hint(&mut out, "bootstrap/app.php", "bootstrap");
        push_hint(
            &mut out,
            "app/Http/Controllers/Controller.php",
            "controller",
        );
    }
    if contains_any(
        lower,
        &[
            "model",
            "eloquent",
            "relationship",
            "migration",
            "schema",
            "factory",
            "seeder",
            "product",
            "catalog",
            "category",
            "order",
        ],
    ) {
        push_hint(&mut out, "routes/web.php", "routes");
        push_hint(&mut out, "composer.json", "stack");
        push_hint(&mut out, "database/migrations", "migrations");
        push_hint(&mut out, "app/Models/User.php", "model");
    }
    if contains_any(
        lower,
        &[
            "auth",
            "policy",
            "permission",
            "role",
            "login",
            "register",
            "password",
        ],
    ) {
        push_hint(&mut out, "config/auth.php", "auth");
    }
    if contains_any(lower, &["test", "feature", "unit", "phpunit", "spec"]) {
        push_hint(&mut out, "tests/Feature", "tests");
        push_hint(&mut out, "phpunit.xml", "phpunit");
    }
    if contains_any(lower, &["queue", "cache", "redis", "horizon", "session"]) {
        push_hint(&mut out, "config/queue.php", "queue");
        push_hint(&mut out, "config/cache.php", "cache");
    }
    if out.is_empty() {
        push_hint(&mut out, "routes/web.php", "default-routes");
        push_hint(&mut out, "composer.json", "default-stack");
        push_hint(&mut out, "bootstrap/app.php", "default-bootstrap");
    }
    out
}

fn push_hint(out: &mut Vec<(&'static str, &'static str)>, path: &'static str, tag: &'static str) {
    if !out.iter().any(|(p, _)| *p == path) {
        out.push((path, tag));
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laravel_hints_cover_routes_and_models() {
        let lower = "design product catalog with eloquent models and listing routes";
        let hints = laravel_scaffold_hints(lower);
        assert!(hints.iter().any(|(p, _)| p.contains("web.php")));
        assert!(hints.iter().any(|(p, _)| p.contains("User.php")));
    }
}
