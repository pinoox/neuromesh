use std::path::Path;

/// Lowercase `/`-separated path for segment checks.
pub fn normalized_source_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn has_dir_segment(path: &Path, names: &[&str]) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_lowercase();
        names.iter().any(|wanted| name == *wanted)
    })
}

/// Benchmark / perf harness paths (`packages/bench`, `benches/`, `__benchmarks__/`).
pub fn is_bench_path(path: &Path) -> bool {
    has_dir_segment(
        path,
        &[
            "bench",
            "benches",
            "benchmark",
            "benchmarks",
            "perf",
            "__benchmarks__",
        ],
    )
}

/// i18n message catalogs (`locales/fa.ts`, `i18n/`, `l10n/`).
pub fn is_locale_path(path: &Path) -> bool {
    has_dir_segment(path, &["locales", "locale", "i18n", "l10n"])
}

pub fn is_test_path(path: &Path) -> bool {
    let lower = normalized_source_path(path);
    has_dir_segment(path, &["tests", "test"])
        || lower.contains("_tests.rs")
        || lower.ends_with("/tests.rs")
        || lower.contains("quality_tests")
        || lower.contains("repo_quality_tests")
}

pub fn is_example_path(path: &Path) -> bool {
    has_dir_segment(path, &["examples", "example"])
}

pub fn is_testdata_path(path: &Path) -> bool {
    has_dir_segment(path, &["testdata", "test_data"])
}

/// Older / compat API surfaces (`src/v3/`, `compat/`, `legacy/`).
pub fn is_legacy_path(path: &Path) -> bool {
    has_dir_segment(path, &["v2", "v3", "compat", "legacy", "deprecated"])
        || path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|stem| {
                matches!(
                    stem.to_lowercase().as_str(),
                    "compat" | "legacy" | "deprecated"
                )
            })
}

/// Trimmed parallel API surfaces (`v4/mini/`, `lite/`, bundle-size variants).
pub fn is_alt_surface_path(path: &Path) -> bool {
    has_dir_segment(path, &["mini", "lite", "slim", "light"])
        || path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|stem| {
                matches!(
                    stem.to_lowercase().as_str(),
                    "mini" | "lite" | "slim" | "light"
                )
            })
}

/// Laravel `database/migrations`, `seeders`, `factories`, and raw `.sql` schema files.
pub fn is_schema_path(path: &Path) -> bool {
    let lower = normalized_source_path(path);
    has_dir_segment(path, &["migrations", "seeders", "seeds", "factories"])
        || lower.contains("/database/")
        || lower.ends_with(".sql")
        || has_dir_segment(path, &["sql"])
}

pub fn prompt_targets_database(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("migration")
        || lower.contains("seeder")
        || lower.contains("factory")
        || lower.contains("eloquent")
        || lower.contains("schema::")
        || lower.contains("create table")
        || lower.contains(".sql")
        || (lower.contains(" sql") || lower.contains("sql "))
}

/// Schema *conversion* twins (`to-json-schema.ts`) — related, not parse/validate.
pub fn is_json_schema_path(path: &Path) -> bool {
    let lower = normalized_source_path(path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    stem.contains("json-schema")
        || stem.contains("json_schema")
        || stem.contains("to-json-schema")
        || lower.contains("/json-schema")
}

pub fn is_core_source_path(path: &Path) -> bool {
    has_dir_segment(path, &["core"])
        && !is_bench_path(path)
        && !is_locale_path(path)
        && !is_legacy_path(path)
        && !is_alt_surface_path(path)
}

/// Parallel API surfaces that steal seeds via similar names.
pub fn is_name_collision_decoy(path: &Path) -> bool {
    is_bench_path(path)
        || is_locale_path(path)
        || is_legacy_path(path)
        || is_alt_surface_path(path)
        || is_schema_path(path)
}

/// Test / bench / example / testdata / locale / legacy — indexed but not
/// first-class for ordinary "how does X work" questions.
pub fn is_low_priority_source_path(path: &Path) -> bool {
    is_test_path(path)
        || is_bench_path(path)
        || is_locale_path(path)
        || is_legacy_path(path)
        || is_alt_surface_path(path)
        || is_example_path(path)
        || is_testdata_path(path)
}

/// Paths excluded from embed tier-0 (file ANN) and flat symbol rebuild.
pub fn is_embed_tier_noise_path(path: &Path) -> bool {
    if is_low_priority_source_path(path) {
        return true;
    }
    let lower = normalized_source_path(path);
    if lower.contains("/docs/")
        || lower.ends_with(".md")
        || lower.ends_with(".rst")
        || lower.contains("/editors/")
    {
        return true;
    }
    lower.contains("/types/") || lower.ends_with(".d.ts")
}

/// `apps/com_shop/...` → `apps/com_shop` so multi-app HMVC (Pinoox) stays in-package.
pub fn hmvc_app_prefix(path: &Path) -> Option<String> {
    let parts: Vec<String> = path
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let idx = parts.iter().position(|p| p.eq_ignore_ascii_case("apps"))?;
    let pkg = parts.get(idx + 1)?.as_str();
    if pkg.is_empty() || pkg.starts_with('.') {
        return None;
    }
    Some(format!("apps/{pkg}"))
}

pub fn prompt_targets_bench(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("benchmark")
        || lower.contains("benches")
        || lower.contains("/bench/")
        || lower.contains("bench/")
        || lower.contains("__benchmarks__")
        || lower.contains("/perf/")
}

pub fn prompt_targets_locale(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("locale")
        || lower.contains("i18n")
        || lower.contains("l10n")
        || lower.contains("translat")
}

pub fn prompt_targets_legacy(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("v3")
        || lower.contains("v2")
        || lower.contains("compat")
        || lower.contains("legacy")
        || lower.contains("deprecated")
}

pub fn prompt_targets_alt_surface(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("/mini/")
        || lower.contains(" mini ")
        || lower.contains("minified")
        || lower.contains("lite api")
        || lower.contains("/lite/")
        || lower.contains("slim")
        || lower.contains("lightweight")
}

pub fn prompt_targets_json_schema(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("json-schema")
        || lower.contains("json schema")
        || lower.contains("tojsonschema")
        || lower.contains("to-json-schema")
}

pub fn prompt_targets_types(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("generic")
        || lower.contains("z.infer")
        || lower.contains("infer")
        || lower.contains("type parameter")
        || lower.contains("type-level")
}

/// True when a bench/locale/legacy/schema path is allowed as a seed for this prompt.
pub fn decoy_allowed_for_prompt(path: &Path, prompt: &str) -> bool {
    if is_bench_path(path) {
        return prompt_targets_bench(prompt);
    }
    if is_locale_path(path) {
        return prompt_targets_locale(prompt);
    }
    if is_legacy_path(path) {
        return prompt_targets_legacy(prompt);
    }
    if is_alt_surface_path(path) {
        return prompt_targets_alt_surface(prompt);
    }
    if is_schema_path(path) {
        return schema_path_allowed_for_prompt(path, prompt);
    }
    true
}

/// Match schema decoys to the prompt kind so a "migration" question cannot
/// seed a raw `.sql` twin (and vice versa).
fn schema_path_allowed_for_prompt(path: &Path, prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    let path_l = normalized_source_path(path);
    if path_l.ends_with(".sql") || has_dir_segment(path, &["sql"]) {
        return lower.contains(".sql")
            || lower.contains("create table")
            || lower.contains(" sql")
            || lower.contains("sql ");
    }
    if has_dir_segment(path, &["migrations"]) {
        return lower.contains("migration") || lower.contains("schema::");
    }
    if has_dir_segment(path, &["seeders", "seeds"]) {
        return lower.contains("seeder") || lower.contains("seed");
    }
    if has_dir_segment(path, &["factories"]) {
        return lower.contains("factory");
    }
    prompt_targets_database(prompt)
}

/// Substring match score in `0..=1`, scaled by how much of `name` is `ident`.
/// `safeParse` vs `parse` outranks `parseNestedObject` vs `parse`.
pub fn name_match_specificity(ident: &str, name: &str) -> f32 {
    let ident = ident.to_lowercase();
    let name = name.to_lowercase();
    if ident.is_empty() || name.is_empty() {
        return 0.0;
    }
    if name == ident {
        return 1.0;
    }
    if !name.contains(&ident) {
        return 0.0;
    }
    (ident.len() as f32 / name.len() as f32)
        .sqrt()
        .clamp(0.12, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn classifies_js_bench_and_locale_dirs() {
        assert!(is_bench_path(Path::new("packages/bench/safeparse.ts")));
        assert!(is_bench_path(Path::new("crates/foo/benches/hot.rs")));
        assert!(is_locale_path(Path::new(
            "packages/zod/src/v4/locales/fa.ts"
        )));
        assert!(!is_bench_path(Path::new(
            "packages/zod/src/v4/core/parse.ts"
        )));
        assert!(is_name_collision_decoy(Path::new(
            "packages/bench/compile-validate-vs-parse.ts"
        )));
        assert_eq!(
            hmvc_app_prefix(Path::new("apps/com_shop/Controller/ShopController.php")).as_deref(),
            Some("apps/com_shop")
        );
        assert_eq!(
            hmvc_app_prefix(Path::new("Controller/MainController.php")),
            None
        );
        assert!(is_schema_path(Path::new(
            "database/migrations/2024_01_01_000000_create_sms_messages_table.php"
        )));
        assert!(is_schema_path(Path::new("database/sql/sms_messages.sql")));
        assert!(!is_schema_path(Path::new(
            "app/Http/Controllers/SmsController.php"
        )));
        assert!(prompt_targets_database(
            "How does the create_sms_messages_table migration create sms_messages?"
        ));
        assert!(!prompt_targets_database(
            "How does SmsController store use SmsMessage?"
        ));
        assert!(!decoy_allowed_for_prompt(
            Path::new("database/factories/SmsMessageFactory.php"),
            "How does SmsController store use SmsMessage?"
        ));
        assert!(decoy_allowed_for_prompt(
            Path::new("database/factories/SmsMessageFactory.php"),
            "How does SmsSeeder run SmsMessageFactory definition?"
        ));
        assert!(!decoy_allowed_for_prompt(
            Path::new("database/sql/sms_messages.sql"),
            "How does the create_sms_messages_table migration create the sms_messages table?"
        ));
        assert!(decoy_allowed_for_prompt(
            Path::new("database/migrations/2024_01_01_000000_create_sms_messages_table.php"),
            "How does the create_sms_messages_table migration create the sms_messages table?"
        ));
        assert!(decoy_allowed_for_prompt(
            Path::new("database/sql/sms_messages.sql"),
            "Where is the sms_messages CREATE TABLE in sms_messages.sql?"
        ));
        assert!(!decoy_allowed_for_prompt(
            Path::new("database/migrations/2024_01_01_000000_create_sms_messages_table.php"),
            "Where is the sms_messages CREATE TABLE in sms_messages.sql?"
        ));
        assert!(!decoy_allowed_for_prompt(
            Path::new("packages/bench/safeparse.ts"),
            "where is the safeParse function implemented"
        ));
        assert!(decoy_allowed_for_prompt(
            Path::new("packages/bench/safeparse.ts"),
            "how does the benchmark in packages/bench/safeparse.ts work"
        ));
        assert!(is_alt_surface_path(Path::new(
            "packages/schema/src/v4/mini/schemas.ts"
        )));
        assert!(!decoy_allowed_for_prompt(
            Path::new("packages/schema/src/v4/mini/schemas.ts"),
            "how does parsing work in zod"
        ));
    }

    #[test]
    fn specificity_prefers_tighter_names() {
        let ident = "parse";
        let safe = name_match_specificity(ident, "safeParse");
        let nested = name_match_specificity(ident, "parseNestedObject");
        assert!(
            safe > nested,
            "safeParse ({safe}) should beat parseNestedObject ({nested})"
        );
        assert_eq!(name_match_specificity(ident, "parse"), 1.0);
    }

    #[test]
    fn classifies_legacy_v3_and_allows_when_prompt_names_it() {
        assert!(is_legacy_path(Path::new("packages/schema/src/v3/types.ts")));
        assert!(is_legacy_path(Path::new(
            "packages/zod/src/v4/classic/compat.ts"
        )));
        assert!(!is_legacy_path(Path::new(
            "packages/schema/src/core/parse.ts"
        )));
        assert!(!is_legacy_path(Path::new(
            "packages/zod/src/v4/core/parse.ts"
        )));
        assert!(!decoy_allowed_for_prompt(
            Path::new("packages/schema/src/v3/types.ts"),
            "how does parse report a validation error path"
        ));
        assert!(decoy_allowed_for_prompt(
            Path::new("packages/schema/src/v3/types.ts"),
            "how does parse work in v3"
        ));
        assert!(is_json_schema_path(Path::new(
            "packages/schema/src/v4/core/to-json-schema.ts"
        )));
        assert!(!is_json_schema_path(Path::new(
            "packages/schema/src/core/parse.ts"
        )));
        assert!(is_core_source_path(Path::new(
            "packages/schema/src/core/parse.ts"
        )));
        assert!(prompt_targets_types(
            "how do ZodType generics flow through z.infer"
        ));
        assert!(!prompt_targets_json_schema(
            "how does parse report a validation error path"
        ));
    }

    #[test]
    fn embed_tier_noise_excludes_docs_and_types() {
        assert!(is_embed_tier_noise_path(Path::new(
            "docs/Guides/Plugins.md"
        )));
        assert!(is_embed_tier_noise_path(Path::new("types/plugin.d.ts")));
        assert!(!is_embed_tier_noise_path(Path::new("lib/plugin-utils.js")));
    }
}
