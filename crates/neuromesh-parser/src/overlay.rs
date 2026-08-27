use crate::query_extract::{self, Grammar, QueryOptions, CSHARP_QUERIES};
use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use crate::typescript::TypeScriptParser;
use neuromesh_core::{EdgeType, NodeType};
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
            js_module_overlay(path, content, ast);
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
            pinoox_view_render_overlay(path, content, ast);
            pinoox_app_manifest_overlay(path, content, ast);
            pinoox_vite_overlay(content, ast);
            symfony_overlay(content, ast);
            wordpress_overlay(content, ast);
        }
        SourceLanguage::YAML | SourceLanguage::JSON => dotenv_example_overlay(path, content, ast),
        SourceLanguage::Rust => {
            tauri_overlay(content, ast);
            axum_overlay(content, ast);
        }
        SourceLanguage::CSharp => aspnet_overlay(content, ast),
        SourceLanguage::Swift => swiftui_overlay(content, ast),
        SourceLanguage::HTML => razor_overlay(path, content, ast),
        SourceLanguage::Twig => {
            twig_overlay(content, ast);
            pinoox_vite_overlay(content, ast);
        }
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
    static DEF_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"@(?:app|router|api)\.(get|post|put|patch|delete|route)\(\s*["']([^"']+)["']"#)
            .unwrap()
    });
    let def_re = DEF_RE.get_or_init(|| {
        Regex::new(r"(?m)^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
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
        let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let line = line_of(content, start);
        let api_name = format!("{method} {route}");
        push_api(ast, &api_name, format!("@{verb}(\"{route}\")"), line);
        let after = &content[cap.get(0).map(|m| m.end()).unwrap_or(0)..];
        if let Some(def) = def_re.captures(after) {
            let handler = def.get(1).map(|m| m.as_str()).unwrap_or("");
            if !handler.is_empty() {
                link_api_to_handler(ast, &api_name, handler, None, None);
            }
        }
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
    laravel_route_overlay(path, content, ast);
    laravel_eloquent_overlay(path, content, ast);
    laravel_schema_overlay(path, content, ast);
    laravel_factory_seeder_overlay(path, content, ast);
    laravel_blade_overlay(path, content, ast);
}

fn laravel_route_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let rel = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if !rel.contains("/routes/")
        && !rel.starts_with("routes/")
        && !content.contains("Route::")
        && !content.contains("Illuminate\\Support\\Facades\\Route")
    {
        return;
    }
    static ROUTE_RE: OnceLock<Regex> = OnceLock::new();
    static MATCH_RE: OnceLock<Regex> = OnceLock::new();
    static RESOURCE_RE: OnceLock<Regex> = OnceLock::new();
    static ACTION_RE: OnceLock<Regex> = OnceLock::new();
    static HANDLER_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"Route::(get|post|put|patch|delete|any|view)\s*\(\s*['"]([^'"]+)['"]"#)
            .unwrap()
    });
    let match_re = MATCH_RE.get_or_init(|| {
        Regex::new(r#"Route::match\(\s*\[[^\]]+\]\s*,\s*['"]([^'"]+)['"]"#).unwrap()
    });
    let resource_re = RESOURCE_RE.get_or_init(|| {
        Regex::new(r#"Route::(apiResource|resource)\s*\(\s*['"]([^'"]+)['"]"#).unwrap()
    });
    let action_re = ACTION_RE.get_or_init(|| {
        Regex::new(r#"\[\s*([A-Za-z_][A-Za-z0-9_\\]*)::class\s*,\s*['"]([^'"]+)['"]\s*\]"#).unwrap()
    });
    let handler_re = HANDLER_RE.get_or_init(|| {
        Regex::new(
            r#"Route::(get|post|put|patch|delete|any|view)\s*\(\s*['"]([^'"]+)['"]\s*,\s*\[\s*([A-Za-z_][A-Za-z0-9_\\]*)::class\s*,\s*['"]([^'"]+)['"]"#,
        )
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
        let method = if method == "VIEW" {
            "GET".into()
        } else if method == "ANY" {
            "ANY".into()
        } else {
            method
        };
        push_api(
            ast,
            &format!("{method} {route}"),
            format!("Route::{method}(\"{route}\")"),
            line,
        );
    }
    for cap in match_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("MATCH {route}"),
            format!("Route::match(\"{route}\")"),
            line,
        );
    }
    for cap in resource_re.captures_iter(content) {
        let kind = cap.get(1).map(|m| m.as_str()).unwrap_or("resource");
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let prefix = if kind.eq_ignore_ascii_case("apiResource") {
            "API-RESOURCE"
        } else {
            "RESOURCE"
        };
        push_api(
            ast,
            &format!("{prefix} /{route}"),
            format!("Route::{kind}(\"{route}\")"),
            line,
        );
    }
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("routes");
    for cap in action_re.captures_iter(content) {
        let class = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let method = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if class.is_empty() || method.is_empty() {
            continue;
        }
        let short = class.rsplit('\\').next().unwrap_or(class);
        ast.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: method.to_string(),
            relationship: EdgeType::Calls,
            target_file_hint: Some(format!("{short}.php")),
            receiver_hint: Some(short.to_string()),
        });
        ast.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: short.to_string(),
            relationship: EdgeType::References,
            target_file_hint: Some(format!("{short}.php")),
            receiver_hint: None,
        });
    }
    for cap in handler_re.captures_iter(content) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".into());
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let class = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        let action = cap.get(4).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() || class.is_empty() || action.is_empty() {
            continue;
        }
        let method = if method == "VIEW" {
            "GET".into()
        } else if method == "ANY" {
            "ANY".into()
        } else {
            method
        };
        let short = class.rsplit('\\').next().unwrap_or(class);
        link_api_to_handler(
            ast,
            &format!("{method} {route}"),
            action,
            Some(format!("{short}.php")),
            Some(short.to_string()),
        );
    }
}

fn laravel_eloquent_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let is_model_path = path_has_dir(path, "Models") || path_has_dir(path, "Model");
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    let class_re = CLASS_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:(?:abstract|final)\s+)?class\s+([A-Za-z_][A-Za-z0-9_]*)\s+extends\s+(?:\\\\)?(?:Illuminate\\Database\\Eloquent\\)?(Model|Authenticatable|Pivot|User)\b",
        )
        .unwrap()
    });
    let mut found_model = false;
    for cap in class_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::DbModel);
            found_model = true;
        }
    }
    if (is_model_path
        || content.contains("Illuminate\\Database\\Eloquent\\Model")
        || content.contains("HasFactory")
        || content.contains("$fillable")
        || content.contains("$casts"))
        && !found_model
    {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !stem.is_empty()
            && !stem.ends_with("Factory")
            && !stem.ends_with("Seeder")
            && !stem.contains("migration")
        {
            promote(ast, stem, NodeType::DbModel);
        }
    }

    static REL_RE: OnceLock<Regex> = OnceLock::new();
    let rel_re = REL_RE.get_or_init(|| {
        Regex::new(
            r"\b(belongsTo|hasMany|hasOne|belongsToMany|morphTo|morphMany|morphToMany|hasManyThrough|hasOneThrough|morphOne)\s*\(\s*([A-Za-z_][A-Za-z0-9_\\]*)::class",
        )
        .unwrap()
    });
    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    for cap in rel_re.captures_iter(content) {
        let related = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let short = related.rsplit('\\').next().unwrap_or(related);
        if short.is_empty() {
            continue;
        }
        ast.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: short.to_string(),
            relationship: EdgeType::DependsOn,
            target_file_hint: Some(format!("{short}.php")),
            receiver_hint: None,
        });
    }

    static TABLE_RE: OnceLock<Regex> = OnceLock::new();
    let table_re =
        TABLE_RE.get_or_init(|| Regex::new(r#"\$table\s*=\s*['"]([^'"]+)['"]"#).unwrap());
    if let Some(cap) = table_re.captures(content) {
        let table = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if !table.is_empty() {
            if let Some(sym) = ast
                .symbols
                .iter_mut()
                .find(|s| s.symbol_type == NodeType::DbModel)
            {
                let extra = format!("$table = '{table}'");
                match &sym.signature {
                    Some(existing) if existing.contains(table) => {}
                    Some(existing) => sym.signature = Some(format!("{existing}; {extra}")),
                    None => sym.signature = Some(extra),
                }
            }
        }
    }
}

fn laravel_schema_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("Schema::")
        && !content.contains("DB::table")
        && !path_has_dir(path, "migrations")
    {
        return;
    }
    static SCHEMA_RE: OnceLock<Regex> = OnceLock::new();
    let schema_re = SCHEMA_RE.get_or_init(|| {
        Regex::new(r#"Schema::(create|table|dropIfExists|drop)\s*\(\s*['"]([^'"]+)['"]"#).unwrap()
    });
    for cap in schema_re.captures_iter(content) {
        let table = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if table.is_empty() {
            continue;
        }
        let op = cap.get(1).map(|m| m.as_str()).unwrap_or("create");
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        if !ast.symbols.iter().any(|s| s.name == table) {
            ast.symbols.push(ParsedSymbol::new(
                table,
                NodeType::DbModel,
                Some(format!("Schema::{op}('{table}')")),
                line..(line + 1),
                true,
            ));
        }
    }
    static DB_RE: OnceLock<Regex> = OnceLock::new();
    let db_re = DB_RE.get_or_init(|| Regex::new(r#"DB::table\(\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in db_re.captures_iter(content) {
        let table = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if table.is_empty() || ast.symbols.iter().any(|s| s.name == table) {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        ast.symbols.push(ParsedSymbol::new(
            table,
            NodeType::DbModel,
            Some(format!("DB::table('{table}')")),
            line..(line + 1),
            true,
        ));
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if path_has_dir(path, "migrations") && !stem.is_empty() {
        promote(ast, stem, NodeType::Function);
        if let Some(short) = migration_action_name(stem) {
            promote(ast, &short, NodeType::Function);
        }
        if let Some(table) = table_from_migration_stem(stem) {
            if !ast.symbols.iter().any(|s| s.name == table) {
                ast.symbols.push(ParsedSymbol::new(
                    table,
                    NodeType::DbModel,
                    Some(stem.to_string()),
                    1..2,
                    true,
                ));
            }
        }
    }
}

fn migration_action_name(stem: &str) -> Option<String> {
    let lower = stem.to_ascii_lowercase();
    let idx = lower.find("_create_")?;
    Some(format!("create_{}", &lower[idx + "_create_".len()..]))
}

fn table_from_migration_stem(stem: &str) -> Option<String> {
    let lower = stem.to_ascii_lowercase();
    let rest = lower
        .find("_create_")
        .map(|i| &lower[i + "_create_".len()..])
        .or_else(|| {
            lower
                .strip_prefix("create_")
                .map(|s| s.strip_suffix("_table").unwrap_or(s))
        })?;
    let table = rest.strip_suffix("_table").unwrap_or(rest);
    if table.is_empty() {
        None
    } else {
        Some(table.to_string())
    }
}

fn laravel_factory_seeder_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let is_factory = path_has_dir(path, "factories")
        || stem.ends_with("Factory")
        || content.contains("extends Factory");
    let is_seeder = path_has_dir(path, "seeders")
        || path_has_dir(path, "seeds")
        || stem.ends_with("Seeder")
        || content.contains("extends Seeder");
    if is_factory {
        promote(ast, stem, NodeType::Function);
        static MODEL_RE: OnceLock<Regex> = OnceLock::new();
        let model_re = MODEL_RE.get_or_init(|| {
            Regex::new(r#"(?:protected\s+)?\$model\s*=\s*([A-Za-z_][A-Za-z0-9_\\]*)::class"#)
                .unwrap()
        });
        if let Some(cap) = model_re.captures(content) {
            let model = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let short = model.rsplit('\\').next().unwrap_or(model);
            ast.relationships.push(ParsedRelationship {
                source_symbol: stem.to_string(),
                target_symbol: short.to_string(),
                relationship: EdgeType::DependsOn,
                target_file_hint: Some(format!("{short}.php")),
                receiver_hint: None,
            });
        }
    }
    if is_seeder {
        promote(ast, stem, NodeType::Function);
    }
    static FACTORY_CALL_RE: OnceLock<Regex> = OnceLock::new();
    let factory_call_re = FACTORY_CALL_RE
        .get_or_init(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_\\]*)::factory\s*\(").unwrap());
    let filename = if stem.is_empty() { "module" } else { stem };
    for cap in factory_call_re.captures_iter(content) {
        let model = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let short = model.rsplit('\\').next().unwrap_or(model);
        if short.is_empty() {
            continue;
        }
        ast.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: short.to_string(),
            relationship: EdgeType::Calls,
            target_file_hint: Some(format!("{short}.php")),
            receiver_hint: None,
        });
        ast.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: format!("{short}Factory"),
            relationship: EdgeType::Calls,
            target_file_hint: Some(format!("{short}Factory.php")),
            receiver_hint: None,
        });
    }
}

fn laravel_blade_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !name.ends_with(".blade.php")
        && !content.contains("@extends")
        && !content.contains("@section")
    {
        return;
    }
    if !name.ends_with(".blade.php") && !path_has_dir(path, "views") {
        return;
    }
    static DIR_RE: OnceLock<Regex> = OnceLock::new();
    let dir_re = DIR_RE.get_or_init(|| {
        Regex::new(r#"@(?:extends|include|section|component|livewire)\(\s*['"]([^'"]+)['"]"#)
            .unwrap()
    });
    for cap in dir_re.captures_iter(content) {
        let target = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if target.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let ident = target.replace(['.', '/', '\\'], "_");
        if !ast.symbols.iter().any(|s| s.name == ident) {
            ast.symbols.push(ParsedSymbol::new(
                ident.clone(),
                NodeType::Component,
                Some(cap.get(0).unwrap().as_str().trim().to_string()),
                line..(line + 1),
                true,
            ));
        }
    }
}

fn js_module_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    static REQUIRE_RE: OnceLock<Regex> = OnceLock::new();
    static DESTRUCTURE_RE: OnceLock<Regex> = OnceLock::new();
    static SIDE_RE: OnceLock<Regex> = OnceLock::new();
    static EXPORTS_RE: OnceLock<Regex> = OnceLock::new();
    let require_re = REQUIRE_RE
        .get_or_init(|| Regex::new(r#"(?:require|import)\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap());
    let destructure_re = DESTRUCTURE_RE.get_or_init(|| {
        Regex::new(
            r#"(?:const|let|var)\s+(?:\{([^}]+)\}|([A-Za-z_][A-Za-z0-9_]*))\s*=\s*require\(\s*['"]([^'"]+)['"]"#,
        )
        .unwrap()
    });
    let side_re = SIDE_RE.get_or_init(|| {
        Regex::new(
            r#"(?m)^\s*import\s+(?:[A-Za-z_{}*,\s]+from\s+)?['"]([^'"]+\.(?:css|scss|sass|less|json|svg))['"]"#,
        )
        .unwrap()
    });
    let exports_re = EXPORTS_RE.get_or_init(|| {
        Regex::new(r#"(?:module\.exports|exports)\.([A-Za-z_][A-Za-z0-9_]*)\s*="#).unwrap()
    });

    for cap in destructure_re.captures_iter(content) {
        let source_path = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        let mut names = Vec::new();
        if let Some(named) = cap.get(1) {
            for part in named.as_str().split(',') {
                let clean = part.split_whitespace().next().unwrap_or("").trim();
                if !clean.is_empty() {
                    names.push(clean.to_string());
                }
            }
        }
        if let Some(binding) = cap.get(2) {
            names.push(binding.as_str().to_string());
        }
        push_js_import(ast, filename, source_path, &names);
    }
    for cap in require_re.captures_iter(content) {
        let source_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if ast.imports.iter().any(|i| i.source_path == source_path) {
            continue;
        }
        let label = Path::new(source_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");
        push_js_import(ast, filename, source_path, &[label.to_string()]);
    }
    for cap in side_re.captures_iter(content) {
        let source_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if ast.imports.iter().any(|i| i.source_path == source_path) {
            continue;
        }
        let label = Path::new(source_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("asset");
        push_js_import(ast, filename, source_path, &[label.to_string()]);
    }
    for cap in exports_re.captures_iter(content) {
        let name = cap.get(1).unwrap().as_str();
        if !ast.exports.contains(&name.to_string()) {
            ast.exports.push(name.to_string());
        }
        if !ast.symbols.iter().any(|s| s.name == name) {
            promote(ast, name, NodeType::Function);
        }
    }
    static MODULE_EXPORTS_RE: OnceLock<Regex> = OnceLock::new();
    let module_exports_re =
        MODULE_EXPORTS_RE.get_or_init(|| Regex::new(r"module\.exports\s*=\s*\{([^}]+)\}").unwrap());
    if let Some(cap) = module_exports_re.captures(content) {
        for part in cap.get(1).unwrap().as_str().split(',') {
            let name = part.split(':').next().unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !ast.exports.iter().any(|e| e == name)
            {
                ast.exports.push(name.to_string());
            }
        }
    }
}

fn push_js_import(
    ast: &mut AstAnalysisResult,
    filename: &str,
    source_path: &str,
    names: &[String],
) {
    if source_path.is_empty() {
        return;
    }
    if !ast.imports.iter().any(|i| i.source_path == source_path) {
        ast.imports.push(ParsedImport {
            source_path: source_path.to_string(),
            imported_symbols: names.to_vec(),
            is_default: names.len() == 1,
            is_namespace: false,
            line_number: 1,
        });
    }
    for name in names {
        if ast
            .relationships
            .iter()
            .any(|r| r.target_symbol == *name && r.target_file_hint.as_deref() == Some(source_path))
        {
            continue;
        }
        ast.relationships.push(ParsedRelationship {
            source_symbol: filename.to_string(),
            target_symbol: name.clone(),
            relationship: EdgeType::Imports,
            target_file_hint: Some(source_path.to_string()),
            receiver_hint: None,
        });
    }
}

/// Pinx / Pinoox routes: `get('/')->action([MainController::class, 'index'])->name('home')`
/// and the older `action([SmsController::class, 'store'])->name('sms.store')`.
/// See https://github.com/pinoox/pinoox, https://github.com/pinoox/app, https://github.com/pinoox/pincore
fn pinoox_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    if !looks_like_pinoox_routes(path, content) {
        return;
    }
    push_pinoox_class_actions(content, ast);
    push_pinoox_named_actions(content, ast);
    push_pinoox_http_routes(content, ast);
    push_pinoox_collections(content, ast);
}

fn path_has_dir(path: &Path, dir: &str) -> bool {
    let rel = path.to_string_lossy().replace('\\', "/").to_lowercase();
    let dir = dir.to_ascii_lowercase();
    rel == dir || rel.starts_with(&format!("{dir}/")) || rel.contains(&format!("/{dir}/"))
}

fn looks_like_pinoox_routes(path: &Path, content: &str) -> bool {
    path_has_dir(path, "routes")
        || path_has_dir(path, "router")
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("routes.php"))
        || content.contains("Pinoox\\Router")
        || content.contains("Pinoox\\Portal\\Router")
        || content.contains("Pinoox\\Portal\\Route")
        || content.contains("function Pinoox\\Router")
        || content.contains("action([")
        || content.contains("action( [")
        || content.contains("action('")
        || content.contains("action(\"")
}

fn push_pinoox_class_actions(content: &str, ast: &mut AstAnalysisResult) {
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
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let name = cap.get(3).map(|m| m.as_str());
        push_pinoox_controller_api(ast, controller, method, name, line);
    }
}

fn push_pinoox_named_actions(content: &str, ast: &mut AstAnalysisResult) {
    static NAMED_RE: OnceLock<Regex> = OnceLock::new();
    let named_re = NAMED_RE.get_or_init(|| {
        Regex::new(
            r#"action\(\s*['"]([^'"]+)['"]\s*,\s*\[\s*([A-Za-z_][A-Za-z0-9_\\]*)::class\s*,\s*['"]([^'"]+)['"]\s*\]"#,
        )
        .unwrap()
    });
    for cap in named_re.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let controller = cap.get(2).map(|m| m.as_str()).unwrap_or("Controller");
        let method = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        if name.is_empty() || method.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_pinoox_controller_api(ast, controller, method, Some(name), line);
    }
}

fn push_pinoox_http_routes(content: &str, ast: &mut AstAnalysisResult) {
    static HTTP_RE: OnceLock<Regex> = OnceLock::new();
    let http_re = HTTP_RE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(get|post|put|patch|delete|any|query|options|head|fallback)\s*\(\s*(?:['"]([^'"]*)['"])?"#,
        )
        .unwrap()
    });
    static ACTION_INLINE_RE: OnceLock<Regex> = OnceLock::new();
    let action_inline_re = ACTION_INLINE_RE.get_or_init(|| {
        Regex::new(r#"\[\s*([A-Za-z_][A-Za-z0-9_\\]*)::class\s*,\s*['"]([^'"]+)['"]\s*\]"#).unwrap()
    });
    static NAME_RE: OnceLock<Regex> = OnceLock::new();
    let name_re = NAME_RE.get_or_init(|| Regex::new(r#"->name\(\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in http_re.captures_iter(content) {
        let verb = cap
            .get(1)
            .map(|m| m.as_str().to_ascii_uppercase())
            .unwrap_or_else(|| "GET".into());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
        let line = line_of(content, start);
        let window = content
            .get(start..)
            .map(|s| &s[..s.len().min(280)])
            .unwrap_or("");
        if verb == "FALLBACK" {
            push_api(ast, "FALLBACK", "fallback()".into(), line);
        } else if !path.is_empty() || path == "/" || cap.get(2).is_some() {
            let route = if path.is_empty() { "/" } else { path };
            let method = if verb == "ANY" || verb == "QUERY" {
                "ANY"
            } else {
                verb.as_str()
            };
            push_api(
                ast,
                &format!("{method} {route}"),
                format!("{method} \"{route}\""),
                line,
            );
        }
        if let Some(action) = action_inline_re.captures(window) {
            let controller = action.get(1).map(|m| m.as_str()).unwrap_or("Controller");
            let method = action.get(2).map(|m| m.as_str()).unwrap_or("");
            let name = name_re
                .captures(window)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str());
            if !method.is_empty() {
                push_pinoox_controller_api(ast, controller, method, name, line);
            }
        }
    }
}

fn push_pinoox_collections(content: &str, ast: &mut AstAnalysisResult) {
    static COLLECTION_RE: OnceLock<Regex> = OnceLock::new();
    let collection_re =
        COLLECTION_RE.get_or_init(|| Regex::new(r#"\bcollection\(\s*['"]([^'"]*)['"]"#).unwrap());
    for cap in collection_re.captures_iter(content) {
        let path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let route = if path.is_empty() { "/" } else { path };
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        push_api(
            ast,
            &format!("COLLECTION {route}"),
            format!("collection(\"{route}\")"),
            line,
        );
    }
}

fn push_pinoox_controller_api(
    ast: &mut AstAnalysisResult,
    controller: &str,
    method: &str,
    route_name: Option<&str>,
    line: usize,
) {
    let short = controller.rsplit('\\').next().unwrap_or(controller);
    let name = route_name
        .map(str::to_string)
        .unwrap_or_else(|| format!("{short}::{method}"));
    push_api(
        ast,
        &name,
        format!("action([{short}::class, '{method}'])"),
        line,
    );
    if ast.relationships.iter().any(|r| {
        r.source_symbol == name
            && r.target_symbol == short
            && r.relationship == EdgeType::References
    }) {
        return;
    }
    ast.relationships.push(ParsedRelationship {
        source_symbol: name,
        target_symbol: short.to_string(),
        relationship: EdgeType::References,
        target_file_hint: Some(format!("Controller/{short}.php")),
        receiver_hint: None,
    });
}

fn pinoox_view_render_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let looks_pinoox = content.contains("View::")
        || content.contains("Pinoox\\")
        || path_has_dir(path, "Controller")
        || path_has_dir(path, "controller");
    if !content.contains("View::render")
        && !(looks_pinoox && (content.contains("render(") || content.contains("view(")))
    {
        return;
    }
    let theme = extract_pinoox_theme(content);
    static RENDER_RE: OnceLock<Regex> = OnceLock::new();
    let render_re = RENDER_RE.get_or_init(|| {
        Regex::new(r#"(?:View::(?:render|ready)|\brender|\bview)\(\s*['"]([^'"]+)['"]"#).unwrap()
    });
    for cap in render_re.captures_iter(content) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        if raw.is_empty() {
            continue;
        }
        let stem = raw
            .trim_end_matches(".twig")
            .trim_end_matches(".html")
            .trim_start_matches('/')
            .trim();
        if stem.is_empty() || stem.contains("::") {
            continue;
        }
        let themed = format!("theme/{theme}/{stem}.twig");
        let call_line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let source = overlay_call_source(ast, call_line);
        ast.relationships.push(ParsedRelationship {
            source_symbol: source.clone(),
            target_symbol: stem.to_string(),
            relationship: EdgeType::Calls,
            target_file_hint: Some(themed),
            receiver_hint: None,
        });
        if theme != "default" {
            ast.relationships.push(ParsedRelationship {
                source_symbol: source,
                target_symbol: stem.to_string(),
                relationship: EdgeType::Calls,
                target_file_hint: Some(format!("{stem}.twig")),
                receiver_hint: None,
            });
        }
    }
}

fn pinoox_app_manifest_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name != "app.php" {
        return;
    }
    static KEY_RE: OnceLock<Regex> = OnceLock::new();
    let key_re = KEY_RE.get_or_init(|| {
        Regex::new(r#"['"](package|name|theme|lang)['"]\s*=>\s*['"]([^'"]+)['"]"#).unwrap()
    });
    for cap in key_re.captures_iter(content) {
        let key = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let value = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if value.is_empty() {
            continue;
        }
        let line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let symbol = if key == "package" {
            value.to_string()
        } else {
            format!("{key}:{value}")
        };
        if ast.symbols.iter().any(|s| s.name == symbol) {
            continue;
        }
        ast.symbols.push(ParsedSymbol::new(
            symbol,
            NodeType::Config,
            Some(format!("{key} => {value}")),
            line..(line + 1),
            true,
        ));
    }
    if content.contains("'pinx'") || content.contains("\"pinx\"") {
        promote(ast, "pinx", NodeType::Config);
    }
}

fn pinoox_vite_overlay(content: &str, ast: &mut AstAnalysisResult) {
    if !content.contains("vite(") && !content.contains("vite_tags(") {
        return;
    }
    static VITE_RE: OnceLock<Regex> = OnceLock::new();
    let vite_re =
        VITE_RE.get_or_init(|| Regex::new(r#"\b(?:vite|vite_tags)\(\s*['"]([^'"]+)['"]"#).unwrap());
    for cap in vite_re.captures_iter(content) {
        let entry = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        if entry.is_empty() {
            continue;
        }
        let call_line = line_of(content, cap.get(0).map(|m| m.start()).unwrap_or(0));
        let source = overlay_call_source(ast, call_line);
        let hint = if entry.contains('.') {
            entry.to_string()
        } else {
            format!("{entry}.js")
        };
        ast.relationships.push(ParsedRelationship {
            source_symbol: source,
            target_symbol: entry.to_string(),
            relationship: EdgeType::Calls,
            target_file_hint: Some(hint),
            receiver_hint: None,
        });
    }
}

/// Prefer the function whose span contains the call so `trace` on
/// `MainController::index` sees the Twig edge, not only the class node.
fn overlay_call_source(ast: &AstAnalysisResult, call_line: usize) -> String {
    let mut best: Option<(&str, usize)> = None;
    for symbol in &ast.symbols {
        if symbol.symbol_type != NodeType::Function {
            continue;
        }
        if symbol.line_range.start <= call_line && call_line < symbol.line_range.end {
            let span = symbol
                .line_range
                .end
                .saturating_sub(symbol.line_range.start);
            if best.map(|(_, current)| span < current).unwrap_or(true) {
                best = Some((symbol.name.as_str(), span));
            }
        }
    }
    if let Some((name, _)) = best {
        return name.to_string();
    }
    ast.symbols
        .iter()
        .find(|s| matches!(s.symbol_type, NodeType::Class | NodeType::Component))
        .or_else(|| {
            ast.symbols
                .iter()
                .find(|s| s.symbol_type == NodeType::Function)
        })
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "index".to_string())
}

fn extract_pinoox_theme(content: &str) -> String {
    static THEME_RE: OnceLock<Regex> = OnceLock::new();
    let theme_re =
        THEME_RE.get_or_init(|| Regex::new(r#"['"]theme['"]\s*=>\s*['"]([^'"]+)['"]"#).unwrap());
    theme_re
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".into())
}

fn dotenv_example_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        name.as_str(),
        ".env.example" | ".env.sample" | ".env.dist" | ".env.template"
    ) {
        return;
    }
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let key = line.split('=').next().unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        ast.symbols.push(ParsedSymbol::new(
            key,
            NodeType::Config,
            Some(format!("{key}=")),
            (i + 1)..(i + 2),
            true,
        ));
    }
}

fn php_controller_overlay(path: &Path, ast: &mut AstAnalysisResult) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if stem.ends_with("Controller")
        || path_has_dir(path, "Controller")
        || path_has_dir(path, "Controllers")
        || path_has_dir(path, "Flow")
        || path_has_dir(path, "Router")
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
        || content.contains("from \"react/")
        || content.contains("from 'react-dom")
        || content.contains("from \"react-dom")
        || content.contains("React.FC")
        || content.contains("React.FunctionComponent")
        || content.contains(": FC<")
        || content.contains(": FC =");
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
            r"(?m)^\s*(?:export\s+default\s+)?(?:export\s+)?(?:const|let)\s+([A-Z][A-Za-z0-9_]*)\s*(?::[^=]{1,80})?\s*=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_][A-Za-z0-9_]*)\s*=>",
        )
        .unwrap()
    });
    static WRAP_RE: OnceLock<Regex> = OnceLock::new();
    let wrap_re = WRAP_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*(?:export\s+)?(?:const|let)\s+([A-Z][A-Za-z0-9_]*)\s*(?::[^=]{1,80})?\s*=\s*(?:React\.)?(?:memo|forwardRef)\s*\(",
        )
        .unwrap()
    });
    for cap in fn_re
        .captures_iter(content)
        .chain(arrow_re.captures_iter(content))
        .chain(wrap_re.captures_iter(content))
    {
        if let Some(name) = cap.get(1) {
            promote(ast, name.as_str(), NodeType::Component);
        }
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if matches!(ext.as_str(), "tsx" | "jsx")
        && stem.starts_with(char::is_uppercase)
        && (content.contains("return (") || content.contains("return <") || content.contains("/>"))
    {
        promote(ast, stem, NodeType::Component);
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
        || content.contains("@primeuix")
        || content.contains("PrimeVue");
    if !uses_prime {
        return;
    }
    static IMPORT_RE: OnceLock<Regex> = OnceLock::new();
    let import_re = IMPORT_RE.get_or_init(|| {
        Regex::new(
            r#"import\s+(?:([A-Z][A-Za-z0-9_]*)\s*,?\s*)?(?:\{\s*([^}]+)\s*\}\s*)?from\s*['"](?:primevue|primereact|@primeuix)[^'"]*['"]"#,
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
    static NAMED_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"(?:app|router|r)\.(get|post|put|patch|delete)\s*\(\s*['"]([^'"]+)['"]"#)
            .unwrap()
    });
    let named_re = NAMED_RE.get_or_init(|| {
        Regex::new(
            r#"(?:app|router|r)\.(get|post|put|patch|delete)\s*\(\s*['"]([^'"]+)['"]\s*,\s*([A-Za-z_][A-Za-z0-9_]*)\s*[,)]"#,
        )
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
    for cap in named_re.captures_iter(content) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".into());
        let route = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let handler = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() || handler.is_empty() {
            continue;
        }
        link_api_to_handler(ast, &format!("{method} {route}"), handler, None, None);
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
    static NAMED_RE: OnceLock<Regex> = OnceLock::new();
    let route_re = ROUTE_RE.get_or_init(|| {
        Regex::new(r#"\.route\(\s*["']([^"']+)["']\s*,\s*(get|post|put|patch|delete)\s*\("#)
            .unwrap()
    });
    let named_re = NAMED_RE.get_or_init(|| {
        Regex::new(
            r#"\.route\(\s*["']([^"']+)["']\s*,\s*(get|post|put|patch|delete)\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)"#,
        )
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
    for cap in named_re.captures_iter(content) {
        let route = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let method = cap
            .get(2)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".into());
        let handler = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        if route.is_empty() || handler.is_empty() {
            continue;
        }
        link_api_to_handler(ast, &format!("{method} {route}"), handler, None, None);
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

/// Walkable edge from an Api route node to a named handler (same pattern as Twig).
fn link_api_to_handler(
    ast: &mut AstAnalysisResult,
    api_name: &str,
    handler: &str,
    target_file_hint: Option<String>,
    receiver_hint: Option<String>,
) {
    if api_name.is_empty() || handler.is_empty() {
        return;
    }
    if ast.relationships.iter().any(|r| {
        r.source_symbol == api_name
            && r.target_symbol == handler
            && r.relationship == EdgeType::Calls
            && r.target_file_hint == target_file_hint
    }) {
        return;
    }
    ast.relationships.push(ParsedRelationship {
        source_symbol: api_name.to_string(),
        target_symbol: handler.to_string(),
        relationship: EdgeType::Calls,
        target_file_hint,
        receiver_hint,
    });
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
        assert!(
            ast.relationships.iter().any(|r| {
                r.source_symbol == "POST /sms"
                    && r.target_symbol == "store"
                    && r.relationship == EdgeType::Calls
                    && r.target_file_hint.as_deref() == Some("SmsController.php")
            }),
            "POST /sms must Call store, relationships = {:?}",
            ast.relationships
        );
        assert!(
            ast.relationships
                .iter()
                .any(|r| r.source_symbol == "web" && r.target_symbol == "store"),
            "filename→handler edge must remain, relationships = {:?}",
            ast.relationships
        );
    }

    #[test]
    fn axum_fastapi_named_express_link_handlers() {
        let axum = "use axum::{routing::post, Router};\nfn app() -> Router { Router::new().route(\"/sms\", post(store)) }\nasync fn store() {}\n";
        let ast = analyze("src/main.rs", axum, SourceLanguage::Rust);
        assert!(has_api(&ast, "POST /sms"));
        assert!(
            ast.relationships.iter().any(|r| {
                r.source_symbol == "POST /sms"
                    && r.target_symbol == "store"
                    && r.relationship == EdgeType::Calls
            }),
            "axum relationships = {:?}",
            ast.relationships
        );

        let fastapi = "@app.post(\"/sms\")\ndef store(body: str):\n    return body\n";
        let ast = analyze("src/main.py", fastapi, SourceLanguage::Python);
        assert!(has_api(&ast, "POST /sms"));
        assert!(
            ast.relationships.iter().any(|r| {
                r.source_symbol == "POST /sms"
                    && r.target_symbol == "store"
                    && r.relationship == EdgeType::Calls
            }),
            "fastapi relationships = {:?}",
            ast.relationships
        );

        let named = "import express from \"express\";\nconst app = express();\nfunction store() {}\napp.post(\"/sms\", store);\n";
        let ast = analyze("src/app.ts", named, SourceLanguage::TypeScript);
        assert!(has_api(&ast, "POST /sms"));
        assert!(
            ast.relationships.iter().any(|r| {
                r.source_symbol == "POST /sms"
                    && r.target_symbol == "store"
                    && r.relationship == EdgeType::Calls
            }),
            "named express relationships = {:?}",
            ast.relationships
        );

        let inline = "import express from \"express\";\nconst app = express();\napp.post(\"/sms\", (req) => saveSms(req.body));\n";
        let ast = analyze("src/app.ts", inline, SourceLanguage::TypeScript);
        assert!(has_api(&ast, "POST /sms"));
        assert!(
            !ast.relationships
                .iter()
                .any(|r| r.source_symbol == "POST /sms" && r.relationship == EdgeType::Calls),
            "inline express must not invent a handler edge: {:?}",
            ast.relationships
        );
    }

    #[test]
    fn laravel_resource_eloquent_and_schema() {
        let routes = "<?php\nRoute::resource('sms', SmsController::class);\nRoute::match(['get','post'], '/inbox', [SmsController::class, 'inbox']);\n";
        let ast = analyze("routes/web.php", routes, SourceLanguage::PHP);
        assert!(has_api(&ast, "RESOURCE /sms"));
        assert!(has_api(&ast, "MATCH /inbox"));

        let model = "<?php\nnamespace App\\Models;\nuse Illuminate\\Database\\Eloquent\\Model;\nuse Illuminate\\Database\\Eloquent\\Factories\\HasFactory;\nclass SmsMessage extends Model {\n  use HasFactory;\n  protected $table = 'sms_messages';\n  public function inbox() { return $this->belongsTo(Inbox::class); }\n}\n";
        let ast = analyze("app/Models/SmsMessage.php", model, SourceLanguage::PHP);
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "SmsMessage" && s.symbol_type == NodeType::DbModel),
            "symbols = {:?}",
            ast.symbols
                .iter()
                .map(|s| (&s.name, s.symbol_type))
                .collect::<Vec<_>>()
        );
        assert!(ast.symbols.iter().any(|s| s.name == "SmsMessage"
            && s.signature
                .as_deref()
                .is_some_and(|sig| sig.contains("sms_messages"))));
        assert!(ast.relationships.iter().any(|r| r.target_symbol == "Inbox"));

        let migration = "<?php\nuse Illuminate\\Database\\Migrations\\Migration;\nuse Illuminate\\Support\\Facades\\Schema;\nreturn new class extends Migration {\n  public function up(): void {\n    Schema::create('sms_messages', function ($table) { $table->id(); });\n  }\n};\n";
        let ast = analyze(
            "database/migrations/2024_01_01_000000_create_sms_messages_table.php",
            migration,
            SourceLanguage::PHP,
        );
        assert!(ast.symbols.iter().any(|s| s.name == "sms_messages"));

        let factory = "<?php\nclass SmsMessageFactory extends Factory {\n  protected $model = SmsMessage::class;\n  public function definition(): array { return ['body' => fake()->text()]; }\n}\n";
        let ast = analyze(
            "database/factories/SmsMessageFactory.php",
            factory,
            SourceLanguage::PHP,
        );
        assert!(ast
            .relationships
            .iter()
            .any(|r| r.target_symbol == "SmsMessage"));

        let seeder = "<?php\nclass SmsSeeder extends Seeder {\n  public function run(): void { SmsMessage::factory()->create(); }\n}\n";
        let ast = analyze(
            "database/seeders/SmsSeeder.php",
            seeder,
            SourceLanguage::PHP,
        );
        assert!(ast
            .relationships
            .iter()
            .any(|r| r.target_symbol == "SmsMessageFactory"));
    }

    #[test]
    fn js_require_and_json_css_imports() {
        let src = "const { createStore } = require('./store');\nimport tokens from './tokens.json';\nimport './badge.css';\nfunction saveSms(body) { return createStore({ body }); }\nmodule.exports = { saveSms };\n";
        let ast = analyze("src/saveSms.cjs", src, SourceLanguage::JavaScript);
        assert!(
            ast.imports.iter().any(|i| i.source_path.contains("store")),
            "imports = {:?}",
            ast.imports
                .iter()
                .map(|i| &i.source_path)
                .collect::<Vec<_>>()
        );
        assert!(ast
            .imports
            .iter()
            .any(|i| i.source_path.contains("tokens.json")));
        assert!(ast
            .imports
            .iter()
            .any(|i| i.source_path.contains("badge.css")));
        assert!(ast.exports.iter().any(|e| e == "saveSms"));
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
    fn pinoox_get_action_is_named_api() {
        let src = "<?php\nuse function Pinoox\\Router\\{get, post, collection};\nget('/')->action([MainController::class, 'index'])->name('home');\npost('/sms', [SmsController::class, 'store'])->name('sms.store');\ncollection('/api', 'routes/api.php');\n";
        let ast = analyze("routes/web.php", src, SourceLanguage::PHP);
        assert!(
            has_api(&ast, "home"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(has_api(&ast, "GET /"));
        assert!(has_api(&ast, "POST /sms"));
        assert!(has_api(&ast, "sms.store"));
        assert!(has_api(&ast, "COLLECTION /api"));
    }

    #[test]
    fn pinoox_named_action_string_first() {
        let src = "<?php\naction('home', [MainController::class, 'index']);\n";
        let ast = analyze("routes/web.php", src, SourceLanguage::PHP);
        assert!(
            has_api(&ast, "home"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pinoox_render_helper_links_twig() {
        let src = "<?php class MainController extends Controller { public function index() { return render('hello', [\"title\" => \"Hi\"]); } }\n";
        let ast = analyze("Controller/MainController.php", src, SourceLanguage::PHP);
        assert!(
            ast.relationships.iter().any(|r| {
                r.relationship == EdgeType::Calls
                    && r.target_file_hint
                        .as_deref()
                        .is_some_and(|h| h == "theme/default/hello.twig")
            }),
            "render() should hint the Twig file: {:?}",
            ast.relationships
        );
    }

    #[test]
    fn pinoox_app_php_package_is_config() {
        let src = "<?php\nreturn [\n    'package' => 'com_pinoox_app',\n    'theme' => 'spark',\n    'pinx' => ['type' => 'app'],\n];\n";
        let ast = analyze("app.php", src, SourceLanguage::PHP);
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "com_pinoox_app" && s.symbol_type == NodeType::Config),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(ast
            .symbols
            .iter()
            .any(|s| s.name == "pinx" && s.symbol_type == NodeType::Config));
    }

    #[test]
    fn pinoox_view_render_links_twig() {
        let src = "<?php class MainController extends Controller { public function index() { return View::render('hello', [\"title\" => \"Hi\"]); } }\n";
        let ast = analyze("Controller/MainController.php", src, SourceLanguage::PHP);
        let rel = ast
            .relationships
            .iter()
            .find(|r| {
                r.relationship == EdgeType::Calls
                    && r.target_file_hint
                        .as_deref()
                        .is_some_and(|h| h == "theme/default/hello.twig")
            })
            .expect("View::render should emit a Calls hint to the Twig file");
        assert_eq!(
            rel.source_symbol, "index",
            "edge must attach to the rendering method so trace(MainController::index) sees it: {rel:?}"
        );
        assert_eq!(rel.target_symbol, "hello");
    }

    #[test]
    fn dotenv_example_extracts_keys() {
        let src = "DB_HOST=localhost\n# comment\nAPP_KEY=\n";
        let ast = analyze(".env.example", src, SourceLanguage::YAML);
        assert!(
            ast.symbols.iter().any(|s| s.name == "DB_HOST"),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
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
    fn react_fc_typed_const_promotes() {
        let src = "import { type FC } from 'react';\nexport const StatCard: FC = () => <div />;\n";
        let ast = analyze("src/StatCard.tsx", src, SourceLanguage::TypeScript);
        let recv = ast
            .symbols
            .iter()
            .find(|s| s.name == "StatCard")
            .expect("StatCard");
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
    fn vue_kebab_primevue_tag_is_component() {
        let src = "<script setup>\nimport DataTable from 'primevue/datatable';\n</script>\n<template>\n  <data-table :value=\"rows\" />\n  <Button label=\"Save\" />\n</template>\n";
        let ast = analyze("theme/spark/src/Dashboard.vue", src, SourceLanguage::Vue);
        assert!(
            ast.symbols
                .iter()
                .any(|s| s.name == "DataTable" && s.symbol_type == NodeType::Component),
            "symbols = {:?}",
            ast.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(ast
            .relationships
            .iter()
            .any(|r| r.target_symbol == "DataTable"));
        assert!(ast
            .relationships
            .iter()
            .any(|r| r.target_symbol == "Button"));
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
