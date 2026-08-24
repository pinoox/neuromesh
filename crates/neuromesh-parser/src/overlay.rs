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
        SourceLanguage::TypeScript | SourceLanguage::JavaScript => next_overlay(path, ast),
        SourceLanguage::PHP => laravel_overlay(path, content, ast),
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

    #[test]
    fn android_receiver_promotes_to_component() {
        let src = "package com.example.app\nimport android.content.BroadcastReceiver\nclass SmsReceiver : BroadcastReceiver() {\n  override fun onReceive(body: String?) { SmsStore.save(body) }\n}\n";
        let ast = CodeIntelligenceEngine::analyze(
            &PathBuf::from("src/SmsReceiver.kt"),
            src,
            SourceLanguage::Kotlin,
        );
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
        let ast = CodeIntelligenceEngine::analyze(
            &PathBuf::from("SmsController.java"),
            src,
            SourceLanguage::Java,
        );
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "GET /sms" && s.symbol_type == NodeType::Api),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn django_path_is_api() {
        let src = "from django.urls import path\nurlpatterns = [path(\"sms/\", views.save)]\n";
        let ast = CodeIntelligenceEngine::analyze(
            &PathBuf::from("app/urls.py"),
            src,
            SourceLanguage::Python,
        );
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "sms/" && s.symbol_type == NodeType::Api));
    }

    #[test]
    fn next_app_route_is_api() {
        let src = "export async function POST() { return saveSms(); }\n";
        let ast = CodeIntelligenceEngine::analyze(
            &PathBuf::from("app/api/sms/route.ts"),
            src,
            SourceLanguage::TypeScript,
        );
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "/api/sms" && s.symbol_type == NodeType::Api),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn laravel_route_is_api() {
        let src = "<?php\nRoute::post('/sms', [SmsController::class, 'store']);\n";
        let ast = CodeIntelligenceEngine::analyze(
            &PathBuf::from("routes/web.php"),
            src,
            SourceLanguage::PHP,
        );
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "POST /sms" && s.symbol_type == NodeType::Api),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }
}
