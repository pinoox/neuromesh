use std::io;
use std::path::{Path, PathBuf};

const MINILM_DIR: &str = "minilm-multilingual-q";
const ONNX_NAME: &str = "model_optimized.onnx";
const TOKENIZER_NAME: &str = "tokenizer.json";
const HF_BASE: &str =
    "https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q/resolve/main";

/// Catalog entry for an on-demand embedding model (extensible in Phase 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbedModelSpec {
    pub id: &'static str,
    pub dir_name: &'static str,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub hf_base: &'static str,
    pub files: &'static [&'static str],
}

pub const MINILM_MULTILINGUAL_Q: EmbedModelSpec = EmbedModelSpec {
    id: "minilm-multilingual-q",
    dir_name: MINILM_DIR,
    label: "Paraphrase MiniLM multilingual Q (384-dim, recommended)",
    aliases: &["minilm", "mini-lm", "mini_lm", "minilm-q"],
    hf_base: HF_BASE,
    files: &[
        ONNX_NAME,
        TOKENIZER_NAME,
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ],
};

pub static CATALOG: &[EmbedModelSpec] = &[MINILM_MULTILINGUAL_Q];

#[derive(Debug, Clone, Copy, Default)]
pub struct InstallOptions {
    pub quiet: bool,
    pub force: bool,
}

#[derive(Debug)]
pub enum ModelInstallError {
    UnknownId(String),
    Io(io::Error),
    Download(String),
}

impl std::fmt::Display for ModelInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "unknown embed model: {id}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Download(msg) => write!(f, "download failed: {msg}"),
        }
    }
}

impl std::error::Error for ModelInstallError {}

impl From<io::Error> for ModelInstallError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn parse_model_id(raw: &str) -> Option<&'static EmbedModelSpec> {
    let key = raw.trim().to_lowercase();
    CATALOG.iter().find(|spec| {
        spec.id.eq_ignore_ascii_case(&key)
            || spec.aliases.iter().any(|a| a.eq_ignore_ascii_case(&key))
    })
}

pub fn default_models_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neuromesh")
        .join("models")
}

pub fn model_install_dir(spec: &EmbedModelSpec) -> PathBuf {
    default_models_root().join(spec.dir_name)
}

pub fn is_model_installed(spec: &EmbedModelSpec) -> bool {
    model_dir_ready(&model_install_dir(spec))
}

fn model_dir_ready(dir: &Path) -> bool {
    dir.join(ONNX_NAME).is_file() && dir.join(TOKENIZER_NAME).is_file()
}

pub fn list_installed() -> Vec<(EmbedModelSpec, PathBuf)> {
    CATALOG
        .iter()
        .filter_map(|spec| {
            let dir = model_install_dir(spec);
            if model_dir_ready(&dir) {
                Some((*spec, dir))
            } else {
                None
            }
        })
        .collect()
}

pub fn install_hint() -> &'static str {
    "Run: neuromesh install embed minilm"
}

pub fn install_hint_with_flag(engine: &str) -> String {
    format!(
        "{engine} requires the MiniLM embedding model.\n\
         {hint}\n\
         Or:  neuromesh config engine {engine} --yes",
        hint = install_hint()
    )
}

#[cfg(feature = "download")]
pub fn install_model(
    spec: &EmbedModelSpec,
    opts: InstallOptions,
) -> Result<PathBuf, ModelInstallError> {
    install_model_inner(spec, opts)
}

#[cfg(not(feature = "download"))]
pub fn install_model(
    _spec: &EmbedModelSpec,
    _opts: InstallOptions,
) -> Result<PathBuf, ModelInstallError> {
    Err(ModelInstallError::Download(
        "this binary was built without embed download support; rebuild with --features embeddings"
            .into(),
    ))
}

#[cfg(feature = "download")]
fn install_model_inner(
    spec: &EmbedModelSpec,
    opts: InstallOptions,
) -> Result<PathBuf, ModelInstallError> {
    let dest = model_install_dir(spec);
    std::fs::create_dir_all(&dest)?;

    if !opts.force && model_dir_ready(&dest) {
        if !opts.quiet {
            eprintln!("MiniLM already installed at {}", dest.display());
        }
        return Ok(dest);
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent("neuromesh-embed-install/1.0")
        .build()
        .map_err(|e| ModelInstallError::Download(e.to_string()))?;

    for name in spec.files {
        let out = dest.join(name);
        if !opts.force && out.is_file() {
            if !opts.quiet {
                eprintln!("  skip {name} (exists)");
            }
            continue;
        }
        let url = format!("{}/{}", spec.hf_base, name);
        if !opts.quiet {
            eprintln!("  fetch {name}…");
        }
        let response = client
            .get(&url)
            .send()
            .map_err(|e| ModelInstallError::Download(format!("{name}: {e}")))?;
        if !response.status().is_success() {
            return Err(ModelInstallError::Download(format!(
                "{name}: HTTP {}",
                response.status()
            )));
        }
        let bytes = response
            .bytes()
            .map_err(|e| ModelInstallError::Download(format!("{name}: {e}")))?;
        let tmp = dest.join(format!(".{name}.download"));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &out)?;
    }

    if !model_dir_ready(&dest) {
        return Err(ModelInstallError::Download(
            "install incomplete after download".into(),
        ));
    }

    if !opts.quiet {
        eprintln!("MiniLM installed at {}", dest.display());
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minilm_aliases() {
        assert_eq!(parse_model_id("minilm").map(|s| s.id), Some(MINILM_DIR));
        assert_eq!(parse_model_id("mini-lm").map(|s| s.id), Some(MINILM_DIR));
        assert_eq!(
            parse_model_id("minilm-multilingual-q").map(|s| s.id),
            Some(MINILM_DIR)
        );
        assert!(parse_model_id("unknown").is_none());
    }
}
