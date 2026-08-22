use crate::anthropic::AnthropicProvider;
use crate::cursor::CursorProvider;
use crate::google::GoogleGeminiProvider;
use crate::mock::MockProvider;
use crate::openai::OpenAIProvider;
use crate::traits::Provider;
use neuromesh_core::{ProviderConfig, ProviderType};
use std::sync::Arc;

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create(config: &ProviderConfig) -> Arc<dyn Provider> {
        let api_key = config.api_key.clone().unwrap_or_default();

        match config.provider_type {
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(api_key, config.base_url.clone())),
            ProviderType::OpenRouter => Arc::new(OpenAIProvider::new_openrouter(api_key)),
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(api_key)),
            ProviderType::Google => Arc::new(GoogleGeminiProvider::new(api_key)),
            ProviderType::Cursor => Arc::new(CursorProvider::new(api_key, config.base_url.clone())),
            ProviderType::Mock | ProviderType::Local => Arc::new(MockProvider::default()),
        }
    }
}
