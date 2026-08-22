use crate::state::AppState;
use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use chrono::Utc;
use futures::StreamExt;
use neuromesh_core::{OptimizationMetadata, TokenCounter};
use neuromesh_provider::{ChatMessage, ProviderRequest};
use neuromesh_router::QualityGate;
use neuromesh_task::TaskSignatureExtractor;
use serde_json::{json, Value};
use std::time::Instant;
use uuid::Uuid;

pub async fn chat_completions(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Response {
    let start_time = Instant::now();
    let request_id = format!("chatcmpl-nm-{}", Uuid::new_v4());

    let model = payload["model"]
        .as_str()
        .unwrap_or(&state.config.provider.default_model)
        .to_string();
    let is_stream = payload["stream"].as_bool().unwrap_or(false);

    let raw_messages: Vec<ChatMessage> = payload["messages"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let role = m["role"].as_str()?.to_string();
                    let content = m["content"].as_str()?.to_string();
                    Some(ChatMessage {
                        role,
                        content,
                        name: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let user_prompt = raw_messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // 1. Extract Task Signature
    let signature = TaskSignatureExtractor::extract(&user_prompt);

    // 2. Evaluate Quality Gate
    let gate = QualityGate::evaluate(&signature, state.config.mode);

    // 3. Count Baseline Tokens
    let raw_prompt_str: String = raw_messages.iter().map(|m| m.content.as_str()).collect();
    let tokens_before = TokenCounter::count_tokens(&raw_prompt_str);

    // 4. Activate Context
    let context_view = state.activator.activate(
        &state.graph,
        &signature,
        gate.effective_mode,
    );

    let nodes_before = state.graph.get_all_nodes().len();
    let nodes_after = context_view.active_nodes.len();

    // 5. Construct Optimized Messages
    let mut optimized_messages = Vec::new();
    if gate.allow_optimization && !context_view.active_nodes.is_empty() {
        let mut context_summary = String::from("\n=== NEUROMESH ACTIVE CONTEXT ===\n");
        for active in &context_view.active_nodes {
            context_summary.push_str(&format!(
                "- {} ({:?}, score: {:.2})\n",
                active.node.name, active.node.node_type, active.activation_score
            ));
            if let Some(content) = &active.node.content {
                context_summary.push_str(&format!("```\n{}\n```\n", content));
            }
        }
        context_summary.push_str("================================\n");

        for m in &raw_messages {
            if m.role == "system" {
                let mut combined = m.content.clone();
                combined.push_str(&context_summary);
                optimized_messages.push(ChatMessage {
                    role: m.role.clone(),
                    content: combined,
                    name: None,
                });
            } else {
                optimized_messages.push(m.clone());
            }
        }

        if !raw_messages.iter().any(|m| m.role == "system") {
            optimized_messages.insert(
                0,
                ChatMessage {
                    role: "system".into(),
                    content: format!("System Context:{}", context_summary),
                    name: None,
                },
            );
        }
    } else {
        optimized_messages = raw_messages;
    }

    let optimized_prompt_str: String = optimized_messages.iter().map(|m| m.content.as_str()).collect();
    let tokens_after = TokenCounter::count_tokens(&optimized_prompt_str);
    let token_reduction_pct = if tokens_before > 0 {
        ((tokens_before.saturating_sub(tokens_after)) as f32 / tokens_before as f32) * 100.0
    } else {
        0.0
    };

    let req = ProviderRequest {
        model: model.clone(),
        messages: optimized_messages,
        temperature: payload["temperature"].as_f64().map(|t| t as f32),
        max_tokens: payload["max_tokens"].as_u64().map(|m| m as usize),
        stream: is_stream,
    };

    if is_stream {
        match state.provider.stream(&req).await {
            Ok(stream) => {
                let sse_stream = stream.map(move |chunk_res| {
                    match chunk_res {
                        Ok(chunk) => {
                            let json_chunk = json!({
                                "id": request_id,
                                "object": "chat.completion.chunk",
                                "created": Utc::now().timestamp(),
                                "model": model,
                                "choices": [{
                                    "index": 0,
                                    "delta": { "content": chunk.delta },
                                    "finish_reason": chunk.finish_reason
                                }]
                            });
                            Ok::<_, axum::Error>(Event::default().data(json_chunk.to_string()))
                        }
                        Err(e) => {
                            let err_json = json!({ "error": e.to_string() });
                            Ok(Event::default().data(err_json.to_string()))
                        }
                    }
                });

                Sse::new(sse_stream)
                    .keep_alive(KeepAlive::default())
                    .into_response()
            }
            Err(e) => {
                Json(json!({ "error": { "message": e.to_string() } })).into_response()
            }
        }
    } else {
        let resp = match state.provider.send(&req).await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": { "message": e.to_string() } })).into_response(),
        };

        let latency_ms = start_time.elapsed().as_millis() as u64;

        // Record Telemetry
        let meta = OptimizationMetadata {
            request_id: request_id.clone(),
            task_id: Some(signature.id),
            project_id: state.graph.project_id().clone(),
            mode: gate.effective_mode.to_string(),
            tokens_before,
            tokens_after,
            token_reduction_pct,
            nodes_before,
            nodes_after,
            expansions_count: 0,
            cache_hit: false,
            provider: state.provider.name().to_string(),
            model: model.clone(),
            latency_ms,
            success: true,
            timestamp: Utc::now(),
        };
        state.metrics.record(meta);

        Json(json!({
            "id": request_id,
            "object": "chat.completion",
            "created": Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": resp.content
                },
                "finish_reason": resp.finish_reason.unwrap_or_else(|| "stop".into())
            }],
            "usage": {
                "prompt_tokens": resp.usage.prompt_tokens,
                "completion_tokens": resp.usage.completion_tokens,
                "total_tokens": resp.usage.total_tokens
            },
            "neuromesh_optimization": {
                "tokens_before": tokens_before,
                "tokens_after": tokens_after,
                "reduction_percentage": format!("{:.1}%", token_reduction_pct),
                "nodes_activated": nodes_after,
                "mode": gate.effective_mode.to_string()
            }
        }))
        .into_response()
    }
}

pub async fn list_models(State(state): State<AppState>) -> Json<Value> {
    let models = state.provider.model_info();
    let data: Vec<Value> = models
        .into_iter()
        .map(|m| {
            json!({
                "id": m.id,
                "object": "model",
                "created": 1740000000,
                "owned_by": state.provider.name(),
                "permission": [],
                "root": m.id,
                "parent": null
            })
        })
        .collect();

    Json(json!({
        "object": "list",
        "data": data
    }))
}
