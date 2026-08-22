use crate::traits::{
    BoxFuture, ChunkStream, CompletionChunk, ModelInfo, Provider, ProviderCapabilities,
    ProviderRequest, ProviderResponse, Usage,
};
use futures_util::StreamExt;
use neuromesh_core::{NeuroMeshError, Result, TokenCounter};
use reqwest::Client;
use serde_json::Value;

pub struct CursorProvider {
    session_token: String,
    base_url: String,
    client: Client,
}

impl CursorProvider {
    pub fn new(session_token: impl Into<String>, base_url: Option<String>) -> Self {
        let token = session_token.into();
        let base = base_url.unwrap_or_else(|| "https://api2.cursor.sh".to_string());
        Self {
            session_token: token,
            base_url: base.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    fn frame_connect_payload(json_bytes: &[u8]) -> Vec<u8> {
        let mut framed = Vec::with_capacity(5 + json_bytes.len());
        framed.push(0u8); // 0x00 data frame
        framed.extend_from_slice(&(json_bytes.len() as u32).to_be_bytes());
        framed.extend_from_slice(json_bytes);
        framed
    }

    fn unframe_connect_payload(bytes: &[u8]) -> (Vec<String>, Option<String>) {
        let mut results = Vec::new();
        let mut error_msg = None;
        let mut cursor = 0;

        while cursor + 5 <= bytes.len() {
            let _flag = bytes[cursor];
            let len = u32::from_be_bytes([
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
                bytes[cursor + 4],
            ]) as usize;
            cursor += 5;

            if cursor + len <= bytes.len() {
                let frame_slice = &bytes[cursor..cursor + len];
                cursor += len;

                if let Ok(v) = serde_json::from_slice::<Value>(frame_slice) {
                    if let Some(text) = v["text"].as_str() {
                        results.push(text.to_string());
                    }
                    if let Some(detail) = v["error"]["debug"]["details"]["detail"].as_str() {
                        error_msg = Some(format!("Cursor auth error: {}", detail));
                    } else if let Some(msg) = v["error"]["message"].as_str() {
                        error_msg = Some(format!("Cursor error: {}", msg));
                    }
                }
            } else {
                break;
            }
        }

        (results, error_msg)
    }
}

impl Provider for CursorProvider {
    fn name(&self) -> &str {
        "cursor"
    }

    fn send<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ProviderResponse>> {
        Box::pin(async move {
            let mut stream = self.stream(request).await?;
            let mut full_text = String::new();

            while let Some(chunk_res) = stream.next().await {
                let chunk = chunk_res?;
                full_text.push_str(&chunk.delta);
            }

            let prompt_text: String = request.messages.iter().map(|m| m.content.as_str()).collect();
            let prompt_tokens = TokenCounter::count_tokens(&prompt_text);
            let completion_tokens = TokenCounter::count_tokens(&full_text);

            Ok(ProviderResponse {
                id: "chatcmpl-cursor".to_string(),
                model: request.model.clone(),
                content: full_text,
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                },
                finish_reason: Some("stop".to_string()),
            })
        })
    }

    fn stream<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ChunkStream>> {
        Box::pin(async move {
            let effective_token = request
                .api_key
                .as_deref()
                .filter(|k| !k.is_empty() && *k != "sk-neuromesh-local")
                .unwrap_or(&self.session_token);

            let model_name = match request.model.as_str() {
                "claude-3-7-sonnet" => "claude-3-7-sonnet",
                "claude-3-5-sonnet" => "claude-3-5-sonnet-200k",
                "gpt-4o" => "gpt-4o",
                other => other,
            };

            let conversation: Vec<Value> = request
                .messages
                .iter()
                .map(|m| {
                    let msg_type = if m.role == "assistant" { 2 } else { 1 };
                    serde_json::json!({
                        "type": msg_type,
                        "text": m.content,
                    })
                })
                .collect();

            let payload = serde_json::json!({
                "modelDetails": {
                    "modelName": model_name
                },
                "conversation": conversation,
                "explicitContext": {
                    "contextGraph": {}
                }
            });

            let json_bytes = serde_json::to_vec(&payload)?;
            let framed_body = Self::frame_connect_payload(&json_bytes);

            let url = format!("{}/aiserver.v1.AiService/StreamChat", self.base_url);

            let resp = self
                .client
                .post(&url)
                .header("Content-Type", "application/connect+json")
                .header("Connect-Protocol-Version", "1")
                .header("Authorization", format!("Bearer {}", effective_token))
                .header("Cookie", format!("WorkosCursorSessionToken={}", effective_token))
                .header("x-cursor-client-version", "0.45.0")
                .header("x-ghost-mode", "false")
                .body(framed_body)
                .send()
                .await
                .map_err(|e| NeuroMeshError::Provider {
                    provider: "cursor".to_string(),
                    message: format!("Error connecting to Cursor: {}", e),
                })?;

            let status = resp.status();
            if !status.is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(NeuroMeshError::Provider {
                    provider: "cursor".to_string(),
                    message: format!("Cursor HTTP error ({}): {}", status, err_text),
                });
            }

            let byte_stream = resp.bytes_stream();
            let mapped = byte_stream.map(|item| match item {
                Ok(bytes) => {
                    let (deltas, maybe_err) = Self::unframe_connect_payload(&bytes);
                    if let Some(err) = maybe_err {
                        return Err(NeuroMeshError::Provider {
                            provider: "cursor".to_string(),
                            message: err,
                        });
                    }
                    let combined = deltas.join("");
                    Ok(CompletionChunk {
                        id: "chunk".to_string(),
                        delta: combined,
                        finish_reason: None,
                    })
                }
                Err(e) => Err(NeuroMeshError::Provider {
                    provider: "cursor".to_string(),
                    message: e.to_string(),
                }),
            });

            Ok(Box::pin(mapped) as ChunkStream)
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
                id: "claude-3-7-sonnet".into(),
                display_name: "Claude 3.7 Sonnet (Cursor)".into(),
                context_window: 200000,
                supports_streaming: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-5-sonnet".into(),
                display_name: "Claude 3.5 Sonnet (Cursor)".into(),
                context_window: 200000,
                supports_streaming: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "gpt-4o".into(),
                display_name: "GPT-4o (Cursor)".into(),
                context_window: 128000,
                supports_streaming: true,
                supports_tools: true,
            },
        ]
    }
}
