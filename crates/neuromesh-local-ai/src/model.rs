use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationType {
    Q4KM,
    Q5KM,
    Q80,
    F16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelDescriptor {
    pub name: String,
    pub parameter_size: String, // "0.6B", "1.5B", "3B", "4B"
    pub quantization: QuantizationType,
    pub ram_required_mb: usize,
    pub model_path: Option<PathBuf>,
    pub loaded: bool,
}

impl LocalModelDescriptor {
    pub fn qwen_0_6b() -> Self {
        Self {
            name: "Qwen2.5-Coder-0.5B-Instruct-GGUF".into(),
            parameter_size: "0.6B".into(),
            quantization: QuantizationType::Q4KM,
            ram_required_mb: 512,
            model_path: None,
            loaded: false,
        }
    }

    pub fn qwen_1_5b() -> Self {
        Self {
            name: "Qwen2.5-Coder-1.5B-Instruct-GGUF".into(),
            parameter_size: "1.5B".into(),
            quantization: QuantizationType::Q4KM,
            ram_required_mb: 1200,
            model_path: None,
            loaded: false,
        }
    }

    pub fn llama_3b() -> Self {
        Self {
            name: "Llama-3.2-3B-Instruct-GGUF".into(),
            parameter_size: "3B".into(),
            quantization: QuantizationType::Q4KM,
            ram_required_mb: 2400,
            model_path: None,
            loaded: false,
        }
    }
}
