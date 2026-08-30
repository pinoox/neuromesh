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

pub fn format_query(prompt: &str) -> String {
    format!("task: search result | query: {prompt}")
}

pub fn format_document(title: &str, kind: &str, signature: &str) -> String {
    let sig = signature.trim();
    if sig.is_empty() {
        format!("title: {title} | text: {kind}")
    } else {
        format!("title: {title} | text: {kind} {sig}")
    }
}

pub struct Embedder {
    model: TextEmbedding,
    config: EmbeddingConfig,
}

impl Embedder {
    pub fn try_new(config: EmbeddingConfig) -> Result<Self, EmbedderError> {
        let fastembed_model = match config.model {
            EmbeddingModelId::Gemma300mQ4 => EmbeddingModel::EmbeddingGemma300MQ4,
            EmbeddingModelId::MiniLmMultilingualQ => EmbeddingModel::ParaphraseMLMiniLML12V2Q,
        };
        let model = TextEmbedding::try_new(
            InitOptions::new(fastembed_model).with_show_download_progress(false),
        )
        .map_err(|e| EmbedderError::Init(e.to_string()))?;
        Ok(Self { model, config })
    }

    pub fn lazy_global(config: EmbeddingConfig) -> Result<Arc<Mutex<Self>>, EmbedderError> {
        if let Some(existing) = GLOBAL.get() {
            return Ok(existing.clone());
        }
        let embedder = Arc::new(Mutex::new(Self::try_new(config)?));
        let _ = GLOBAL.set(embedder.clone());
        Ok(embedder)
    }

    pub fn config(&self) -> &EmbeddingConfig {
        &self.config
    }

    pub fn embed_query(&mut self, prompt: &str) -> Result<Vec<f32>, EmbedderError> {
        let text = if self.config.model == EmbeddingModelId::Gemma300mQ4 {
            format_query(prompt)
        } else {
            prompt.to_string()
        };
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
