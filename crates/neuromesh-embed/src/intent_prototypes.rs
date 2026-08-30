//! Static intent prototype phrases embedded once at warm.

use crate::search::cosine_similarity;
use neuromesh_core::EmbeddingConfig;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentPrototype {
    TraceRouting,
    TraceMiddleware,
    TraceAuth,
    TraceRender,
    TraceQuery,
    TraceDependency,
}

impl IntentPrototype {
    pub fn phrase(self) -> &'static str {
        match self {
            Self::TraceRouting => "trace HTTP route handler registration and dispatch",
            Self::TraceMiddleware => "find middleware pipeline and request filters",
            Self::TraceAuth => "authentication guard login session validation",
            Self::TraceRender => "template view render HTML response",
            Self::TraceQuery => "database repository query SQL fetch records",
            Self::TraceDependency => "import dependency module calls graph",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::TraceRouting,
            Self::TraceMiddleware,
            Self::TraceAuth,
            Self::TraceRender,
            Self::TraceQuery,
            Self::TraceDependency,
        ]
    }
}

struct PrototypeEntry {
    intent: IntentPrototype,
    vector: Vec<f32>,
}

static PROTOTYPES: OnceCell<Arc<Mutex<Vec<PrototypeEntry>>>> = OnceCell::new();

pub fn warm_intent_prototypes(config: &EmbeddingConfig) -> Result<(), crate::EmbedderError> {
    if !config.enabled || !config.embed_intent_for_general {
        return Ok(());
    }
    if PROTOTYPES.get().is_some() {
        return Ok(());
    }
    let arc = crate::Embedder::lazy_global(config.clone())?;
    let mut entries = Vec::new();
    {
        let mut embedder = arc.lock();
        for proto in IntentPrototype::all() {
            let vector = embedder.embed_query(proto.phrase())?;
            entries.push(PrototypeEntry {
                intent: *proto,
                vector,
            });
        }
    }
    let _ = PROTOTYPES.set(Arc::new(Mutex::new(entries)));
    Ok(())
}

pub fn best_intent_match(query_vec: &[f32], min_cosine: f32) -> Option<(IntentPrototype, f32)> {
    let cell = PROTOTYPES.get()?;
    let guard = cell.lock();
    let mut best: Option<(IntentPrototype, f32)> = None;
    for entry in guard.iter() {
        let score = cosine_similarity(&entry.vector, query_vec);
        if score >= min_cosine && best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((entry.intent, score));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prototype_phrases_non_empty() {
        for p in IntentPrototype::all() {
            assert!(p.phrase().len() > 8);
        }
    }
}
