#!/usr/bin/env bash
# Optional recovery: download MiniLM ONNX + tokenizer when missing from repo checkout (~50–80 MB).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/crates/neuromesh-embed/models/minilm-multilingual-q"
BASE="https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q/resolve/main"

mkdir -p "${DEST}"

files=(
  model_optimized.onnx
  tokenizer.json
  config.json
  special_tokens_map.json
  tokenizer_config.json
)

for f in "${files[@]}"; do
  out="${DEST}/${f}"
  if [[ -f "${out}" ]]; then
    echo "  skip ${f} (exists)"
    continue
  fi
  echo "  fetch ${f}…"
  curl -fSL --progress-bar "${BASE}/${f}" -o "${out}"
done

echo "MiniLM bundled at ${DEST}"
