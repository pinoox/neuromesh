use crate::model_install::{default_models_root, install_hint, MINILM_MULTILINGUAL_Q};
use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use neuromesh_core::EmbeddingModelId;
use std::path::{Path, PathBuf};

const ONNX_NAME: &str = "model_optimized.onnx";

#[derive(Debug)]
pub enum BundledModelError {
    Missing(String),
    Io(std::io::Error),
    Init(String),
}

impl std::fmt::Display for BundledModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(msg) => write!(f, "bundled model missing: {msg}"),
            Self::Io(e) => write!(f, "bundled model io: {e}"),
            Self::Init(msg) => write!(f, "bundled model init: {msg}"),
        }
    }
}

impl std::error::Error for BundledModelError {}

pub fn bundled_model_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(dir) = std::env::var("NEUROMESH_MODEL_DIR") {
        paths.push(PathBuf::from(dir).join(MINILM_MULTILINGUAL_Q.dir_name));
    }
    paths.push(default_models_root().join(MINILM_MULTILINGUAL_Q.dir_name));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            paths.push(parent.join("models").join(MINILM_MULTILINGUAL_Q.dir_name));
        }
    }
    if let Some(dev) = option_env!("NEUROMESH_DEV_MODEL_DIR") {
        paths.push(PathBuf::from(dev));
    }
    paths
}

pub fn resolve_bundled_minilm_dir() -> Option<PathBuf> {
    bundled_model_search_paths()
        .into_iter()
        .find(|dir| dir.join(ONNX_NAME).is_file() && dir.join("tokenizer.json").is_file())
}

pub fn bundled_minilm_available() -> bool {
    resolve_bundled_minilm_dir().is_some()
}

fn read_required(dir: &Path, name: &str) -> Result<Vec<u8>, BundledModelError> {
    std::fs::read(dir.join(name)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BundledModelError::Missing(format!("{}/{}", dir.display(), name))
        } else {
            BundledModelError::Io(e)
        }
    })
}

pub fn try_load_bundled_minilm(
    model: EmbeddingModelId,
    intra_threads: Option<usize>,
) -> Result<TextEmbedding, BundledModelError> {
    if model != EmbeddingModelId::MiniLmMultilingualQ {
        return Err(BundledModelError::Missing(format!(
            "no bundled weights for {}",
            model.as_str()
        )));
    }
    let dir = resolve_bundled_minilm_dir().ok_or_else(|| {
        BundledModelError::Missing(format!(
            "MiniLM not installed ({}). Expected: {}",
            install_hint(),
            default_models_root()
                .join(MINILM_MULTILINGUAL_Q.dir_name)
                .display()
        ))
    })?;

    let user_model = UserDefinedEmbeddingModel::new(
        read_required(&dir, ONNX_NAME)?,
        TokenizerFiles {
            tokenizer_file: read_required(&dir, "tokenizer.json")?,
            config_file: read_required(&dir, "config.json")?,
            special_tokens_map_file: read_required(&dir, "special_tokens_map.json")?,
            tokenizer_config_file: read_required(&dir, "tokenizer_config.json")?,
        },
    )
    .with_pooling(Pooling::Mean)
    .with_quantization(QuantizationMode::Static);

    let mut opts = InitOptionsUserDefined::default();
    if let Some(n) = intra_threads {
        opts = opts.with_intra_threads(n);
    }

    TextEmbedding::try_new_from_user_defined(user_model, opts)
        .map_err(|e| BundledModelError::Init(e.to_string()))
}
