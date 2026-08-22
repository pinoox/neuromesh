use crate::traits::{
    BoxFuture, ChunkStream, CompletionChunk, ModelInfo, Provider, ProviderCapabilities,
    ProviderRequest, ProviderResponse, Usage,
};
use neuromesh_core::{NeuroMeshError, Result};
use reqwest::Client;
use serde_json::Value;

pub struct GoogleGeminiProvider {
    api_key: String,
    client: Client,
}

impl GoogleGeminiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: Client::new(),
        }
    }
}

impl Provider for GoogleGeminiProvider {
    fn name(&self) -> &str {
        "google"
    }

    fn send<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ProviderResponse>> {
        Box::pin(async move {
            let model = if request.model.is_empty() {
                "gemini-2.5-pro"
            } else {
                &request.model
            };

            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model, self.api_key
            );

            let mut contents = Vec::new();
            for m in &request.messages {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                contents.push(serde_json::json!({
                    "role": role,
                    "parts": [{"text": m.content}]
                }));
            }

            let body = serde_json::json!({
                "contents": contents,
            });

            let resp = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| NeuroMeshError::Provider {
                    provider: "google".to_string(),
                    message: e.to_string(),
                })?;

            if !resp.status().is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(NeuroMeshError::Provider {
                    provider: "google".to_string(),
                    message: format!("Google API error: {}", err_text),
                });
            }

            let json: Value = resp.json().await.map_err(|e| NeuroMeshError::Provider {
                provider: "google".to_string(),
                message: e.to_string(),
            })?;

            let content = json["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let prompt_tokens = json["usageMetadata"]["promptTokenCount"]
                .as_u64()
                .unwrap_or(0) as usize;
            let completion_tokens = json["usageMetadata"]["candidatesTokenCount"]
                .as_u64()
                .unwrap_or(0) as usize;

            Ok(ProviderResponse {
                id: "gemini-resp".to_string(),
                model: model.to_string(),
                content,
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                },
                finish_reason: Some("STOP".to_string()),
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
            context_window_max: 1000000,
        }
    }

    fn model_info(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gemini-2.5-pro".into(),
                display_name: "Gemini 2.5 Pro".into(),
                context_window: 1000000,
                supports_streaming: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "gemini-2.5-flash".into(),
                display_name: "Gemini 2.5 Flash".into(),
                context_window: 1000000,
                supports_streaming: true,
                supports_tools: true,
            },
        ]
    }
}
