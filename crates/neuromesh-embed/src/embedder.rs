use crate::search::truncate_and_normalize;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::Arc;

static GLOBAL: OnceCell<Arc<Mutex<Embedder>>> = OnceCell::new();

#[derive(Debug)]
pub enum EmbedderError {
    Init(String),
    Embed(String),
}

impl std::fmt::Display for EmbedderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Init(msg) => write!(f, "embedder init: {msg}"),
            Self::Embed(msg) => write!(f, "embed failed: {msg}"),
        }
    }
}

impl std::error::Error for EmbedderError {}

pub fn format_query_gemma(prompt: &str) -> String {
    format!("task: search result | query: {prompt}")
}

pub fn format_query_minilm(prompt: &str) -> String {
    format!("query: {prompt}")
}

/// Alias for Gemma asymmetric query prefix.
pub fn format_query(prompt: &str) -> String {
    format_query_gemma(prompt)
}

pub fn format_document_gemma(title: &str, kind: &str, signature: &str) -> String {
    let sig = signature.trim();
    if sig.is_empty() {
        format!("title: {title} | text: {kind}")
    } else {
        format!("title: {title} | text: {kind} {sig}")
    }
}

pub fn format_document_minilm(title: &str, kind: &str, signature: &str) -> String {
    let sig = signature.trim();
    if sig.is_empty() {
        format!("passage: {title} {kind}")
    } else {
        format!("passage: {title} {kind} {sig}")
    }
}

pub fn format_document(title: &str, kind: &str, signature: &str) -> String {
    format_document_gemma(title, kind, signature)
}

pub fn format_query_for_model(model: EmbeddingModelId, prompt: &str) -> String {
    match model {
        EmbeddingModelId::Gemma300mQ4 => format_query_gemma(prompt),
        EmbeddingModelId::MiniLmMultilingualQ => format_query_minilm(prompt),
    }
}

pub fn format_document_for_model(
    model: EmbeddingModelId,
    title: &str,
    kind: &str,
    signature: &str,
) -> String {
    match model {
        EmbeddingModelId::Gemma300mQ4 => format_document_gemma(title, kind, signature),
        EmbeddingModelId::MiniLmMultilingualQ => format_document_minilm(title, kind, signature),
    }
}

fn init_options(config: &EmbeddingConfig) -> InitOptions {
    let fastembed_model = match config.model {
        EmbeddingModelId::Gemma300mQ4 => EmbeddingModel::EmbeddingGemma300MQ4,
        EmbeddingModelId::MiniLmMultilingualQ => EmbeddingModel::ParaphraseMLMiniLML12V2Q,
    };
    let mut opts = InitOptions::new(fastembed_model).with_show_download_progress(false);
    if let Some(n) = config.intra_threads {
        opts = opts.with_intra_threads(n);
    }
    opts
}

pub struct Embedder {
    model: TextEmbedding,
    config: EmbeddingConfig,
}

impl Embedder {
    pub fn try_new(config: EmbeddingConfig) -> Result<Self, EmbedderError> {
        let model = TextEmbedding::try_new(init_options(&config))
            .map_err(|e| EmbedderError::Init(e.to_string()))?;
        Ok(Self { model, config })
    }

    pub fn lazy_global(config: EmbeddingConfig) -> Result<Arc<Mutex<Self>>, EmbedderError> {
        if let Some(existing) = GLOBAL.get() {
            let guard = existing.lock();
            if guard.config.model != config.model
                || guard.config.matryoshka_dim != config.matryoshka_dim
            {
                tracing::warn!(
                    "embedding singleton already loaded for {} dim {}; restart MCP to switch models",
                    guard.config.model.as_str(),
                    guard.config.matryoshka_dim
                );
            }
            drop(guard);
            return Ok(existing.clone());
        }
        let embedder = Arc::new(Mutex::new(Self::try_new(config)?));
        let _ = GLOBAL.set(embedder.clone());
        Ok(embedder)
    }

    /// Load singleton and run one dummy query (cold-start amortization).
    pub fn warm(config: EmbeddingConfig) -> Result<(), EmbedderError> {
        if !config.enabled {
            return Ok(());
        }
        let arc = Self::lazy_global(config)?;
        let mut embedder = arc.lock();
        let _ = embedder.embed_query("neuromesh warmup")?;
        Ok(())
    }

    pub fn config(&self) -> &EmbeddingConfig {
        &self.config
    }

    pub fn embed_query(&mut self, prompt: &str) -> Result<Vec<f32>, EmbedderError> {
        let text = format_query_for_model(self.config.model, prompt);
        self.embed_one(&text)
    }

    pub fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let raw = self
            .model
            .embed(texts, None)
            .map_err(|e| EmbedderError::Embed(e.to_string()))?;
        Ok(raw
            .into_iter()
            .map(|mut v| {
                truncate_and_normalize(&mut v, self.config.matryoshka_dim);
                v
            })
            .collect())
    }

    fn embed_one(&mut self, text: &str) -> Result<Vec<f32>, EmbedderError> {
        let mut vectors = self.embed_documents(&[text.to_string()])?;
        vectors
            .pop()
            .ok_or_else(|| EmbedderError::Embed("empty embedding output".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::EmbeddingModelId;

    #[test]
    fn minilm_query_prefix() {
        assert_eq!(
            format_query_for_model(EmbeddingModelId::MiniLmMultilingualQ, "auth middleware"),
            "query: auth middleware"
        );
    }

    #[test]
    fn minilm_document_prefix() {
        assert_eq!(
            format_document_for_model(
                EmbeddingModelId::MiniLmMultilingualQ,
                "AuthGuard",
                "function",
                "fn check()"
            ),
            "passage: AuthGuard function fn check()"
        );
    }

    #[test]
    fn gemma_asymmetric_prefixes() {
        assert!(format_query_for_model(EmbeddingModelId::Gemma300mQ4, "x").contains("query:"));
        assert!(
            format_document_for_model(EmbeddingModelId::Gemma300mQ4, "T", "k", "")
                .starts_with("title:")
        );
    }
}
