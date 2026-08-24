use crate::query_extract::{self, Grammar, QueryOptions, CSHARP_QUERIES};
use crate::types::{AstAnalysisResult, ParsedSymbol};
use crate::typescript::TypeScriptParser;
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
            ktor_overlay(content, ast);
        }
        SourceLanguage::Python => {
            django_overlay(path, content, ast);
            fastapi_overlay(content, ast);
        }
        SourceLanguage::Ruby => rails_overlay(path, content, ast),
        SourceLanguage::Dart => flutter_overlay(content, ast),
        SourceLanguage::TypeScript | SourceLanguage::JavaScript => {
            next_overlay(path, ast);
            sveltekit_overlay(path, ast);
            react_overlay(path, content, ast);
            vue_router_overlay(path, content, ast);
            prime_overlay(content, ast);
            vite_overlay(path, ast);
            electron_overlay(content, ast);
            express_overlay(content, ast);
            nest_overlay(content, ast);
            angular_overlay(content, ast);
            remix_overlay(path, content, ast);
        }
        SourceLanguage::Go => gin_overlay(content, ast),
        SourceLanguage::Vue | SourceLanguage::Svelte => {
            sveltekit_overlay(path, ast);
            vue_router_overlay(path, content, ast);
            prime_overlay(content, ast);
            nuxt_overlay(path, ast);
        }
        SourceLanguage::Astro => astro_overlay(path, content, ast),
        SourceLanguage::PHP => {
            laravel_overlay(path, content, ast);
            pinoox_overlay(path, content, ast);
            php_controller_overlay(path, ast);
            symfony_overlay(content, ast);
            wordpress_overlay(content, ast);
        }
        SourceLanguage::Rust => {
            tauri_overlay(content, ast);
            axum_overlay(content, ast);
        }
        SourceLanguage::CSharp => aspnet_overlay(content, ast),
        SourceLanguage::Swift => swiftui_overlay(content, ast),
        SourceLanguage::HTML => razor_overlay(path, content, ast),
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

fn fastapi_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("@app.") && !content.contains("@router.") && !content.contains("@api.") {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"@(?:app|router|api)\.(get|post|put|patch|delete|route)\(\s*["']([^"']+)["']"#)
            .unwrap()
    });
    for cap in route_re.captures_iter(content) {
        let verb = cap.get(1).map(|m| m.as_str()).unwrap_or("route");
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let method = if verb == "route" {
            "ANY".into()
        } else {
            verb.to_uppercase()
        };
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("@{verb}(\"{route}\")"),
            line,
        );
    }
}

fn rails_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name != "routes.rb"
        && !content.contains("Rails.application.routes")
        && !content.contains("draw do")
    {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"(?m)^\s*(get|post|put|patch|delete)\s+['"]([^'"]+)['"]"#).unwrap()
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
            format!("{method} \"{route}\""),
            line,
        );
    }
}

fn flutter_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("Widget") && !content.contains("flutter") {
        return;
    }
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    let class_re = CLASS_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:class|mixin)\s+([A-Za-z_][A-Za-z0-9_]*)\s+extends\s+(?:StatelessWidget|StatefulWidget|Widget|ConsumerWidget)\b",
        )
        .unwrap()
    });
    for cap in class_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
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

fn express_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains(".get(")
        && !content.contains(".post(")
        && !content.contains(".put(")
        && !content.contains(".patch(")
        && !content.contains(".delete(")
    {
        return;
    }
    if !content.contains("express")
        && !content.contains("Router(")
        && !content.contains("app.post")
        && !content.contains("app.get")
        && !content.contains("router.post")
        && !content.contains("router.get")
    {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"(?:app|router|r)\.(get|post|put|patch|delete)\s*\(\s*['"]([^'"]+)['"]"#)
            .unwrap()
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
            format!("{method} {route}"),
            line,
        );
    }
}

fn nest_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("@Controller")
        && !content.contains("@Get(")
        && !content.contains("@Post(")
        && !content.contains("@Put(")
        && !content.contains("@Patch(")
        && !content.contains("@Delete(")
    {
        return;
    }
    static CTRL_RE: OnceLock<Regex> = OnceLock::new();
    static HTTP_RE: OnceLock<Regex> = OnceLock::new();
    let ctrl_re =
        CTRL_RE.get_or_init(|| Regex::new(r#"@Controller\(\s*['"]([^'"]*)['"]"#).unwrap());
    let http_re = HTTP_RE.get_or_init(|| {
        Regex::new(r#"@(Get|Post|Put|Patch|Delete)\(\s*(?:['"]([^'"]*)['"]\s*)?\)"#).unwrap()
    });
    let mut events: Vec<(usize, NestEvent<'_>)> = Vec::new();
    for cap in ctrl_re.captures_iter(content) {
        let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let prefix = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        events.push((start, NestEvent::Controller(prefix)));
    }
    for cap in http_re.captures_iter(content) {
        let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let method = cap.get(1).map(|m| m.as_str()).unwrap_or("Get");
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        events.push((start, NestEvent::Http(method, path)));
    }
    events.sort_by_key(|(pos, _)| *pos);
    let mut prefix = "";
    for (start, event) in events {
        match event {
            NestEvent::Controller(p) => prefix = p,
            NestEvent::Http(method, path) => {
                let route = nest_route(prefix, path);
                let method = method.to_uppercase();
                let line = line_of(content, start);
                push_api(
                    ast,
                    &format!("{method} {route}"),
                    format!("@{method}(\"{route}\")"),
                    line,
                );
            }
        }
    }
}

enum NestEvent<'a> {
    Controller(&'a str),
    Http(&'a str, &'a str),
}

fn nest_route(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim().trim_matches('/');
    let path = path.trim().trim_matches('/');
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".into(),
        (true, false) => format!("/{path}"),
        (false, true) => format!("/{prefix}"),
        (false, false) => format!("/{prefix}/{path}"),
    }
}

fn angular_overlay(content: &str, ast: &mut AstAnalysisResult) {
    let looks_angular = content.contains("@angular/")
        || content.contains("@Component(")
        || content.contains("Routes")
        || content.contains("RouterModule");
    if !looks_angular {
        return;
    }
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    let class_re = CLASS_RE.get_or_init(|| {
        Regex::new(r"@Component\([\s\S]{0,400}?\)\s*(?:export\s+)?class\s+([A-Z][A-Za-z0-9_]*)")
            .unwrap()
    });
    for cap in class_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let path_re = PATH_RE.get_or_init(|| {
        Regex::new(r#"path:\s*['"](/?[A-Za-z0-9~_-]+(?:/[A-Za-z0-9~_-]+)*)['"]"#).unwrap()
    });
    for cap in path_re.captures_iter(content) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if raw.is_empty() || raw.contains('.') {
            continue;
        }
        let route = if raw.starts_with('/') {
            raw.to_string()
        } else {
            format!("/{raw}")
        };
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, &route, format!("Angular path:{route}"), line);
    }
}

fn gin_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains(".GET(")
        && !content.contains(".POST(")
        && !content.contains(".PUT(")
        && !content.contains(".PATCH(")
        && !content.contains(".DELETE(")
    {
        return;
    }
    if !content.contains("gin") && !content.contains("echo") && !content.contains("Echo") {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"\.(GET|POST|PUT|PATCH|DELETE)\(\s*["']([^"']+)["']"#).unwrap()
    });
    for cap in route_re.captures_iter(content) {
        let method = cap.get(1).map(|m| m.as_str()).unwrap_or("GET");
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("{method} {route}"),
            line,
        );
    }
}

fn aspnet_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("MapGet")
        && !content.contains("MapPost")
        && !content.contains("MapPut")
        && !content.contains("MapPatch")
        && !content.contains("MapDelete")
        && !content.contains("[HttpGet")
        && !content.contains("[HttpPost")
        && !content.contains("[HttpPut")
        && !content.contains("[HttpPatch")
        && !content.contains("[HttpDelete")
        && !content.contains("[Route(")
    {
        return;
    }
    static MAP_RE: OnceLock<Regex> = OnceLock::new();
    let map_re = MAP_RE.get_or_init(|| {
        Regex::new(r#"Map(Get|Post|Put|Patch|Delete)\s*\(\s*["']([^"']+)["']"#).unwrap()
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
            format!("Map{method}(\"{route}\")"),
            line,
        );
    }
    static HTTP_RE: OnceLock<Regex> = OnceLock::new();
    let http_re = HTTP_RE.get_or_init(|| {
        Regex::new(r#"\[Http(Get|Post|Put|Patch|Delete)\(\s*["']([^"']+)["']"#).unwrap()
    });
    for cap in http_re.captures_iter(content) {
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
            format!("[Http{method}(\"{route}\")]"),
            line,
        );
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_HTTP_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| Regex::new(r#"\[Route\(\s*["']([^"']+)["']"#).unwrap());
    let bare_http =
        BARE_HTTP_RE.get_or_init(|| Regex::new(r"\[Http(Get|Post|Put|Patch|Delete)\]").unwrap());
    let mut events: Vec<(usize, AspNetEvent<'_>)> = Vec::new();
    for cap in route_re.captures_iter(content) {
        let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let prefix = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if prefix.contains('[') {
            continue;
        }
        events.push((start, AspNetEvent::Route(prefix)));
    }
    for cap in bare_http.captures_iter(content) {
        let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let method = cap.get(1).map(|m| m.as_str()).unwrap_or("Get");
        events.push((start, AspNetEvent::Http(method)));
    }
    events.sort_by_key(|(pos, _)| *pos);
    let mut prefix = "";
    for (start, event) in events {
        match event {
            AspNetEvent::Route(p) => prefix = p,
            AspNetEvent::Http(method) => {
                if prefix.is_empty() {
                    continue;
                }
                let route = if prefix.starts_with('/') {
                    prefix.to_string()
                } else {
                    format!("/{prefix}")
                };
                let method = method.to_uppercase();
                let line = line_of(content, start);
                push_api(
                    ast,
                    &format!("{method} {route}"),
                    format!("[Http{method}] [Route(\"{prefix}\")]"),
                    line,
                );
            }
        }
    }
}

enum AspNetEvent<'a> {
    Route(&'a str),
    Http(&'a str),
}

fn razor_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "cshtml" | "razor")
        && !content.contains("@page")
        && !content.contains("@code")
    {
        return;
    }
    static PAGE_RE: OnceLock<Regex> = OnceLock::new();
    let page_re = PAGE_RE.get_or_init(|| Regex::new(r#"@page\s+["']([^"']+)["']"#).unwrap());
    for cap in page_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, route, format!("@page \"{route}\""), line);
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if matches!(ext.as_str(), "cshtml" | "razor") && !stem.is_empty() {
        promote(ast, stem, NodeType::Component);
    }
    for block in razor_code_blocks(content) {
        let wrapped = format!("class __RazorCode {{\n{block}\n}}\n");
        let Some(extra) = query_extract::parse(
            path,
            &wrapped,
            Grammar::CSharp,
            CSHARP_QUERIES,
            QueryOptions::csharp(),
        ) else {
            continue;
        };
        for sym in extra.symbols {
            if sym.name == "__RazorCode" || ast.symbols.iter().any(|s| s.name == sym.name) {
                continue;
            }
            ast.symbols.push(sym);
        }
        ast.imports.extend(extra.imports);
        ast.relationships.extend(extra.relationships);
    }
}

fn razor_code_blocks(content: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    for (idx, _) in content.match_indices("@code") {
        let after = &content[idx + 5..];
        let Some(rel) = after.find('{') else {
            continue;
        };
        let open = idx + 5 + rel;
        let Some(close) = matching_brace(content, open) else {
            continue;
        };
        if close > open + 1 {
            blocks.push(&content[open + 1..close]);
        }
    }
    blocks
}

fn matching_brace(content: &str, open: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    if open >= bytes.len() || bytes[open] != b'{' {
        return None;
    }
    let mut depth = 0i32;
    for (k, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

fn swiftui_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains(": View")
        && !content.contains(": App")
        && !content.contains("SwiftUI")
        && !content.contains("UIViewController")
        && !content.contains("UIView")
    {
        return;
    }
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    let class_re = CLASS_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:struct|class)\s+([A-Z][A-Za-z0-9_]*)\s*:\s*(?:View|App|UIViewController|UIView|ObservableObject)\b",
        )
        .unwrap()
    });
    for cap in class_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
}

fn ktor_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("ktor") && !content.contains("routing {") && !content.contains("routing{")
    {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"\b(get|post|put|patch|delete)\s*\(\s*["']([^"']+)["']"#).unwrap()
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
            format!("{method} {route}"),
            line,
        );
    }
}

fn remix_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    if let Some(route) = remix_file_route(path) {
        push_api(ast, &route, format!("Remix {route}"), 1);
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let looks_rr = content.contains("createBrowserRouter")
        || content.contains("createHashRouter")
        || content.contains("react-router")
        || content.contains("@remix-run")
        || name.contains("router")
        || name.contains("routes");
    if !looks_rr {
        return;
    }
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let path_re = PATH_RE.get_or_init(|| Regex::new(r#"path:\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in path_re.captures_iter(content) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if raw.is_empty() || raw.contains('.') {
            continue;
        }
        let route = if raw.starts_with('/') {
            raw.to_string()
        } else {
            format!("/{raw}")
        };
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(ast, &route, format!("path: \"{route}\""), line);
    }
}

fn remix_file_route(path: &Path) -> Option<String> {
    let s = path.to_string_lossy().replace('\\', "/");
    let after = s.split("/routes/").nth(1)?;
    if after.contains("+page") || after.contains("+layout") || after.contains("+server") {
        return None;
    }
    let without_ext = after.rsplit_once('.').map(|(n, _)| n).unwrap_or(after);
    if without_ext.starts_with('_') && without_ext != "_index" && !without_ext.contains("._index") {
        return None;
    }
    let parts: Vec<&str> = without_ext
        .split('.')
        .filter(|p| !p.is_empty() && *p != "_index" && !p.starts_with('_'))
        .collect();
    Some(join_next_segments(&parts.join("/")))
}

fn axum_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains(".route(") {
        return;
    }
    if !content.contains("axum") && !content.contains("Router::new") {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"\.route\(\s*["']([^"']+)["']\s*,\s*(get|post|put|patch|delete)\s*\("#)
            .unwrap()
    });
    for cap in route_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let method = cap
            .get(2)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".into());
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("{method} {route}"),
            line,
        );
    }
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

fn astro_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    if let Some(route) = file_page_route(path, "pages", "astro") {
        push_api(ast, &route, format!("Astro {route}"), 1);
    }
    let Some(frontmatter) = astro_frontmatter(content) else {
        return;
    };
    let extra = TypeScriptParser::parse(path, frontmatter);
    for sym in extra.symbols {
        if ast.symbols.iter().any(|s| s.name == sym.name) {
            continue;
        }
        ast.symbols.push(sym);
    }
    ast.imports.extend(extra.imports);
    ast.relationships.extend(extra.relationships);
}

fn astro_frontmatter(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let end = rest.find("\n---").or_else(|| rest.find("\r\n---"))?;
    Some(&rest[..end])
}

fn nuxt_overlay(path: &Path, ast: &mut AstAnalysisResult) {
    let Some(route) = file_page_route(path, "pages", "vue") else {
        return;
    };
    push_api(ast, &route, format!("Nuxt {route}"), 1);
}

fn file_page_route(path: &Path, folder: &str, ext: &str) -> Option<String> {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./");
    let marker = format!("/{folder}/");
    let after = s
        .split(&marker)
        .nth(1)
        .or_else(|| s.strip_prefix(&format!("{folder}/")))?;
    if !after.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
        return None;
    }
    let without_ext = after.rsplit_once('.').map(|(n, _)| n).unwrap_or(after);
    let (dir, file) = without_ext.rsplit_once('/').unwrap_or(("", without_ext));
    let rel = if file == "index" {
        dir
    } else if dir.is_empty() {
        file
    } else {
        return Some(join_next_segments(&format!("{dir}/{file}")));
    };
    Some(join_next_segments(rel))
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

    #[test]
    fn fastapi_post_is_api() {
        let src = "from fastapi import FastAPI\napp = FastAPI()\n@app.post(\"/sms\")\ndef store(body: str):\n    SmsStore.save(body)\n";
        let ast = analyze("main.py", src, SourceLanguage::Python);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rails_route_is_api() {
        let src = "Rails.application.routes.draw do\n  post '/sms', to: 'sms#create'\nend\n";
        let ast = analyze("config/routes.rb", src, SourceLanguage::Ruby);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flutter_widget_promotes_to_component() {
        let src = "import 'package:flutter/material.dart';\nclass SmsInbox extends StatelessWidget {\n  void onReceive(String body) { SmsStore.save(body); }\n}\n";
        let ast = analyze("lib/sms_inbox.dart", src, SourceLanguage::Dart);
        let recv = ast
            .symbols
            .iter()
            .find(|s| s.name == "SmsInbox")
            .expect("SmsInbox");
        assert_eq!(recv.symbol_type, NodeType::Component);
    }

    #[test]
    fn astro_page_and_frontmatter() {
        let src = "---\nimport { saveSms } from '../lib/sms_store';\nfunction store() { saveSms('x'); }\n---\n<p>inbox</p>\n";
        let ast = analyze("src/pages/sms.astro", src, SourceLanguage::Astro);
        assert!(
            has_api(&ast, "/sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(ast.symbols.iter().any(|s| s.name == "store"));
        assert!(ast
            .imports
            .iter()
            .any(|i| i.imported_symbols.iter().any(|n| n == "saveSms")));
    }

    #[test]
    fn nuxt_page_is_api() {
        let src = "<script setup>import { saveSms } from '~/lib/sms'</script>\n<template><button /></template>\n";
        let ast = analyze("pages/sms.vue", src, SourceLanguage::Vue);
        assert!(
            has_api(&ast, "/sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn express_post_is_api() {
        let src = "import express from 'express';\nconst app = express();\napp.post('/sms', (req, res) => saveSms(req.body));\n";
        let ast = analyze("src/app.ts", src, SourceLanguage::TypeScript);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn nest_controller_post_is_api() {
        let src = "@Controller('sms')\nexport class SmsController {\n  @Post()\n  store(body: string) { SmsStore.save(body); }\n}\n";
        let ast = analyze("src/sms.controller.ts", src, SourceLanguage::TypeScript);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn angular_component_and_path_are_overlayed() {
        let src = "import { Component } from '@angular/core';\n@Component({ selector: 'sms-inbox', template: '' })\nexport class SmsInboxComponent {\n  store(body: string) { saveSms(body); }\n}\n";
        let ast = analyze(
            "src/sms-inbox.component.ts",
            src,
            SourceLanguage::TypeScript,
        );
        let recv = ast
            .symbols
            .iter()
            .find(|s| s.name == "SmsInboxComponent")
            .expect("SmsInboxComponent");
        assert_eq!(recv.symbol_type, NodeType::Component);
        let routes = analyze(
            "src/sms.routes.ts",
            "import { Routes } from '@angular/router';\nexport const routes: Routes = [{ path: 'sms', component: SmsInboxComponent }];\n",
            SourceLanguage::TypeScript,
        );
        assert!(
            has_api(&routes, "/sms"),
            "symbols = {:?}",
            routes.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn gin_post_is_api() {
        let src = "package main\nimport \"github.com/gin-gonic/gin\"\nfunc main() { r := gin.Default(); r.POST(\"/sms\", store) }\nfunc store(c *gin.Context) { SmsStoreSave() }\n";
        let ast = analyze("main.go", src, SourceLanguage::Go);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn axum_route_is_api() {
        let src = "use axum::{routing::post, Router};\nfn app() -> Router { Router::new().route(\"/sms\", post(store)) }\nasync fn store() { sms_store::save(); }\n";
        let ast = analyze("src/main.rs", src, SourceLanguage::Rust);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn aspnet_map_post_is_api() {
        let src = "var app = WebApplication.Create();\napp.MapPost(\"/sms\", Store);\nstatic void Store(string body) { SmsStore.Save(body); }\n";
        let ast = analyze("Program.cs", src, SourceLanguage::CSharp);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn razor_page_and_code_extract_store() {
        let src =
            "@page \"/sms\"\n@code {\n  void Store(string body) { SmsStore.Save(body); }\n}\n";
        let ast = analyze("Pages/Sms.cshtml", src, SourceLanguage::HTML);
        assert!(
            has_api(&ast, "/sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(
            ast.symbols.iter().any(|s| s.name == "Store"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn swiftui_view_promotes_to_component() {
        let src = "import SwiftUI\nstruct SmsInbox: View {\n  func store(body: String) { SmsStore.save(body) }\n  var body: some View { Text(\"sms\") }\n}\n";
        let ast = analyze("SmsInbox.swift", src, SourceLanguage::Swift);
        let recv = ast
            .symbols
            .iter()
            .find(|s| s.name == "SmsInbox")
            .expect("SmsInbox");
        assert_eq!(recv.symbol_type, NodeType::Component);
    }

    #[test]
    fn ktor_post_is_api() {
        let src = "import io.ktor.server.routing.*\nfun Application.module() { routing { post(\"/sms\") { store() } } }\nfun store() { SmsStore.save(\"\") }\n";
        let ast = analyze("Application.kt", src, SourceLanguage::Kotlin);
        assert!(
            has_api(&ast, "POST /sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn remix_route_module_is_api() {
        let src = "import { saveSms } from '../lib/sms_store';\nexport async function action() { return saveSms('x'); }\n";
        let ast = analyze("app/routes/sms.tsx", src, SourceLanguage::TypeScript);
        assert!(
            has_api(&ast, "/sms"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        let router = analyze(
            "app/router.ts",
            "import { createBrowserRouter } from 'react-router';\nexport const router = createBrowserRouter([{ path: '/sms', action }]);\n",
            SourceLanguage::TypeScript,
        );
        assert!(
            has_api(&router, "/sms"),
            "symbols = {:?}",
            router.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn less_and_svg_extract_tokens_and_ids() {
        let less = analyze(
            "styles/sms.less",
            "@smsUnread: #ef4444;\n.smsBadge(@color) { color: @smsUnread; }\n",
            SourceLanguage::Less,
        );
        assert!(less.symbols.iter().any(|s| s.name == "smsUnread"));
        assert!(less.symbols.iter().any(|s| s.name == "smsBadge"));

        let svg = analyze(
            "assets/sms-inbox.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><symbol id=\"smsInbox\" viewBox=\"0 0 24 24\"><path d=\"M2 4\"/></symbol></svg>\n",
            SourceLanguage::Svg,
        );
        assert!(
            svg.symbols.iter().any(|s| s.name == "smsInbox"),
            "symbols = {:?}",
            svg.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }
}
