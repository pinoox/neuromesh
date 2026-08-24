use crate::project::ProjectFact;
use neuromesh_core::ProjectId;
use std::fs;
use std::path::Path;

/// Seed project memory from the repository itself — manifests, docs, crate layout.
pub fn extract_project_facts(root: &Path, project_id: &ProjectId) -> Vec<ProjectFact> {
    let mut facts = Vec::new();

    if let Ok(cargo) = fs::read_to_string(root.join("Cargo.toml")) {
        if cargo.contains("[workspace]") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "architecture",
                "build_system",
                "Rust cargo workspace",
            ));
        }
        if let Some(members) = extract_workspace_members(&cargo) {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "architecture",
                "workspace_crates",
                members,
            ));
        }
        if cargo.contains("neuromesh") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "product",
                "NeuroMesh biomimetic MCP context engine",
            ));
        }
        if cargo.contains("axum") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "axum",
                "Axum HTTP app (Cargo.toml)",
            ));
        }
    }

    if let Ok(pkg) = fs::read_to_string(root.join("package.json")) {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "javascript_toolchain",
            "Node/TypeScript project with package.json",
        ));
        for (needle, key, label) in [
            (
                "\"next\"",
                "nextjs",
                "Next.js app (package.json dependency)",
            ),
            ("\"react\"", "react", "React app (package.json dependency)"),
            ("\"vue\"", "vue", "Vue app (package.json dependency)"),
            (
                "\"svelte\"",
                "svelte",
                "Svelte app (package.json dependency)",
            ),
            (
                "\"vite\"",
                "vite",
                "Vite toolchain (package.json dependency)",
            ),
            (
                "\"electron\"",
                "electron",
                "Electron app (package.json dependency)",
            ),
            (
                "\"@tauri-apps/",
                "tauri",
                "Tauri app (package.json dependency)",
            ),
            (
                "\"primereact\"",
                "primereact",
                "PrimeReact UI (package.json dependency)",
            ),
            (
                "\"primevue\"",
                "primevue",
                "PrimeVue UI (package.json dependency)",
            ),
            ("\"astro\"", "astro", "Astro app (package.json dependency)"),
            ("\"nuxt\"", "nuxt", "Nuxt app (package.json dependency)"),
            (
                "\"express\"",
                "express",
                "Express app (package.json dependency)",
            ),
            (
                "\"@nestjs/core\"",
                "nestjs",
                "NestJS app (package.json dependency)",
            ),
            (
                "\"@angular/core\"",
                "angular",
                "Angular app (package.json dependency)",
            ),
        ] {
            if pkg.contains(needle) {
                facts.push(ProjectFact::new(
                    project_id.clone(),
                    "framework",
                    key,
                    label,
                ));
            }
        }
    }

    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "python_toolchain",
            "Python project",
        ));
        if file_mentions(root, &["pyproject.toml", "requirements.txt"], "Django") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "django",
                "Django project (manifest names Django)",
            ));
        }
        if file_mentions(root, &["pyproject.toml", "requirements.txt"], "fastapi")
            || file_mentions(root, &["pyproject.toml", "requirements.txt"], "FastAPI")
        {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "fastapi",
                "FastAPI project (manifest names FastAPI)",
            ));
        }
    }

    if root.join("settings.gradle.kts").exists()
        || root.join("settings.gradle").exists()
        || root.join("build.gradle.kts").exists()
        || root.join("build.gradle").exists()
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "android_kotlin",
            "Gradle/Kotlin project (settings or build.gradle.kts)",
        ));
        if file_mentions(
            root,
            &[
                "build.gradle.kts",
                "build.gradle",
                "settings.gradle.kts",
                "settings.gradle",
                "app/build.gradle.kts",
                "app/build.gradle",
            ],
            "com.android",
        ) {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "android",
                "Android Gradle project (com.android plugin)",
            ));
        }
        if file_mentions(
            root,
            &[
                "build.gradle.kts",
                "build.gradle",
                "settings.gradle.kts",
                "settings.gradle",
            ],
            "org.springframework",
        ) {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "spring",
                "Spring project (org.springframework in Gradle)",
            ));
        }
    }

    if let Ok(composer) = fs::read_to_string(root.join("composer.json")) {
        if composer.contains("laravel/framework") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "laravel",
                "Laravel project (composer.json)",
            ));
        }
        if composer.contains("pinoox/pincore") || composer.contains("pinoox/pinx") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "pinoox",
                "Pinoox/Pinx app (composer.json pinoox/pincore)",
            ));
        }
        if composer.contains("symfony/framework-bundle") || composer.contains("symfony/symfony") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "symfony",
                "Symfony project (composer.json)",
            ));
        }
        if composer.contains("johnpbloch/wordpress") || composer.contains("wpackagist") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "wordpress",
                "WordPress project (composer.json)",
            ));
        }
        let lower = composer.to_ascii_lowercase();
        if lower.contains("shopfa") || lower.contains("shopyfa") || composer.contains("شاپفا")
        {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "shopfa",
                "Shopfa/Shopyfa mention in composer.json (hosted store; no source router)",
            ));
        }
    }

    if root.join("app.php").exists()
        && (root.join("Controller").is_dir()
            || root.join("routes").is_dir()
            || file_mentions(root, &["composer.json", "app.php"], "pinoox")
            || file_mentions(root, &["app.php"], "package"))
        && !facts
            .iter()
            .any(|f| f.category == "framework" && f.key == "pinoox")
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "pinoox",
            "Pinoox/Pinx layout (app.php + Controller/routes)",
        ));
    }

    if root.join("wp-config.php").exists() || root.join("wp-content").is_dir() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "wordpress",
            "WordPress project (wp-config.php or wp-content)",
        ));
    }

    if root.join("src-tauri").is_dir() || root.join("src-tauri/tauri.conf.json").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "tauri",
            "Tauri desktop app (src-tauri)",
        ));
    }

    if root.join("pubspec.yaml").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "dart_flutter",
            "Dart/Flutter project (pubspec.yaml)",
        ));
        if file_mentions(root, &["pubspec.yaml"], "flutter:") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "flutter",
                "Flutter SDK in pubspec.yaml",
            ));
        }
    }
    if root.join("Package.swift").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "swift_package",
            "Swift package (Package.swift)",
        ));
    }
    if root.join("go.mod").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "go_module",
            "Go module (go.mod)",
        ));
        if file_mentions(root, &["go.mod"], "gin-gonic/gin") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "gin",
                "Gin HTTP app (go.mod)",
            ));
        }
        if file_mentions(root, &["go.mod"], "labstack/echo") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "echo",
                "Echo HTTP app (go.mod)",
            ));
        }
    }

    if root.join("angular.json").exists()
        && !facts
            .iter()
            .any(|f| f.category == "framework" && f.key == "angular")
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "angular",
            "Angular workspace (angular.json)",
        ));
    }
    if root.join("nest-cli.json").exists()
        && !facts
            .iter()
            .any(|f| f.category == "framework" && f.key == "nestjs")
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "nestjs",
            "NestJS workspace (nest-cli.json)",
        ));
    }

    if root.join("Gemfile").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "ruby_toolchain",
            "Ruby project (Gemfile)",
        ));
        if file_mentions(root, &["Gemfile"], "rails") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "rails",
                "Rails project (Gemfile)",
            ));
        }
    }

    let mut doc_paths = vec![root.join("README.md")];
    if let Ok(entries) = fs::read_dir(root.join("docs")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                doc_paths.push(path);
            }
        }
    }
    for path in doc_paths {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("doc")
            .to_lowercase();
        let summary = first_meaningful_lines(&content, 8);
        if !summary.is_empty() {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "documentation",
                name,
                summary,
            ));
        }
    }

    if root.join("crates/neuromesh-mcp").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "convention",
            "agent_interface",
            "Primary agent interface is MCP (stdio JSON-RPC). Prefer neuromesh_get_context for task-conditioned evidence packets.",
        ));
    }

    facts
}

fn file_mentions(root: &Path, names: &[&str], needle: &str) -> bool {
    names
        .iter()
        .any(|name| fs::read_to_string(root.join(name)).is_ok_and(|s| s.contains(needle)))
}

fn extract_workspace_members(cargo: &str) -> Option<String> {
    let start = cargo.find("members")?;
    let slice = &cargo[start..];
    let open = slice.find('[')?;
    let close = slice.find(']')?;
    let inner = &slice[open + 1..close];
    let members: Vec<&str> = inner
        .split(',')
        .filter_map(|part| {
            let trimmed = part.trim().trim_matches('"').trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .collect();
    if members.is_empty() {
        None
    } else {
        Some(members.join(", "))
    }
}

fn first_meaningful_lines(content: &str, max_lines: usize) -> String {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("<") && *line != "---")
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(600)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("neuromesh-facts-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn composer_detects_pinoox_and_shopfa() {
        let dir = temp_root();
        fs::write(
            dir.join("composer.json"),
            r#"{ "require": { "pinoox/pincore": "*", "shopfa/theme": "*" } }"#,
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("demo"));
        assert!(facts.iter().any(|f| f.key == "pinoox"));
        assert!(facts.iter().any(|f| f.key == "shopfa"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_json_detects_react_and_vite() {
        let dir = temp_root();
        fs::write(
            dir.join("package.json"),
            r#"{ "dependencies": { "react": "19", "vite": "6" } }"#,
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("demo"));
        assert!(facts.iter().any(|f| f.key == "react"));
        assert!(facts.iter().any(|f| f.key == "vite"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifests_detect_express_nest_angular_gin_axum() {
        let dir = temp_root();
        fs::write(
            dir.join("package.json"),
            r#"{ "dependencies": { "express": "4", "@nestjs/core": "11", "@angular/core": "19" } }"#,
        )
        .unwrap();
        fs::write(
            dir.join("go.mod"),
            "module x\nrequire github.com/gin-gonic/gin v1.10.0\n",
        )
        .unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname=\"x\"\n[dependencies]\naxum=\"0.8\"\n",
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("demo"));
        assert!(facts.iter().any(|f| f.key == "express"));
        assert!(facts.iter().any(|f| f.key == "nestjs"));
        assert!(facts.iter().any(|f| f.key == "angular"));
        assert!(facts.iter().any(|f| f.key == "gin"));
        assert!(facts.iter().any(|f| f.key == "axum"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pyproject_detects_fastapi() {
        let dir = temp_root();
        fs::write(dir.join("requirements.txt"), "fastapi>=0.115\n").unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("demo"));
        assert!(facts.iter().any(|f| f.key == "fastapi"));
        let _ = fs::remove_dir_all(&dir);
    }
}
