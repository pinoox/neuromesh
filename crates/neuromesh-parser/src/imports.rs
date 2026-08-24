use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship};
use neuromesh_core::EdgeType;

const SOURCE_EXTS: &[&str] = &[".dart", ".swift", ".rb", ".cs", ".kt", ".java", ".py"];

fn strip_source_ext(spec: &str) -> String {
    let mut spec = spec.to_string();
    for ext in SOURCE_EXTS {
        if let Some(stripped) = spec.strip_suffix(ext) {
            spec = stripped.to_string();
            break;
        }
    }
    spec
}

/// Turn a module spec into a path-like hint so `path_hint_matches` can require
/// more than the last dotted segment (`com.example.SmsStore` → `com/example/SmsStore`).
pub fn normalize_module_hint(source: &str) -> String {
    let trimmed = strip_source_ext(source.trim().trim_matches(['"', '\'', '`', ';']).trim());
    if trimmed.starts_with('.') || trimmed.contains('/') {
        return trimmed.replace('\\', "/");
    }
    trimmed.replace("::", "/").replace(['.', '\\'], "/")
}

/// Last identifier in a dotted / slashed / namespaced import spec.
pub fn last_import_segment(source: &str) -> String {
    let spec = strip_source_ext(
        source
            .trim()
            .trim_matches(['"', '\'', '`', ';'])
            .trim_end_matches(['*', ';'])
            .trim_end_matches('.'),
    );
    spec.split(['/', '\\', '.', ':'])
        .map(str::trim)
        .rfind(|s| !s.is_empty() && *s != "*")
        .unwrap_or("")
        .to_string()
}

pub fn split_import_alias(spec: &str) -> (String, Option<String>) {
    let spec = spec.trim().trim_end_matches(';').trim();
    if let Some((left, right)) = spec.rsplit_once(" as ") {
        (left.trim().to_string(), Some(right.trim().to_string()))
    } else {
        (spec.to_string(), None)
    }
}

/// Expand `use foo::{Bar, baz as Qux}` into individual imported names.
pub fn expand_rust_use(spec: &str) -> Vec<(String, String)> {
    let spec = spec.trim().trim_end_matches(';').trim();
    let mut out = Vec::new();
    expand_tree(spec, "", &mut out);
    out
}

fn expand_tree(spec: &str, prefix: &str, out: &mut Vec<(String, String)>) {
    let spec = spec.trim();
    if spec.is_empty() {
        return;
    }

    if let Some((head, rest)) = split_brace_group(spec) {
        let next_prefix = if prefix.is_empty() {
            head.to_string()
        } else {
            format!("{prefix}::{head}")
        };
        for part in split_top_level(rest) {
            expand_tree(&part, &next_prefix, out);
        }
        return;
    }

    let (name_part, alias) = if let Some((left, right)) = spec.rsplit_once(" as ") {
        (left.trim(), Some(right.trim()))
    } else {
        (spec, None)
    };

    let full = if prefix.is_empty() {
        name_part.to_string()
    } else if name_part == "self" {
        prefix.to_string()
    } else {
        format!("{prefix}::{name_part}")
    };

    let imported = alias
        .map(|a| a.to_string())
        .or_else(|| full.split("::").last().map(|s| s.to_string()))
        .unwrap_or_default();

    if imported.is_empty() || imported == "*" || imported == "self" || imported == "super" {
        return;
    }
    out.push((imported, full));
}

fn split_brace_group(spec: &str) -> Option<(&str, &str)> {
    let open = spec.find("::{")?;
    let inner_start = open + 3;
    let close = spec.rfind('}')?;
    if close <= inner_start {
        return None;
    }
    let head = spec[..open].trim();
    let inner = spec[inner_start..close].trim();
    Some((head, inner))
}

fn split_top_level(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '{' => {
                depth += 1;
                current.push(ch);
            }
            '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

pub fn record_import(
    result: &mut AstAnalysisResult,
    source_symbol: &str,
    imported: String,
    source_path: String,
    line_number: usize,
) {
    if imported.is_empty() {
        return;
    }
    result.imports.push(ParsedImport {
        source_path: source_path.clone(),
        imported_symbols: vec![imported.clone()],
        is_default: false,
        is_namespace: false,
        line_number,
    });
    result.relationships.push(ParsedRelationship {
        source_symbol: source_symbol.to_string(),
        target_symbol: imported,
        relationship: EdgeType::Imports,
        target_file_hint: Some(source_path),
        receiver_hint: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_grouped_use() {
        let items = expand_rust_use("neuromesh_core::{NodeId, OptimizationMode, Result}");
        let names: Vec<_> = items.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"NodeId"));
        assert!(names.contains(&"OptimizationMode"));
        assert!(names.contains(&"Result"));
    }

    #[test]
    fn expands_alias() {
        let items = expand_rust_use("foo::bar as Baz");
        assert_eq!(items, vec![("Baz".into(), "foo::bar".into())]);
    }

    #[test]
    fn dotted_module_becomes_slash_hint() {
        assert_eq!(
            normalize_module_hint("com.example.app.SmsStore"),
            "com/example/app/SmsStore"
        );
        assert_eq!(normalize_module_hint("./store"), "./store");
        assert_eq!(last_import_segment("App\\Installer\\Foo"), "Foo");
        assert_eq!(last_import_segment("sms_store.dart"), "sms_store");
        assert_eq!(normalize_module_hint("sms_store.dart"), "sms_store");
    }
}
