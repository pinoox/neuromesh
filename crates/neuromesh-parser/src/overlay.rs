use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::NodeType;
use neuromesh_index::SourceLanguage;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// Framework overlay on top of language extract. Unknown annotations are a
/// soft miss — they never fail the index. Stack is inferred from file layout
/// and annotation text, not from a compiler.
pub fn apply(path: &Path, content: &str, language: SourceLanguage, ast: &mut AstAnalysisResult) {
    match language {
        SourceLanguage::Kotlin | SourceLanguage::Java => {
            android_overlay(path, content, ast);
            spring_overlay(content, ast);
        }
        SourceLanguage::Python => django_overlay(path, content, ast),
        SourceLanguage::TypeScript | SourceLanguage::JavaScript => {
            next_overlay(path, ast);
            sveltekit_overlay(path, ast);
            react_overlay(path, content, ast);
            vue_router_overlay(path, content, ast);
            prime_overlay(content, ast);
            vite_overlay(path, ast);
            electron_overlay(content, ast);
        }
        SourceLanguage::Vue | SourceLanguage::Svelte => {
            sveltekit_overlay(path, ast);
            vue_router_overlay(path, content, ast);
            prime_overlay(content, ast);
        }
        SourceLanguage::PHP => {
            laravel_overlay(path, content, ast);
            pinoox_overlay(path, content, ast);
            php_controller_overlay(path, ast);
            symfony_overlay(content, ast);
            wordpress_overlay(content, ast);
        }
        SourceLanguage::Rust => tauri_overlay(content, ast),
        SourceLanguage::Twig => twig_overlay(content, ast),
        _ => {}
    }
}

fn android_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    static COMPOSE_RE: OnceLock<Regex> = OnceLock::new();
    let class_re = CLASS_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:(?:public|open|abstract|internal|private)\s+)*class\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::|extends)\s+[A-Za-z0-9_.<>,\s]*\b(BroadcastReceiver|AppCompatActivity|ComponentActivity|Activity|Service|Fragment)\b",
        )
        .unwrap()
    });
    let compose_re = COMPOSE_RE.get_or_init(|| {
        Regex::new(
            r"@Composable\s+(?:(?:private|internal|public)\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)",
        )
        .unwrap()
    });
    for cap in class_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
    for cap in compose_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if (stem.ends_with("Receiver") || stem.ends_with("Activity") || stem.ends_with("Service"))
        && (content.contains("BroadcastReceiver")
            || content.contains("android")
            || content.contains("AppCompatActivity")
            || content.contains("ComponentActivity"))
    {
        promote(ast, stem, NodeType::Component);
    }
}

fn spring_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("Mapping") && !content.contains("RestController") {
        return;
    }
    static MAP_RE: OnceLock<Regex> = OnceLock::new();
    let map_re = MAP_RE.get_or_init(|| {
        Regex::new(
            r#"@(Get|Post|Put|Patch|Delete)Mapping\s*\(\s*(?:value\s*=\s*|path\s*=\s*)?["']([^"']+)["']"#,
        )
        .unwrap()
    });
    static REQ_RE: OnceLock<Regex> = OnceLock::new();
    let req_re = REQ_RE.get_or_init(|| {
        Regex::new(r#"@RequestMapping\s*\(\s*(?:value\s*=\s*|path\s*=\s*)?["']([^"']+)["']"#)
            .unwrap()
    });
    for cap in map_re.captures_iter(content) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".into());
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("@{method}Mapping(\"{route}\")"),
            line,
        );
    }
    for cap in req_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("ANY {route}"),
            format!("@RequestMapping(\"{route}\")"),
            line,
        );
    }
}

fn django_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name != "urls.py" && !content.contains("urlpatterns") {
        return;
    }
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let path_re =
        PATH_RE.get_or_init(|| Regex::new(r#"(?:re_)?path\(\s*["']([^"']+)["']"#).unwrap());
    for cap in path_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, route, format!("path(\"{route}\")"), line);
    }
}

fn next_overlay(path: &Path, ast: &mut AstAnalysisResult) {
    let Some(route) = next_app_route(path) else {
        return;
    };
    push_api(ast, &route, format!("Next.js {route}"), 1);
}

fn laravel_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let rel = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if !rel.contains("/routes/")
        && !content.contains("Route::")
        && !content.contains("Illuminate\\Support\\Facades\\Route")
    {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"Route::(get|post|put|patch|delete)\s*\(\s*['"]([^'"]+)['"]"#).unwrap()
    });
    for cap in route_re.captures_iter(content) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".into());
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("Route::{method}(\"{route}\")"),
            line,
        );
    }
}

/// Pinx / Pinoox named actions: `action([SmsController::class, 'store'])->name('sms.store')`.
/// See https://github.com/pinoox/pinoox and https://github.com/pinoox/docs
fn pinoox_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("action(") && !content.contains("Pinoox\\") {
        return;
    }
    let rel = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if !rel.contains("/routes/")
        && !rel.ends_with("routes.php")
        && !content.contains("action([")
        && !content.contains("action( [")
    {
        return;
    }
    static ACTION_RE: OnceLock<Regex> = OnceLock::new();
    let action_re = ACTION_RE.get_or_init(|| {
        Regex::new(
            r#"action\(\s*\[\s*([A-Za-z_][A-Za-z0-9_\\]*)::class\s*,\s*['"]([^'"]+)['"]\s*\]\s*\)(?:\s*->name\(\s*['"]([^'"]+)['"]\s*\))?"#,
        )
        .unwrap()
    });
    for cap in action_re.captures_iter(content) {
        let controller = cap.get(1).map(|m| m.as_str()).unwrap_or("Controller");
        let method = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if method.is_empty() {
            continue;
        }
        let short = controller.rsplit('\\').next().unwrap_or(controller);
        let name = cap
            .get(3)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| format!("{short}::{method}"));
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &name,
            format!("action([{short}::class, '{method}'])"),
            line,
        );
    }
}

fn php_controller_overlay(path: &Path, ast: &mut AstAnalysisResult) {
    let rel = path.to_string_lossy().replace('\\', "/");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if stem.ends_with("Controller") || rel.contains("/Controller/") || rel.contains("/controller/")
    {
        promote(ast, stem, NodeType::Component);
    }
}

fn symfony_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("Route") {
        return;
    }
    static ATTR_RE: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR_RE.get_or_init(|| Regex::new(r"#\[Route\((.*?)\)\]").unwrap());
    static ANN_RE: OnceLock<Regex> = OnceLock::new();
    let ann_re = ANN_RE.get_or_init(|| Regex::new(r"@Route\(([^)]*)\)").unwrap());
    for cap in attr_re
        .captures_iter(content)
        .chain(ann_re.captures_iter(content))
    {
        let inner = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let Some(route) = first_quoted(inner) else {
            continue;
        };
        let method = route_method(inner).unwrap_or("ANY");
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("Route(\"{route}\")"),
            line,
        );
    }
}

fn first_quoted(inner: &str) -> Option<&str> {
    let bytes = inner.as_bytes();
    let start = inner.find(['\'', '"'])?;
    let quote = bytes[start];
    let rest = &inner[start + 1..];
    let end = rest.find(quote as char)?;
    Some(&rest[..end])
}

fn route_method(inner: &str) -> Option<&'static str> {
    let lower = inner.to_ascii_lowercase();
    for (needle, method) in [
        ("post", "POST"),
        ("put", "PUT"),
        ("patch", "PATCH"),
        ("delete", "DELETE"),
        ("get", "GET"),
    ] {
        if lower.contains(needle) {
            return Some(method);
        }
    }
    None
}

fn wordpress_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("register_rest_route") && !content.contains("add_action") {
        return;
    }
    static REST_RE: OnceLock<Regex> = OnceLock::new();
    let rest_re = REST_RE.get_or_init(|| {
        Regex::new(r#"register_rest_route\(\s*['"]([^'"]+)['"]\s*,\s*['"]([^'"]+)['"]"#).unwrap()
    });
    for cap in rest_re.captures_iter(content) {
        let ns = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if ns.is_empty() || route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let name = format!("{ns}{route}");
        push_api(
            ast,
            &name,
            format!("register_rest_route(\"{ns}\", \"{route}\")"),
            line,
        );
    }
}

fn react_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let looks_react = matches!(ext.as_str(), "tsx" | "jsx")
        || content.contains("from 'react'")
        || content.contains("from \"react\"")
        || content.contains("from 'react/")
        || content.contains("from \"react/");
    if !looks_react {
        return;
    }
    static FN_RE: OnceLock<Regex> = OnceLock::new();
    let fn_re = FN_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:export\s+default\s+)?(?:export\s+)?(?:default\s+)?function\s+([A-Z][A-Za-z0-9_]*)",
        )
        .unwrap()
    });
    static ARROW_RE: OnceLock<Regex> = OnceLock::new();
    let arrow_re = ARROW_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:export\s+default\s+)?(?:export\s+)?(?:const|let)\s+([A-Z][A-Za-z0-9_]*)\s*=\s*(?:\([^)]*\)|[A-Za-z_][A-Za-z0-9_]*)\s*=>",
        )
        .unwrap()
    });
    for cap in fn_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
    for cap in arrow_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
}

fn vue_router_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !name.contains("router")
        && !content.contains("createRouter")
        && !content.contains("vue-router")
    {
        return;
    }
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let path_re = PATH_RE.get_or_init(|| Regex::new(r#"path:\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in path_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, route, format!("path: \"{route}\""), line);
    }
}

fn prime_overlay(content: &str, ast: &mut AstAnalysisResult) {
    let uses_prime = content.contains("primevue")
        || content.contains("primereact")
        || content.contains("primeicons")
        || content.contains("@primeuix");
    if !uses_prime {
        return;
    }
    static IMPORT_RE: OnceLock<Regex> = OnceLock::new();
    let import_re = IMPORT_RE.get_or_init(|| {
        Regex::new(
            r#"import\s+(?:([A-Z][A-Za-z0-9_]*)\s*,?\s*)?(?:\{\s*([^}]+)\s*\}\s*)?from\s*['"](?:primevue|primereact)[^'"]*['"]"#,
        )
        .unwrap()
    });
    for cap in import_re.captures_iter(content) {
        if let Some(default) = cap.get(1) {
            promote(ast, default.as_str(), NodeType::Component);
        }
        if let Some(named) = cap.get(2) {
            for part in named.as_str().split(',') {
                let name = part
                    .rsplit_once(" as ")
                    .map(|(_, alias)| alias)
                    .unwrap_or(part)
                    .trim()
                    .trim_start_matches("type ");
                if name.starts_with(char::is_uppercase)
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    promote(ast, name, NodeType::Component);
                }
            }
        }
    }
}

fn vite_overlay(path: &Path, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !name.starts_with("vite.config") {
        return;
    }
    if ast.symbols.iter().any(|s| s.name == "vite") {
        return;
    }
    ast.symbols.push(ParsedSymbol::new(
        "vite",
        NodeType::Config,
        Some("vite.config".into()),
        1..2,
        true,
    ));
}

fn electron_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("electron")
        && !content.contains("ipcMain")
        && !content.contains("BrowserWindow")
    {
        return;
    }
    static IPC_RE: OnceLock<Regex> = OnceLock::new();
    let ipc_re =
        IPC_RE.get_or_init(|| Regex::new(r#"ipcMain\.(handle|on)\(\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in ipc_re.captures_iter(content) {
        let channel = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if channel.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, channel, format!("ipcMain.handle(\"{channel}\")"), line);
    }
    if content.contains("BrowserWindow") {
        promote(ast, "BrowserWindow", NodeType::Component);
    }
}

fn sveltekit_overlay(path: &Path, ast: &mut AstAnalysisResult) {
    let Some(route) = sveltekit_route(path) else {
        return;
    };
    push_api(ast, &route, format!("SvelteKit {route}"), 1);
}

fn sveltekit_route(path: &Path) -> Option<String> {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./");
    let after = s
        .split("/routes/")
        .nth(1)
        .or_else(|| s.strip_prefix("src/routes/"))
        .or_else(|| s.strip_prefix("routes/"))?;
    let (dir, file) = after.rsplit_once('/').unwrap_or(("", after));
    let file_l = file.to_ascii_lowercase();
    let is_page = file_l == "+page.svelte"
        || file_l == "+page.ts"
        || file_l == "+page.js"
        || file_l == "+page.server.ts"
        || file_l == "+page.server.js"
        || file_l == "+server.ts"
        || file_l == "+server.js";
    if !is_page {
        return None;
    }
    Some(join_next_segments(dir))
}

fn tauri_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("tauri::command") && !content.contains("tauri::generate_handler") {
        return;
    }
    static CMD_RE: OnceLock<Regex> = OnceLock::new();
    let cmd_re = CMD_RE.get_or_init(|| {
        Regex::new(r"#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
            .unwrap()
    });
    for cap in cmd_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Api);
            let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
            push_api(
                ast,
                name.as_str(),
                format!("#[tauri::command] fn {}()", name.as_str()),
                line,
            );
        }
    }
}

fn twig_overlay(content: &str, ast: &mut AstAnalysisResult) {
    static BLOCK_RE: OnceLock<Regex> = OnceLock::new();
    let block_re =
        BLOCK_RE.get_or_init(|| Regex::new(r"\{%\s*block\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap());
    for cap in block_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let path_re =
        PATH_RE.get_or_init(|| Regex::new(r#"\{\{\s*path\(\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in path_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, route, format!("path(\"{route}\")"), line);
    }
}

fn next_app_route(path: &Path) -> Option<String> {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./");
    let after = s
        .strip_prefix("app/")
        .or_else(|| s.split("/app/").nth(1))
        .or_else(|| s.strip_prefix("pages/"))
        .or_else(|| s.split("/pages/").nth(1))?;
    let (dir, file) = after.rsplit_once('/').unwrap_or(("", after));
    let file_l = file.to_ascii_lowercase();
    let is_page = matches!(
        file_l.as_str(),
        "page.tsx"
            | "page.ts"
            | "page.jsx"
            | "page.js"
            | "index.tsx"
            | "index.ts"
            | "index.jsx"
            | "index.js"
    );
    let is_route = matches!(file_l.as_str(), "route.ts" | "route.js" | "route.tsx");
    if !is_page && !is_route {
        if dir.is_empty() && file_l.starts_with("api") {
            return Some(format!("/{}", strip_ext(file)));
        }
        return None;
    }
    Some(join_next_segments(dir))
}

fn join_next_segments(rel: &str) -> String {
    let parts: Vec<&str> = rel
        .split('/')
        .filter(|s| !s.is_empty())
        .filter(|s| !(s.starts_with('(') && s.ends_with(')')))
        .collect();
    if parts.is_empty() {
        "/".into()
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn strip_ext(file: &str) -> &str {
    file.rsplit_once('.').map(|(n, _)| n).unwrap_or(file)
}

fn promote(ast: &mut AstAnalysisResult, name: &str, kind: NodeType) {
    if let Some(sym) = ast.symbols.iter_mut().find(|s| s.name == name) {
        if matches!(
            sym.symbol_type,
            NodeType::Class | NodeType::Symbol | NodeType::Function
        ) {
            sym.symbol_type = kind;
        }
        return;
    }
    ast.symbols
        .push(ParsedSymbol::new(name, kind, None, 1..2, true));
}

fn push_api(ast: &mut AstAnalysisResult, name: &str, signature: String, line: usize) {
    if ast.symbols.iter().any(|s| s.name == name) {
        return;
    }
    let line = line.max(1);
    ast.symbols.push(ParsedSymbol::new(
        name,
        NodeType::Api,
        Some(signature),
        line..(line + 1),
        true,
    ));
}

fn line_of(content: &str, byte: usize) -> usize {
    content
        .get(..byte)
        .map(|head| head.bytes().filter(|b| *b == b'\n').count() + 1)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CodeIntelligenceEngine;
    use std::path::PathBuf;

    fn analyze(rel: &str, src: &str, language: SourceLanguage) -> AstAnalysisResult {
        CodeIntelligenceEngine::analyze(&PathBuf::from(rel), src, language)
    }

    fn has_api(ast: &AstAnalysisResult, name: &str) -> bool {
        ast.symbols
            .iter()
            .any(|s| s.name == name && s.symbol_type == NodeType::Api)
    }

    #[test]
    fn android_receiver_promotes_to_component() {
        let src = "package com.example.app\nimport android.content.BroadcastReceiver\nclass SmsReceiver : BroadcastReceiver() {\n  override fun onReceive(body: String?) { SmsStore.save(body) }\n}\n";
        let ast = analyze("src/SmsReceiver.kt", src, SourceLanguage::Kotlin);
        let recv = ast
            .symbols
            .iter()
            .find(|s| s.name == "SmsReceiver")
            .expect("SmsReceiver");
        assert_eq!(recv.symbol_type, NodeType::Component);
    }

    #[test]
    fn spring_get_mapping_is_api() {
        let src = "class SmsController {\n  @GetMapping(\"/sms\")\n  fun list() {}\n}\n";
        let ast = analyze("SmsController.java", src, SourceLanguage::Java);
        assert!(
            has_api(&ast, "GET /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn django_path_is_api() {
        let src = "from django.urls import path\nurlpatterns = [path(\"sms/\", views.save)]\n";
        let ast = analyze("app/urls.py", src, SourceLanguage::Python);
        assert!(has_api(&ast, "sms/"));
    }

    #[test]
    fn next_app_route_is_api() {
        let src = "export async function POST() { return saveSms(); }\n";
        let ast = analyze("app/api/sms/route.ts", src, SourceLanguage::TypeScript);
        assert!(
            has_api(&ast, "/api/sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn laravel_route_is_api() {
        let src = "<?php\nRoute::post('/sms', [SmsController::class, 'store']);\n";
        let ast = analyze("routes/web.php", src, SourceLanguage::PHP);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pinoox_action_is_api() {
        let src = "<?php\naction([SmsController::class, 'store'])->name('sms.store');\n";
        let ast = analyze("routes/web.php", src, SourceLanguage::PHP);
        assert!(
            has_api(&ast, "sms.store"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        let ctrl = analyze(
            "Controller/SmsController.php",
            "<?php class SmsController { public function store($body) { SmsStore::save($body); } }\n",
            SourceLanguage::PHP,
        );
        let recv = ctrl
            .symbols
            .iter()
            .find(|s| s.name == "SmsController")
            .expect("SmsController");
        assert_eq!(recv.symbol_type, NodeType::Component);
    }

    #[test]
    fn symfony_route_attribute_is_api() {
        let src = "<?php\nclass SmsController {\n  #[Route('/sms', methods: ['POST'])]\n  public function store() {}\n}\n";
        let ast = analyze("src/SmsController.php", src, SourceLanguage::PHP);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn wordpress_rest_route_is_api() {
        let src = "<?php\nregister_rest_route('sms/v1', '/inbox', ['callback' => 'save_sms']);\n";
        let ast = analyze("plugin.php", src, SourceLanguage::PHP);
        assert!(
            has_api(&ast, "sms/v1/inbox"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn react_function_component_promotes() {
        let src = "import { useState } from 'react';\nexport default function SmsInbox() { return <div />; }\n";
        let ast = analyze("src/SmsInbox.tsx", src, SourceLanguage::TypeScript);
        let recv = ast
            .symbols
            .iter()
            .find(|s| s.name == "SmsInbox")
            .expect("SmsInbox");
        assert_eq!(recv.symbol_type, NodeType::Component);
    }

    #[test]
    fn vue_router_path_is_api() {
        let src = "import { createRouter } from 'vue-router';\nexport default createRouter({ routes: [{ path: '/sms', component: Inbox }] });\n";
        let ast = analyze("src/router.ts", src, SourceLanguage::TypeScript);
        assert!(has_api(&ast, "/sms"));
    }

    #[test]
    fn primevue_import_is_component() {
        let src = "import Button from 'primevue/button';\nexport default function App() { return Button; }\n";
        let ast = analyze("src/App.tsx", src, SourceLanguage::TypeScript);
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "Button" && s.symbol_type == NodeType::Component));
    }

    #[test]
    fn electron_ipc_is_api() {
        let src = "const { ipcMain, BrowserWindow } = require('electron');\nipcMain.handle('save-sms', () => saveSms());\n";
        let ast = analyze("main.js", src, SourceLanguage::JavaScript);
        assert!(has_api(&ast, "save-sms"));
    }

    #[test]
    fn tauri_command_is_api() {
        let src = "#[tauri::command]\npub fn save_sms(body: String) { persist(body); }\n";
        let ast = analyze("src-tauri/src/lib.rs", src, SourceLanguage::Rust);
        assert!(
            has_api(&ast, "save_sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn vite_config_is_config_node() {
        let src = "import { defineConfig } from 'vite';\nexport default defineConfig({});\n";
        let ast = analyze("vite.config.ts", src, SourceLanguage::TypeScript);
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "vite" && s.symbol_type == NodeType::Config));
    }

    #[test]
    fn sveltekit_page_is_api() {
        let src = "<script>import { saveSms } from '$lib/sms';</script>\n<button on:click={saveSms}>Save</button>\n";
        let ast = analyze("src/routes/sms/+page.svelte", src, SourceLanguage::Svelte);
        assert!(
            has_api(&ast, "/sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "+page" && s.symbol_type == NodeType::Component));
    }

    #[test]
    fn twig_block_and_path() {
        let src = "{% block inbox %}{{ path('sms.store') }}{% endblock %}\n";
        let ast = analyze("theme/home.twig", src, SourceLanguage::Twig);
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "inbox" && s.symbol_type == NodeType::Component));
        assert!(has_api(&ast, "sms.store"));
    }
}
