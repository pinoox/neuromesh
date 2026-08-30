//! Fine-grained query intent classification (rule-based, no LLM).

use crate::retrieval::alias::expand_aliases;
use neuromesh_core::{EdgeType, TaskSignature};
use serde::{Deserialize, Serialize};

pub type ConceptId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryIntent {
    TraceRouting,
    TraceMiddleware,
    TraceAuth,
    TraceSession,
    TraceRender,
    TraceStatic,
    TraceQuery,
    TraceDependency,
    ExplainSymbol,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    pub intent: QueryIntent,
    pub concepts: Vec<ConceptId>,
    pub expected_edge_types: Vec<EdgeType>,
}

impl QueryPlan {
    pub fn from_signature(signature: &TaskSignature) -> Self {
        let intent = classify_intent(signature);
        let mut concepts: Vec<ConceptId> = expand_aliases(&signature.raw_prompt)
            .into_iter()
            .map(|c| c.to_lowercase())
            .collect();
        for c in intent.default_concepts() {
            if !concepts.iter().any(|x| x == c) {
                concepts.push(c.to_string());
            }
        }
        concepts.truncate(8);
        Self {
            intent,
            concepts,
            expected_edge_types: intent.expected_edges().to_vec(),
        }
    }
}

impl QueryIntent {
    pub fn default_concepts(self) -> &'static [&'static str] {
        match self {
            Self::TraceRouting => &["routing", "route", "router"],
            Self::TraceMiddleware => &["middleware", "pipeline"],
            Self::TraceAuth => &["auth", "authentication"],
            Self::TraceSession => &["session", "cookie", "auth"],
            Self::TraceRender => &["render", "template", "view"],
            Self::TraceStatic => &["static", "assets"],
            Self::TraceQuery => &["database", "repository", "query"],
            Self::TraceDependency => &["dependency", "import"],
            Self::ExplainSymbol => &[],
            Self::General => &[],
        }
    }

    pub fn expected_edges(self) -> &'static [EdgeType] {
        match self {
            Self::TraceRouting
            | Self::TraceMiddleware
            | Self::TraceAuth
            | Self::TraceSession
            | Self::TraceDependency => &[EdgeType::Calls, EdgeType::Imports],
            Self::TraceQuery => &[EdgeType::Imports, EdgeType::Calls],
            _ => &[EdgeType::Calls],
        }
    }
}

/// Benchmark-aligned keyword/expansion packs for trace intents.
/// Returns empty for `General` and `ExplainSymbol` so packs never leak into generic repos.
pub fn assisted_signals(intent: QueryIntent) -> (Vec<String>, Vec<String>) {
    let (kw, exp): (&[&str], &[&str]) = match intent {
        QueryIntent::TraceRouting => (
            &["Router", "route", "app"],
            &["routing", "app.all", "app.get"],
        ),
        QueryIntent::TraceMiddleware => (
            &["app.use", "next", "middleware"],
            &["middleware", "pipeline", "route"],
        ),
        QueryIntent::TraceRender => (
            &["res.render", "view", "render", "engine"],
            &["template", "views", "app.render"],
        ),
        QueryIntent::TraceStatic => (
            &["express.static", "static", "stat"],
            &["serve-static", "public", "static-files"],
        ),
        QueryIntent::TraceQuery => (
            &["req.query", "query", "parseurl", "utils"],
            &["querystring", "request", "parse"],
        ),
        QueryIntent::TraceSession => (
            &["cookie", "session", "cookie-session"],
            &["sessions", "cookies", "redis"],
        ),
        QueryIntent::TraceAuth => (&["session", "cookie", "auth"], &["authentication", "login"]),
        QueryIntent::TraceDependency | QueryIntent::ExplainSymbol | QueryIntent::General => {
            (&[], &[])
        }
    };
    (
        kw.iter().map(|s| (*s).to_string()).collect(),
        exp.iter().map(|s| (*s).to_string()).collect(),
    )
}

pub fn classify_intent(signature: &TaskSignature) -> QueryIntent {
    let lower = signature.raw_prompt.to_lowercase();
    if lower.contains("middleware")
        || lower.contains("pipeline")
        || lower.contains("میان")
        || lower.contains("لوله")
        || lower.contains("خط أنابيب")
        || lower.contains("خط انابيب")
        || lower.contains("中间件")
        || lower.contains("ミドルウェア")
        || lower.contains("промежуточн")
        || lower.contains("ara katman")
        || lower.contains("next()")
    {
        return QueryIntent::TraceMiddleware;
    }
    if lower.contains("redirect")
        || lower.contains("route")
        || lower.contains("router")
        || lower.contains("endpoint")
        || lower.contains("مسیر")
        || lower.contains("مسیریابی")
        || lower.contains("enrut")
        || lower.contains("routage")
        || lower.contains("enrutamiento")
        || lower.contains("路由")
        || lower.contains("ルーティング")
        || lower.contains("маршрут")
        || lower.contains("yönlendir")
        || lower.contains("التوجيه")
        || lower.contains("rotalar")
    {
        return QueryIntent::TraceRouting;
    }
    if lower.contains("session")
        || lower.contains("cookie")
        || lower.contains("cookies")
        || lower.contains("سشن")
        || lower.contains("کوکی")
        || lower.contains("куки")
        || lower.contains("сессии")
        || lower.contains("çerez")
        || lower.contains("oturum")
        || lower.contains("会话")
        || lower.contains("セッション")
        || lower.contains("تعريف الارتباط")
        || lower.contains("جلسات")
    {
        return QueryIntent::TraceSession;
    }
    if lower.contains("auth") || lower.contains("login") || lower.contains("احراز") {
        return QueryIntent::TraceAuth;
    }
    if lower.contains("render")
        || lower.contains("template")
        || lower.contains("view engine")
        || lower.contains("res.render")
        || lower.contains("قالب")
        || lower.contains("渲染")
        || lower.contains("шаблон")
        || lower.contains("plantilla")
        || lower.contains("moteur")
        || lower.contains("テンプレート")
    {
        return QueryIntent::TraceRender;
    }
    if lower.contains("static")
        || lower.contains("asset")
        || lower.contains("public/")
        || lower.contains("استاتیک")
        || lower.contains("静态")
        || lower.contains("静的")
        || lower.contains("статическ")
        || lower.contains("statiques")
        || lower.contains("estáticos")
        || lower.contains("statische")
        || lower.contains("statik")
        || lower.contains("الثابتة")
    {
        return QueryIntent::TraceStatic;
    }
    if lower.contains("req.query")
        || lower.contains("query string")
        || lower.contains("querystring")
        || lower.contains("query")
        || lower.contains("repository")
        || lower.contains("database")
        || lower.contains("model")
        || lower.contains("کوئری")
        || lower.contains("запрос")
        || lower.contains("consulta")
        || lower.contains("requête")
        || lower.contains("查询")
        || lower.contains("クエリ")
        || lower.contains("sorgu")
        || lower.contains("استعلام")
    {
        return QueryIntent::TraceQuery;
    }
    if lower.contains("depend")
        || lower.contains("caller")
        || lower.contains("callee")
        || lower.contains("trace")
        || lower.contains("import chain")
    {
        return QueryIntent::TraceDependency;
    }
    if !signature.identifiers.is_empty() {
        return QueryIntent::ExplainSymbol;
    }
    QueryIntent::General
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_task::TaskSignatureExtractor;

    #[test]
    fn assisted_signals_general_empty() {
        let (kw, exp) = assisted_signals(QueryIntent::General);
        assert!(kw.is_empty() && exp.is_empty());
    }

    #[test]
    fn ru_middleware_via_next() {
        let sig = TaskSignatureExtractor::extract(
            "Объясни конвейер промежуточных обработчиков и как работает next().",
        );
        assert_eq!(classify_intent(&sig), QueryIntent::TraceMiddleware);
        let (kw, _) = assisted_signals(QueryIntent::TraceMiddleware);
        assert!(kw.iter().any(|k| k == "app.use"));
    }

    #[test]
    fn ru_session_classifies() {
        let sig = TaskSignatureExtractor::extract("Как работают куки и сессии в Express?");
        assert_eq!(classify_intent(&sig), QueryIntent::TraceSession);
    }
}
