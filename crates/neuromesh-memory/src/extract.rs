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
        push_js_toolchain_facts(&mut facts, project_id, &pkg);
    }
    for nested in nested_package_jsons(root) {
        if let Ok(pkg) = fs::read_to_string(&nested) {
            push_js_toolchain_facts(&mut facts, project_id, &pkg);
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
        if file_mentions(
            root,
            &[
                "build.gradle.kts",
                "build.gradle",
                "settings.gradle.kts",
                "settings.gradle",
            ],
            "io.ktor",
        ) {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "ktor",
                "Ktor HTTP app (io.ktor in Gradle)",
            ));
        }
    }

    if let Ok(composer) = fs::read_to_string(root.join("composer.json")) {
        if composer.contains("laravel/framework")
            || composer.contains("illuminate/database")
            || composer.contains("laravel/tinker")
        {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "laravel",
                "Laravel project (composer.json)",
            ));
        }
        if composer.contains("pinoox/pincore")
            || composer.contains("pinoox/pinx")
            || composer.contains("pinoox/app")
        {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "pinoox",
                "Pinoox/Pinx app (composer.json pinoox/pincore)",
            ));
        }
        if composer.contains("pinoox/pinx-cli") || composer.contains("pinoox/pinx-inspector") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "pinx",
                "Pinx CLI (composer.json pinoox/pinx-cli)",
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

    let nested_apps = pinoox_app_packages(root);
    let root_is_app = root.join("app.php").exists()
        && (root.join("Controller").is_dir()
            || root.join("routes").is_dir()
            || root.join("theme").is_dir()
            || file_mentions(root, &["composer.json", "app.php"], "pinoox")
            || file_mentions(root, &["app.php"], "package")
            || file_mentions(root, &["app.php"], "pinx"));
    let has_pinx_cli = root.join("bin").join("pinx").exists()
        || facts
            .iter()
            .any(|f| f.category == "framework" && f.key == "pinx");
    if (root_is_app || !nested_apps.is_empty() || root.join("pinoox").is_file())
        && !facts
            .iter()
            .any(|f| f.category == "framework" && f.key == "pinoox")
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "pinoox",
            "Pinoox/Pinx layout (app.php, apps/, or pinoox CLI)",
        ));
    }
    if root_is_app && nested_apps.is_empty() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "architecture",
            "pinoox_mode",
            "single-app (root app.php; Pinx layout)",
        ));
        if (has_pinx_cli
            || file_mentions(root, &["app.php"], "pinx")
            || root.join("platform").join("apps.config.php").exists())
            && !facts
                .iter()
                .any(|f| f.category == "framework" && f.key == "pinx")
        {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "pinx",
                "Pinx single-app (bin/pinx, app.php pinx, or platform/)",
            ));
        }
    } else if !nested_apps.is_empty() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "architecture",
            "pinoox_mode",
            format!("multi-app ({})", nested_apps.join(", ")),
        ));
    }

    let laravel_layout = root.join("artisan").exists()
        || root.join("app").join("Models").is_dir()
        || root.join("database").join("migrations").is_dir()
        || root.join("database").join("seeders").is_dir()
        || root.join("database").join("factories").is_dir();
    if laravel_layout
        && !facts
            .iter()
            .any(|f| f.category == "framework" && f.key == "laravel")
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "laravel",
            "Laravel layout (artisan, app/Models, or database/migrations)",
        ));
    }
    if laravel_layout {
        let mut parts = Vec::new();
        if root.join("database").join("migrations").is_dir() {
            parts.push("migrations");
        }
        if root.join("database").join("seeders").is_dir()
            || root.join("database").join("seeds").is_dir()
        {
            parts.push("seeders");
        }
        if root.join("database").join("factories").is_dir() {
            parts.push("factories");
        }
        if root.join("app").join("Models").is_dir() {
            parts.push("eloquent");
        }
        if !parts.is_empty() {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "architecture",
                "laravel_database",
                parts.join(", "),
            ));
        }
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
        if file_mentions(root, &["Package.swift"], "SwiftUI") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "swiftui",
                "SwiftUI app (Package.swift)",
            ));
        }
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

    if csproj_mentions_aspnet(root) {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "aspnet",
            "ASP.NET project (Sdk.Web or AspNetCore in .csproj)",
        ));
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

fn push_js_toolchain_facts(facts: &mut Vec<ProjectFact>, project_id: &ProjectId, pkg: &str) {
    if !facts
        .iter()
        .any(|f| f.category == "framework" && f.key == "javascript_toolchain")
    {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "javascript_toolchain",
            "Node/TypeScript project with package.json",
        ));
    }
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
        (
            "\"@primeuix/",
            "primeuix",
            "PrimeUIX theme tokens (package.json dependency)",
        ),
        (
            "\"pinia\"",
            "pinia",
            "Pinia store (package.json dependency)",
        ),
        (
            "\"vue-router\"",
            "vue_router",
            "Vue Router (package.json dependency)",
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
        (
            "\"@remix-run/",
            "remix",
            "Remix app (package.json dependency)",
        ),
        (
            "\"react-router\"",
            "react_router",
            "React Router app (package.json dependency)",
        ),
        (
            "\"typescript\"",
            "typescript",
            "TypeScript (package.json dependency)",
        ),
        ("\"sass\"", "sass", "Sass/SCSS (package.json dependency)"),
        ("\"less\"", "less", "Less (package.json dependency)"),
    ] {
        if pkg.contains(needle)
            && !facts
                .iter()
                .any(|f| f.category == "framework" && f.key == key)
        {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                key,
                label,
            ));
        }
    }
}

fn nested_package_jsons(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    collect_named_files(root, "package.json", 0, 5, &mut out);
    out.retain(|path| path != &root.join("package.json"));
    out
}

fn pinoox_app_packages(root: &Path) -> Vec<String> {
    let apps = root.join("apps");
    let Ok(entries) = fs::read_dir(&apps) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().join("app.php").exists())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    names
}

fn collect_named_files(
    dir: &Path,
    filename: &str,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<std::path::PathBuf>,
) {
    if depth > max_depth || out.len() >= 24 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let lower = name.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "node_modules" | "vendor" | "target" | ".git" | "dist" | "build" | "storage" | "~pinx"
        ) {
            continue;
        }
        if path.is_dir() {
            collect_named_files(&path, filename, depth + 1, max_depth, out);
        } else if name.eq_ignore_ascii_case(filename) {
            out.push(path);
        }
    }
}

fn file_mentions(root: &Path, names: &[&str], needle: &str) -> bool {
    names
        .iter()
        .any(|name| fs::read_to_string(root.join(name)).is_ok_and(|s| s.contains(needle)))
}

fn csproj_mentions_aspnet(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("csproj"))
            && fs::read_to_string(&path).is_ok_and(|s| {
                s.contains("Microsoft.NET.Sdk.Web") || s.contains("Microsoft.AspNetCore")
            })
    })
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
    fn pinx_single_app_layout_and_nested_primevue() {
        let dir = temp_root();
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("Controller")).unwrap();
        fs::create_dir_all(dir.join("theme/spark")).unwrap();
        fs::write(dir.join("bin/pinx"), "#!/usr/bin/env php\n").unwrap();
        fs::write(
            dir.join("app.php"),
            "<?php return ['package' => 'com_pinoox_app', 'pinx' => ['type' => 'app']];\n",
        )
        .unwrap();
        fs::write(
            dir.join("theme/spark/package.json"),
            r#"{ "dependencies": { "vue": "3", "primevue": "4", "@primeuix/themes": "1", "pinia": "3" } }"#,
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("shop"));
        assert!(facts.iter().any(|f| f.key == "pinoox"));
        assert!(facts.iter().any(|f| f.key == "pinx"));
        assert!(facts
            .iter()
            .any(|f| f.key == "pinoox_mode" && f.content.contains("single-app")));
        assert!(facts.iter().any(|f| f.key == "vue"));
        assert!(facts.iter().any(|f| f.key == "primevue"));
        assert!(facts.iter().any(|f| f.key == "primeuix"));
        assert!(facts.iter().any(|f| f.key == "pinia"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pinoox_multi_app_detects_packages() {
        let dir = temp_root();
        fs::create_dir_all(dir.join("apps/com_shop")).unwrap();
        fs::create_dir_all(dir.join("apps/com_blog")).unwrap();
        fs::write(
            dir.join("composer.json"),
            r#"{ "require": { "pinoox/pincore": "*" } }"#,
        )
        .unwrap();
        fs::write(
            dir.join("apps/com_shop/app.php"),
            "<?php return ['package' => 'com_shop'];\n",
        )
        .unwrap();
        fs::write(
            dir.join("apps/com_blog/app.php"),
            "<?php return ['package' => 'com_blog'];\n",
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("platform"));
        assert!(facts.iter().any(|f| f.key == "pinoox"));
        let mode = facts
            .iter()
            .find(|f| f.key == "pinoox_mode")
            .map(|f| f.content.as_str())
            .unwrap_or("");
        assert!(mode.contains("multi-app"), "{mode}");
        assert!(mode.contains("com_shop"));
        assert!(mode.contains("com_blog"));
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
            r#"{ "dependencies": { "express": "4", "@nestjs/core": "11", "@angular/core": "19", "@remix-run/node": "2", "react-router": "7" } }"#,
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
        fs::write(
            dir.join("App.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>\n",
        )
        .unwrap();
        fs::write(
            dir.join("build.gradle.kts"),
            "implementation(\"io.ktor:ktor-server-core\")\n",
        )
        .unwrap();
        fs::write(
            dir.join("Package.swift"),
            "// swift-tools-version: 5.9\nimport PackageDescription\nimport SwiftUI\n",
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("demo"));
        assert!(facts.iter().any(|f| f.key == "express"));
        assert!(facts.iter().any(|f| f.key == "nestjs"));
        assert!(facts.iter().any(|f| f.key == "angular"));
        assert!(facts.iter().any(|f| f.key == "remix"));
        assert!(facts.iter().any(|f| f.key == "react_router"));
        assert!(facts.iter().any(|f| f.key == "gin"));
        assert!(facts.iter().any(|f| f.key == "axum"));
        assert!(facts.iter().any(|f| f.key == "aspnet"));
        assert!(facts.iter().any(|f| f.key == "ktor"));
        assert!(facts.iter().any(|f| f.key == "swiftui"));
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

    #[test]
    fn laravel_layout_and_database_facts() {
        let dir = temp_root();
        fs::create_dir_all(dir.join("app/Models")).unwrap();
        fs::create_dir_all(dir.join("database/migrations")).unwrap();
        fs::create_dir_all(dir.join("database/seeders")).unwrap();
        fs::create_dir_all(dir.join("database/factories")).unwrap();
        fs::write(dir.join("artisan"), "#!/usr/bin/env php\n").unwrap();
        fs::write(
            dir.join("composer.json"),
            r#"{ "require": { "laravel/framework": "^11" } }"#,
        )
        .unwrap();
        let facts = extract_project_facts(&dir, &ProjectId::new("shop"));
        assert!(facts.iter().any(|f| f.key == "laravel"));
        assert!(facts
            .iter()
            .any(|f| f.key == "laravel_database" && f.content.contains("migrations")));
        let _ = fs::remove_dir_all(&dir);
    }
}
