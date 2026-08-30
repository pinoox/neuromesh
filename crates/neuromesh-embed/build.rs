use std::path::Path;

fn main() {
    let model_dir = Path::new("models/minilm-multilingual-q");
    let onnx = model_dir.join("model_optimized.onnx");
    if onnx.is_file() {
        if let Ok(abs) = std::fs::canonicalize(model_dir) {
            println!("cargo:rustc-env=NEUROMESH_DEV_MODEL_DIR={}", abs.display());
        }
        println!("cargo:rerun-if-changed={}", onnx.display());
    }
    for name in [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ] {
        let p = model_dir.join(name);
        if p.is_file() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}
