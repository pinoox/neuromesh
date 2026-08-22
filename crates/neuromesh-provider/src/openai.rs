use crate::traits::{
    BoxFuture, ChunkStream, CompletionChunk, ModelInfo, Provider, ProviderCapabilities,
    ProviderRequest, ProviderResponse, Usage,
};
use futures_util::StreamExt;
use neuromesh_core::{NeuroMeshError, Result};
use reqwest::Client;
use serde_json::Value;

pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    default_model: String,
    client: Client,
    provider_name: String,
}

impl OpenAIProvider {
    pub fn new(api_key: impl Into<String>, base_url: Option<String>) -> Self {
        let base = base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        Self {
            api_key: api_key.into(),
            base_url: base.trim_end_matches('/').to_string(),
            default_model: "mimo-v2.5-free".to_string(),
            client: Client::new(),
            provider_name: "openai".to_string(),
        }
    }

    pub fn new_openrouter(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            default_model: "meta/llama-3.3-70b-instruct".to_string(),
            client: Client::new(),
            provider_name: "openrouter".to_string(),
        }
    }

    fn resolve_endpoint_and_model<'a>(&self, model: &'a str, api_key: &str) -> (String, String) {
        let target_model = if model == "neuromesh-auto"
            || model == "auto"
            || model == "default"
            || model == "neuromesh"
            || model.is_empty()
        {
            &self.default_model
        } else {
            model
        };

        // 1. OpenCode Zen models (mimo-v2.5-free) or when base_url points to opencode
        if target_model.contains("mimo") || self.base_url.contains("opencode.ai") {
            return (
                "https://opencode.ai/zen/v1/chat/completions".to_string(),
                target_model.to_string(),
            );
        }

        // 2. If user set a custom base_url that is not standard openai, respect it directly
        if self.base_url != "https://api.openai.com/v1" && !self.base_url.is_empty() {
            return (
                format!("{}/chat/completions", self.base_url.trim_end_matches('/')),
                target_model.to_string(),
            );
        }

        // 3. Key-based detection: NVIDIA NIM key starts with nvapi-
        if api_key.starts_with("nvapi-") {
            let effective = if target_model.starts_with("meta/")
                || target_model.starts_with("nvidia/")
                || target_model.starts_with("deepseek-ai/")
                || target_model.starts_with("qwen/")
                || target_model.starts_with("mistralai/")
            {
                target_model.to_string()
            } else if target_model.contains("r1") || target_model.contains("deepseek") {
                "deepseek-ai/deepseek-r1".to_string()
            } else if target_model.contains("coder") || target_model.contains("qwen") {
                "qwen/qwen2.5-coder-32b-instruct".to_string()
            } else {
                "meta/llama-3.3-70b-instruct".to_string()
            };
            return (
                "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
                effective,
            );
        }

        // 4. Model-based detection:
        if target_model.starts_with("meta/")
            || target_model.starts_with("nvidia/")
            || target_model.starts_with("deepseek-ai/")
            || target_model.starts_with("qwen/")
            || target_model.starts_with("mistralai/")
        {
            (
                "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
                target_model.to_string(),
            )
        } else if target_model.contains("minimax") || target_model.starts_with("abab") {
            (
                "https://api.minimaxi.chat/v1/chat/completions".to_string(),
                target_model.to_string(),
            )
        } else {
            (
                format!("{}/chat/completions", self.base_url.trim_end_matches('/')),
                target_model.to_string(),
            )
        }
    }

    fn select_effective_key<'a>(&'a self, incoming_key: Option<&'a str>, url: &str) -> &'a str {
        let is_valid = |key: &str| -> bool {
            if key.is_empty()
                || key == "sk-neuromesh-local"
                || key.starts_with("${input:")
                || key.contains("chat.lm.secret")
                || key.len() < 10
            {
                return false;
            }
            if url.contains("opencode.ai") {
                return key.starts_with("sk-");
            }
            if url.contains("integrate.api.nvidia.com") {
                return key.starts_with("nvapi-");
            }
            true
        };

        incoming_key
            .filter(|k| is_valid(k))
            .unwrap_or(&self.api_key)
    }
}

impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn send<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ProviderResponse>> {
        Box::pin(async move {
            let incoming = request.api_key.as_deref().unwrap_or("");
            let (url, effective_model) =
                self.resolve_endpoint_and_model(&request.model, incoming);

            let effective_key = self.select_effective_key(request.api_key.as_deref(), &url);

            let mut req_body = serde_json::json!({
                "model": effective_model,
                "messages": request.messages,
                "stream": false,
            });

            if let Some(t) = request.temperature {
                req_body["temperature"] = serde_json::json!(t);
            }
            if let Some(m) = request.max_tokens {
                req_body["max_tokens"] = serde_json::json!(m);
            }

            let resp = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", effective_key))
                .json(&req_body)
                .send()
                .await
                .map_err(|e| NeuroMeshError::Provider {
                    provider: self.provider_name.clone(),
                    message: format!("Error connecting to {}: {}", url, e),
                })?;

            let status = resp.status();
            if !status.is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(NeuroMeshError::Provider {
                    provider: self.provider_name.clone(),
                    message: format!("HTTP error ({}) from {}: {}", status, url, err_text),
                });
            }

            let json: Value = resp.json().await.map_err(|e| NeuroMeshError::Provider {
                provider: self.provider_name.clone(),
                message: e.to_string(),
            })?;

            let id = json["id"].as_str().unwrap_or("chatcmpl-unknown").to_string();
            let model = json["model"].as_str().unwrap_or(&request.model).to_string();
            let content = json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let finish_reason = json["choices"][0]["finish_reason"]
                .as_str()
                .map(|s| s.to_string());

            let prompt_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize;
            let completion_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as usize;

            Ok(ProviderResponse {
                id,
                model,
                content,
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                },
                finish_reason,
            })
        })
    }

    fn stream<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ChunkStream>> {
        Box::pin(async move {
            let incoming = request.api_key.as_deref().unwrap_or("");
            let (url, effective_model) =
                self.resolve_endpoint_and_model(&request.model, incoming);

            let effective_key = self.select_effective_key(request.api_key.as_deref(), &url);

            let mut req_body = serde_json::json!({
                "model": effective_model,
                "messages": request.messages,
                "stream": true,
            });

            if let Some(t) = request.temperature {
                req_body["temperature"] = serde_json::json!(t);
            }

            let resp = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", effective_key))
                .json(&req_body)
                .send()
                .await
                .map_err(|e| NeuroMeshError::Provider {
                    provider: self.provider_name.clone(),
                    message: format!("Error connecting to {}: {}", url, e),
                })?;

            let status = resp.status();
            if !status.is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(NeuroMeshError::Provider {
                    provider: self.provider_name.clone(),
                    message: format!("HTTP stream error ({}) from {}: {}", status, url, err_text),
                });
            }

            let byte_stream = resp.bytes_stream();
            let mapped = byte_stream.map(|item| {
                match item {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut full_delta = String::new();
                        let mut finish = None;

                        for line in text.lines() {
                            let trimmed = line.trim();
                            if let Some(data) = trimmed.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    finish = Some("stop".to_string());
                                    break;
                                }
                                if let Ok(v) = serde_json::from_str::<Value>(data) {
                                    if let Some(delta_str) = v["choices"][0]["delta"]["content"].as_str() {
                                        full_delta.push_str(delta_str);
                                    }
                                    if let Some(fr) = v["choices"][0]["finish_reason"].as_str() {
                                        finish = Some(fr.to_string());
                                    }
                                }
                            }
                        }

                        Ok(CompletionChunk {
                            id: "chunk".to_string(),
                            delta: full_delta,
                            finish_reason: finish,
                        })
                    }
                    Err(e) => Err(NeuroMeshError::Provider {
                        provider: "openai".to_string(),
                        message: e.to_string(),
                    }),
                }
            });

            Ok(Box::pin(mapped) as ChunkStream)
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
                id: "mimo-v2.5-free".into(),
                display_name: "MiMo v2.5 Free (OpenCode Zen)".into(),
                context_window: 1000000,
                supports_streaming: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "meta/llama-3.3-70b-instruct".into(),
                display_name: "Llama 3.3 70B (NVIDIA NIM)".into(),
                context_window: 128000,
                supports_streaming: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "deepseek-ai/deepseek-r1".into(),
                display_name: "DeepSeek R1 (NVIDIA NIM)".into(),
                context_window: 128000,
                supports_streaming: true,
                supports_tools: true,
            },
        ]
    }
}
