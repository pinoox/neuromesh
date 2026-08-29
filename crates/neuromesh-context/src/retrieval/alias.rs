use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AliasEntry {
    pub concept: &'static str,
    pub terms: &'static [&'static str],
}

/// Minimal cross-lingual concept clusters (~50 terms) — not benchmark-overfit.
static ALIAS_CLUSTERS: &[AliasEntry] = &[
    AliasEntry {
        concept: "routing",
        terms: &[
            "route",
            "router",
            "routing",
            "endpoint",
            "مسیر",
            "مسیریابی",
            "ruta",
            "routen",
            "路由",
        ],
    },
    AliasEntry {
        concept: "middleware",
        terms: &[
            "middleware",
            "pipeline",
            "میان‌افزار",
            "中间件",
            "middleware",
        ],
    },
    AliasEntry {
        concept: "auth",
        terms: &[
            "auth",
            "authentication",
            "login",
            "session",
            "cookie",
            "احراز",
            "ورود",
            "认证",
            "authentification",
        ],
    },
    AliasEntry {
        concept: "database",
        terms: &[
            "database",
            "db",
            "model",
            "repository",
            "query",
            "پایگاه",
            "دیتابیس",
            "数据库",
            "datenbank",
        ],
    },
    AliasEntry {
        concept: "render",
        terms: &[
            "render", "template", "view", "engine", "رندر", "قالب", "渲染",
        ],
    },
    AliasEntry {
        concept: "static",
        terms: &["static", "assets", "public", "فایل", "استاتیک", "静态"],
    },
    AliasEntry {
        concept: "test",
        terms: &["test", "spec", "mock", "آزمون", "تست", "测试", "prueba"],
    },
    AliasEntry {
        concept: "config",
        terms: &[
            "config",
            "configuration",
            "env",
            "settings",
            "تنظیم",
            "配置",
        ],
    },
    AliasEntry {
        concept: "refactor",
        terms: &[
            "refactor",
            "rename",
            "restructure",
            "بازسازی",
            "重构",
            "refactoriser",
        ],
    },
    AliasEntry {
        concept: "error",
        terms: &[
            "error",
            "bug",
            "fix",
            "exception",
            "خطا",
            "باگ",
            "错误",
            "fehler",
        ],
    },
];

/// Expand prompt tokens with English code terms from minimal alias clusters.
pub fn expand_aliases(prompt: &str) -> Vec<String> {
    let lower = prompt.to_lowercase();
    let mut out = Vec::new();
    for cluster in ALIAS_CLUSTERS {
        if cluster
            .terms
            .iter()
            .any(|t| lower.contains(&t.to_lowercase()))
        {
            out.push(cluster.concept.to_string());
            for term in cluster.terms {
                if term.is_ascii() && term.len() >= 4 {
                    let en = term.to_string();
                    if !out.contains(&en) {
                        out.push(en);
                    }
                }
            }
        }
    }
    out.truncate(12);
    out
}

/// Inject alias-expanded terms into signature related_concepts (L1 internal expansion).
pub fn inject_alias_expansion(related: &mut Vec<String>, prompt: &str) {
    for term in expand_aliases(prompt) {
        if !related.iter().any(|r| r.eq_ignore_ascii_case(&term)) {
            related.push(term);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fa_routing_expands() {
        let terms = expand_aliases("مسیردهی و router را توضیح بده");
        assert!(terms.iter().any(|t| t == "routing" || t == "route"));
    }
}
