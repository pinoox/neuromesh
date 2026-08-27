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

/// Parallel API surfaces that steal seeds via similar names (bench + locale).
pub fn is_name_collision_decoy(path: &Path) -> bool {
    is_bench_path(path) || is_locale_path(path)
}

/// Test / bench / example / testdata / locale — indexed but not first-class for
/// ordinary "how does X work" questions.
pub fn is_low_priority_source_path(path: &Path) -> bool {
    is_test_path(path)
        || is_bench_path(path)
        || is_locale_path(path)
        || is_example_path(path)
        || is_testdata_path(path)
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

/// True when a bench/locale path is allowed as a seed for this prompt.
pub fn decoy_allowed_for_prompt(path: &Path, prompt: &str) -> bool {
    if is_bench_path(path) {
        return prompt_targets_bench(prompt);
    }
    if is_locale_path(path) {
        return prompt_targets_locale(prompt);
    }
    true
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
        assert!(!decoy_allowed_for_prompt(
            Path::new("packages/bench/safeparse.ts"),
            "where is the safeParse function implemented"
        ));
        assert!(decoy_allowed_for_prompt(
            Path::new("packages/bench/safeparse.ts"),
            "how does the benchmark in packages/bench/safeparse.ts work"
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
}
