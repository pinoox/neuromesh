//! Embedding-assisted refinement when rule-based intent is General.

use crate::retrieval::query_intent::{classify_intent, QueryIntent, QueryPlan};
use neuromesh_core::{EmbeddingConfig, TaskSignature};
use neuromesh_embed::{best_intent_match, embed_query_cached, IntentPrototype};

const MIN_PROTOTYPE_COSINE: f32 = 0.55;

fn map_prototype(proto: IntentPrototype) -> QueryIntent {
    match proto {
        IntentPrototype::TraceRouting => QueryIntent::TraceRouting,
        IntentPrototype::TraceMiddleware => QueryIntent::TraceMiddleware,
        IntentPrototype::TraceAuth => QueryIntent::TraceAuth,
        IntentPrototype::TraceRender => QueryIntent::TraceRender,
        IntentPrototype::TraceQuery => QueryIntent::TraceQuery,
        IntentPrototype::TraceDependency => QueryIntent::TraceDependency,
    }
}

pub fn from_signature_with_embeddings(
    signature: &TaskSignature,
    embedding_config: &EmbeddingConfig,
) -> QueryPlan {
    let mut plan = QueryPlan::from_signature(signature);
    if !embedding_config.enabled || !embedding_config.embed_intent_for_general {
        return plan;
    }
    if classify_intent(signature) != QueryIntent::General {
        return plan;
    }
    let query_vec = match embed_query_cached(embedding_config, &signature.raw_prompt) {
        Ok(v) => v,
        Err(_) => return plan,
    };
    let Some((proto, _)) = best_intent_match(&query_vec, MIN_PROTOTYPE_COSINE) else {
        return plan;
    };
    let intent = map_prototype(proto);
    let mut concepts = plan.concepts.clone();
    for c in intent.default_concepts() {
        if !concepts.iter().any(|x| x == c) {
            concepts.push(c.to_string());
        }
    }
    concepts.truncate(8);
    plan.intent = intent;
    plan.concepts = concepts;
    plan.expected_edge_types = intent.expected_edges().to_vec();
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::query_intent::QueryIntent;
    use neuromesh_core::{EmbeddingConfig, TaskSignature};

    #[test]
    fn general_without_embed_flag_unchanged() {
        let sig = TaskSignature::new("fix the thing");
        let cfg = EmbeddingConfig {
            embed_intent_for_general: false,
            ..EmbeddingConfig::default()
        };
        let plan = from_signature_with_embeddings(&sig, &cfg);
        assert_eq!(plan.intent, QueryIntent::General);
    }
}
