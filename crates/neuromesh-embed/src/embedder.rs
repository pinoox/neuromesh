use crate::bundled_model::{bundled_minilm_available, try_load_bundled_minilm};
use crate::model_install::install_hint;
use crate::search::truncate_and_normalize;
use fastembed::TextEmbedding;
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

pub fn format_document_gemma(
    title: &str,
    kind: &str,
    signature: &str,
    doc: Option<&str>,
) -> String {
    let sig = signature.trim();
    let doc_part = doc
        .filter(|d| !d.trim().is_empty())
        .map(|d| format!(" - {}", d.trim()))
        .unwrap_or_default();
    if sig.is_empty() {
        format!("title: {title} | text: {kind}{doc_part}")
    } else {
        format!("title: {title} | text: {kind} {sig}{doc_part}")
    }
}

pub fn format_document_minilm(
    title: &str,
    kind: &str,
    signature: &str,
    doc: Option<&str>,
) -> String {
    let sig = signature.trim();
    let doc_part = doc
        .filter(|d| !d.trim().is_empty())
        .map(|d| format!(" - {}", d.trim()))
        .unwrap_or_default();
    if sig.is_empty() {
        format!("passage: {title} {kind}{doc_part}")
    } else {
        format!("passage: {title} {kind} {sig}{doc_part}")
    }
}

pub fn format_document(title: &str, kind: &str, signature: &str) -> String {
    format_document_gemma(title, kind, signature, None)
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
    doc: Option<&str>,
) -> String {
    match model {
        EmbeddingModelId::Gemma300mQ4 => format_document_gemma(title, kind, signature, doc),
        EmbeddingModelId::MiniLmMultilingualQ => {
            format_document_minilm(title, kind, signature, doc)
        }
    }
}

fn try_init_text_embedding(
    config: &EmbeddingConfig,
    _show_download_progress: bool,
) -> Result<TextEmbedding, EmbedderError> {
    match config.model {
        EmbeddingModelId::MiniLmMultilingualQ => {
            if !bundled_minilm_available() {
                return Err(EmbedderError::Init(format!(
                    "MiniLM not installed. {}",
                    install_hint()
                )));
            }
            try_load_bundled_minilm(config.model, config.intra_threads)
                .map_err(|e| EmbedderError::Init(format!("{e}. {}", install_hint())))
        }
        EmbeddingModelId::Gemma300mQ4 => Err(EmbedderError::Init(format!(
            "gemma300m_q4 is not installable yet; use MiniLM ({}). {}",
            EmbeddingModelId::MiniLmMultilingualQ.as_str(),
            install_hint()
        ))),
    }
}

pub struct Embedder {
    model: TextEmbedding,
    config: EmbeddingConfig,
}

impl Embedder {
    pub fn try_new(config: EmbeddingConfig) -> Result<Self, EmbedderError> {
        Self::try_new_with_options(config, false)
    }

    fn try_new_with_options(
        config: EmbeddingConfig,
        show_download_progress: bool,
    ) -> Result<Self, EmbedderError> {
        let model = try_init_text_embedding(&config, show_download_progress)?;
        Ok(Self { model, config })
    }

    pub fn is_global_loaded() -> bool {
        GLOBAL.get().is_some()
    }

    pub fn lazy_global(config: EmbeddingConfig) -> Result<Arc<Mutex<Self>>, EmbedderError> {
        Self::lazy_global_with_progress(config, false)
    }

    pub fn lazy_global_with_progress(
        config: EmbeddingConfig,
        show_download_progress: bool,
    ) -> Result<Arc<Mutex<Self>>, EmbedderError> {
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
        let embedder = Arc::new(Mutex::new(Self::try_new_with_options(
            config,
            show_download_progress,
        )?));
        tracing::warn!(
            "ONNX embedder loaded (~600 MB retained until MCP restart); matryoshka does not reduce session RAM"
        );
        let _ = GLOBAL.set(embedder.clone());
        Ok(embedder)
    }

    /// Warm installed MiniLM weights (requires `neuromesh install embed minilm`).
    pub fn prefetch_model(
        config: EmbeddingConfig,
        show_download_progress: bool,
    ) -> Result<(), EmbedderError> {
        Self::warm_with_progress(config, show_download_progress)
    }

    /// Load singleton and run one dummy query (cold-start amortization).
    pub fn warm(config: EmbeddingConfig) -> Result<(), EmbedderError> {
        Self::warm_with_progress(config, false)
    }

    pub fn warm_with_progress(
        config: EmbeddingConfig,
        show_download_progress: bool,
    ) -> Result<(), EmbedderError> {
        if !config.enabled {
            return Ok(());
        }
        let arc = Self::lazy_global_with_progress(config.clone(), show_download_progress)?;
        let mut embedder = arc.lock();
        let _ = embedder.embed_query("neuromesh warmup")?;
        drop(embedder);
        crate::intent_prototypes::warm_intent_prototypes(&config)?;
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
                "fn check()",
                None,
            ),
            "passage: AuthGuard function fn check()"
        );
    }

    #[test]
    fn minilm_document_with_doc() {
        assert!(format_document_for_model(
            EmbeddingModelId::MiniLmMultilingualQ,
            "AuthGuard",
            "function",
            "fn check()",
            Some("Validates JWT"),
        )
        .contains("Validates JWT"));
    }

    #[test]
    fn gemma_asymmetric_prefixes() {
        assert!(format_query_for_model(EmbeddingModelId::Gemma300mQ4, "x").contains("query:"));
        assert!(
            format_document_for_model(EmbeddingModelId::Gemma300mQ4, "T", "k", "", None)
                .starts_with("title:")
        );
    }
}
