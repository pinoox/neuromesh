use crate::traits::{
    BoxFuture, ChunkStream, CompletionChunk, ModelInfo, Provider, ProviderCapabilities,
    ProviderRequest, ProviderResponse, Usage,
};
use neuromesh_core::{NeuroMeshError, Result};
use reqwest::Client;
use serde_json::Value;

pub struct AnthropicProvider {
    api_key: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: Client::new(),
        }
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn send<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ProviderResponse>> {
        Box::pin(async move {
            let url = "https://api.anthropic.com/v1/messages";

            let mut system_prompt = None;
            let mut messages = Vec::new();

            for m in &request.messages {
                if m.role == "system" {
                    system_prompt = Some(m.content.clone());
                } else {
                    messages.push(serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    }));
                }
            }

            let mut body = serde_json::json!({
                "model": request.model,
                "messages": messages,
                "max_tokens": request.max_tokens.unwrap_or(4096),
            });

            if let Some(sys) = system_prompt {
                body["system"] = serde_json::json!(sys);
            }

            let resp = self
                .client
                .post(url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await
                .map_err(|e| NeuroMeshError::Provider {
                    provider: "anthropic".to_string(),
                    message: e.to_string(),
                })?;

            if !resp.status().is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(NeuroMeshError::Provider {
                    provider: "anthropic".to_string(),
                    message: format!("HTTP error: {}", err_text),
                });
            }

            let json: Value = resp.json().await.map_err(|e| NeuroMeshError::Provider {
                provider: "anthropic".to_string(),
                message: e.to_string(),
            })?;

            let id = json["id"].as_str().unwrap_or("msg-unknown").to_string();
            let content = json["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let prompt_tokens = json["usage"]["input_tokens"].as_u64().unwrap_or(0) as usize;
            let completion_tokens = json["usage"]["output_tokens"].as_u64().unwrap_or(0) as usize;

            Ok(ProviderResponse {
                id,
                model: request.model.clone(),
                content,
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                },
                finish_reason: Some("end_turn".to_string()),
            })
        })
    }

    fn stream<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ChunkStream>> {
        Box::pin(async move {
            let resp = self.send(request).await?;
            let chunk = CompletionChunk {
                id: resp.id,
                delta: resp.content,
                finish_reason: resp.finish_reason,
            };
            Ok(Box::pin(futures_util::stream::once(async move { Ok(chunk) })) as ChunkStream)
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_vision: true,
            supports_function_calling: true,
            context_window_max: 200000,
        }
    }

    fn model_info(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-3-7-sonnet-20250219".into(),
                display_name: "Claude 3.7 Sonnet".into(),
                context_window: 200000,
                supports_streaming: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".into(),
                display_name: "Claude 3.5 Haiku".into(),
                context_window: 200000,
                supports_streaming: true,
                supports_tools: true,
            },
        ]
    }
}
