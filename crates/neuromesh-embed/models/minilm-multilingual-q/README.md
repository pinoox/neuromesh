# MiniLM multilingual Q (bundled)

ONNX weights for `ParaphraseMLMiniLML12V2Q` — loaded via fastembed `UserDefinedEmbeddingModel` (no HuggingFace download at runtime when present).

## Fetch once (dev / CI)

```bash
# Linux / macOS
./scripts/fetch-minilm-model.sh

# Windows
./scripts/fetch-minilm-model.ps1
```

Required files in this directory:

- `model_optimized.onnx`
- `tokenizer.json`
- `config.json`
- `special_tokens_map.json`
- `tokenizer_config.json`

Source: [Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q](https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q)

## Runtime search order

1. `$NEUROMESH_MODEL_DIR/minilm-multilingual-q`
2. `{exe}/models/minilm-multilingual-q` (release tarball layout)
3. `~/.local/share/neuromesh/models/minilm-multilingual-q`
4. `%LOCALAPPDATA%/neuromesh/models/minilm-multilingual-q`
5. This crate path (source builds after fetch)

If none match, fastembed falls back to HuggingFace cache download.
