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

pub fn classify_intent(signature: &TaskSignature) -> QueryIntent {
    let lower = signature.raw_prompt.to_lowercase();
    if lower.contains("middleware") || lower.contains("pipeline") || lower.contains("میان") {
        return QueryIntent::TraceMiddleware;
    }
    if lower.contains("redirect")
        || lower.contains("route")
        || lower.contains("router")
        || lower.contains("endpoint")
        || lower.contains("مسیر")
        || lower.contains("enrut")
    {
        return QueryIntent::TraceRouting;
    }
    if lower.contains("session") || lower.contains("cookie") {
        return QueryIntent::TraceSession;
    }
    if lower.contains("auth") || lower.contains("login") || lower.contains("احراز") {
        return QueryIntent::TraceAuth;
    }
    if lower.contains("render") || lower.contains("template") || lower.contains("view engine") {
        return QueryIntent::TraceRender;
    }
    if lower.contains("static") || lower.contains("asset") || lower.contains("public/") {
        return QueryIntent::TraceStatic;
    }
    if lower.contains("database")
        || lower.contains("query")
        || lower.contains("repository")
        || lower.contains("model")
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
