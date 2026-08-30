use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AliasEntry {
    pub concept: &'static str,
    pub terms: &'static [&'static str],
}

/// Minimal cross-lingual concept clusters — extended for 10-language Express benchmark families.
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
            "ルーティング",
            "маршрут",
            "yönlendir",
            "routage",
            "enrutamiento",
            "rotalar",
            "التوجيه",
        ],
    },
    AliasEntry {
        concept: "middleware",
        terms: &[
            "middleware",
            "pipeline",
            "next()",
            "next",
            "میان‌افزار",
            "میان افزار",
            "لوله",
            "لوله‌ی",
            "خط أنابيب",
            "خط انابيب",
            "中间件",
            "ミドルウェア",
            "промежуточн",
            "ara katman",
            "middlewares",
            "zwischen",
        ],
    },
    AliasEntry {
        concept: "session",
        terms: &[
            "session",
            "cookie",
            "cookies",
            "cookie-session",
            "سشن",
            "کوکی",
            "куки",
            "сессии",
            "çerez",
            "oturum",
            "会话",
            "セッション",
            "تعريف الارتباط",
            "جلسات",
        ],
    },
    AliasEntry {
        concept: "auth",
        terms: &[
            "auth",
            "authentication",
            "login",
            "jwt",
            "token",
            "bearer",
            "احراز",
            "ورود",
            "توکن",
            "اعتبارسنجی",
            "认证",
            "authentification",
        ],
    },
    AliasEntry {
        concept: "jwt",
        terms: &[
            "jwt",
            "json web token",
            "verify jwt",
            "validate token",
            "token expires",
            "توکن",
            "jwt",
            "اعتبارسنجی",
        ],
    },
    AliasEntry {
        concept: "query",
        terms: &[
            "query",
            "querystring",
            "query string",
            "req.query",
            "کوئری",
            "запрос",
            "consulta",
            "requête",
            "查询",
            "クエリ",
            "sorgu",
            "استعلام",
        ],
    },
    AliasEntry {
        concept: "database",
        terms: &[
            "database",
            "db",
            "model",
            "repository",
            "پایگاه",
            "دیتابیس",
            "数据库",
            "datenbank",
        ],
    },
    AliasEntry {
        concept: "render",
        terms: &[
            "render",
            "template",
            "view",
            "engine",
            "رندر",
            "قالب",
            "渲染",
            "шаблон",
            "plantilla",
            "moteur",
            "テンプレート",
        ],
    },
    AliasEntry {
        concept: "static",
        terms: &[
            "static",
            "assets",
            "public",
            "فایل",
            "استاتیک",
            "静态",
            "статическ",
            "statiques",
            "estáticos",
            "statik",
            "statische",
            "الثابتة",
            "静的",
        ],
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

/// Concrete code symbols to seed when an alias cluster matches (NL → code bridge).
static ALIAS_CODE_SEEDS: &[(&str, &[&str])] = &[
    ("middleware", &["app.use", "next", "middleware"]),
    ("routing", &["Router", "route", "app"]),
    ("render", &["res.render", "render", "view", "engine"]),
    ("static", &["express.static", "static", "stat"]),
    ("session", &["cookie", "session", "cookie-session"]),
    ("query", &["req.query", "query", "parseurl", "utils"]),
    (
        "auth",
        &["session", "cookie", "auth", "validateToken", "verifyJwt"],
    ),
    (
        "jwt",
        &[
            "validateToken",
            "verifyJwt",
            "JwtPayload",
            "authMiddleware",
            "token_expires",
        ],
    ),
    ("database", &["req.query", "query"]),
];

/// Canonical concept ids from static alias clusters (NL → concept).
pub fn canonical_concepts() -> &'static [&'static str] {
    &[
        "routing",
        "middleware",
        "session",
        "auth",
        "jwt",
        "query",
        "database",
        "render",
        "static",
        "test",
        "config",
        "refactor",
        "error",
    ]
}

/// Expand prompt tokens with English code terms from minimal alias clusters.
pub fn expand_aliases(prompt: &str) -> Vec<String> {
    let lower = prompt.to_lowercase();
    let mut out: Vec<String> = Vec::new();
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

/// Code-oriented seed queries derived from matched alias clusters (L1 anchor path).
/// Middleware/routing only — broader NL→code bridging runs in `alias_code_seeds_for_prompt`.
pub fn alias_seed_queries(prompt: &str) -> Vec<String> {
    alias_code_seeds_inner(prompt, true)
}

/// All matched alias clusters → code seeds (server-side assisted inference).
pub fn alias_code_seeds_for_prompt(prompt: &str) -> Vec<String> {
    alias_code_seeds_inner(prompt, false)
}

fn alias_code_seeds_inner(prompt: &str, middleware_routing_only: bool) -> Vec<String> {
    let lower = prompt.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    for cluster in ALIAS_CLUSTERS {
        let matched = cluster
            .terms
            .iter()
            .any(|t| lower.contains(&t.to_lowercase()));
        if !matched {
            continue;
        }
        for (concept, seeds) in ALIAS_CODE_SEEDS {
            if cluster.concept != *concept {
                continue;
            }
            if middleware_routing_only && !matches!(*concept, "middleware" | "routing") {
                continue;
            }
            for seed in *seeds {
                if !out.iter().any(|x| x.eq_ignore_ascii_case(seed)) {
                    out.push((*seed).to_string());
                }
            }
        }
    }
    out.truncate(8);
    out
}

/// Inject alias-expanded terms into signature related_concepts (L1 internal expansion).
pub fn inject_alias_expansion(related: &mut Vec<String>, prompt: &str) {
    for term in expand_aliases(prompt) {
        if !related.iter().any(|r| r.eq_ignore_ascii_case(&term)) {
            related.push(term);
        }
    }
    for term in alias_seed_queries(prompt) {
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

    #[test]
    fn alias_seeds_all_families_for_render() {
        let seeds =
            alias_code_seeds_for_prompt("How does res.render() work with template engines?");
        assert!(seeds.iter().any(|s| s == "res.render"));
        assert!(seeds.iter().any(|s| s == "render"));
    }

    #[test]
    fn alias_seeds_session_ru() {
        let seeds = alias_code_seeds_for_prompt("Как работают куки и сессии в Express?");
        assert!(seeds.iter().any(|s| s.eq_ignore_ascii_case("cookie")));
        assert!(seeds.iter().any(|s| s.eq_ignore_ascii_case("session")));
    }
}
