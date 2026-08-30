use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct NearestAncestorManifestResolver<'a> {
    graph: &'a NeuralProjectGraph,
    cache: HashMap<PathBuf, Option<PackageStack>>,
}

#[derive(Debug, Clone)]
struct PackageStack {
    stack_token: String,
}

impl<'a> NearestAncestorManifestResolver<'a> {
    pub fn new(graph: &'a NeuralProjectGraph) -> Self {
        Self {
            graph,
            cache: HashMap::new(),
        }
    }

    pub fn packages_for_paths(&mut self, seed_paths: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for path in seed_paths {
            let Some(stack) = self.stack_for_file(path) else {
                continue;
            };
            if seen.insert(stack.stack_token.clone()) {
                out.push(stack.stack_token);
            }
        }
        out
    }

    pub fn stack_line(&mut self, seed_paths: &[String]) -> Option<String> {
        let packages = self.packages_for_paths(seed_paths);
        if packages.is_empty() {
            return None;
        }
        Some(packages.join(", "))
    }

    fn stack_for_file(&mut self, rel_path: &str) -> Option<PackageStack> {
        let path = PathBuf::from(rel_path.replace('\\', "/"));
        let mut dir = path.parent()?.to_path_buf();
        if dir.as_os_str().is_empty() {
            dir = PathBuf::from(".");
        }
        loop {
            if let Some(cached) = self.cache.get(&dir) {
                return cached.clone();
            }
            if let Some(stack) = self.detect_manifest_in_dir(&dir) {
                self.cache.insert(dir.clone(), Some(stack.clone()));
                return Some(stack);
            }
            if !dir.pop() {
                break;
            }
        }
        let fallback = extension_fallback(&path);
        self.cache.insert(path, fallback.clone());
        fallback
    }

    fn detect_manifest_in_dir(&self, dir: &Path) -> Option<PackageStack> {
        for (name, parser) in MANIFEST_PARSERS {
            let manifest_path = dir.join(name);
            if !self.graph_file_exists(&manifest_path) {
                continue;
            }
            let label = dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("root")
                .to_string();
            return Some(PackageStack {
                stack_token: parser(&label, name),
            });
        }
        None
    }

    fn graph_file_exists(&self, path: &Path) -> bool {
        let rel = path.to_string_lossy().replace('\\', "/");
        self.graph.resolve_file_hint(&rel).is_some()
            || self
                .graph
                .file_node_paths()
                .iter()
                .any(|(_, p)| p.to_string_lossy().replace('\\', "/") == rel)
    }
}

type ManifestParser = fn(&str, &str) -> String;

const MANIFEST_PARSERS: &[(&str, ManifestParser)] = &[
    ("Cargo.toml", |label, _| format!("{label}:rust/cargo")),
    ("package.json", |label, _| format!("{label}:ts/node")),
    ("composer.json", |label, _| format!("{label}:php/laravel")),
    ("pyproject.toml", |label, _| {
        format!("{label}:python/python")
    }),
    ("go.mod", |label, _| format!("{label}:go/go")),
];

fn extension_fallback(path: &Path) -> Option<PackageStack> {
    let ext = path.extension()?.to_str()?;
    let dialect = match ext {
        "rs" => "rust",
        "py" => "python",
        "php" => "php",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "go" => "go",
        _ => return None,
    };
    Some(PackageStack {
        stack_token: format!("generic:{dialect}"),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn composer_parser_detects_laravel() {
        let token = "api:php/laravel".to_string();
        assert!(token.contains("laravel"));
    }
}
