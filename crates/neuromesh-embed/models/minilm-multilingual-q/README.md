# MiniLM multilingual Q (on-demand)

ONNX weights for `ParaphraseMLMiniLML12V2Q` — loaded via fastembed `UserDefinedEmbeddingModel`.

## Install (users)

Release binaries do **not** ship model weights. Install once:

```bash
neuromesh install embed minilm
# or when switching engine:
neuromesh config engine hybrid --install
```

Weights are stored under:

- Linux/macOS: `~/.local/share/neuromesh/models/minilm-multilingual-q/`
- Windows: `%LOCALAPPDATA%\neuromesh\models\minilm-multilingual-q\`

Legacy releases may still have `{exe}/models/minilm-multilingual-q/` next to the binary.

## Dev-only fetch (not in git)

```bash
./scripts/fetch-minilm-model.sh   # Linux / macOS
./scripts/fetch-minilm-model.ps1  # Windows
```

Required files:

- `model_optimized.onnx`
- `tokenizer.json`
- `config.json`
- `special_tokens_map.json`
- `tokenizer_config.json`

Source: [Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q](https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q)

## Runtime search order

1. `$NEUROMESH_MODEL_DIR/minilm-multilingual-q`
2. `~/.local/share/neuromesh/models/minilm-multilingual-q` (or `%LOCALAPPDATA%\neuromesh\models\…`)
3. `{exe}/models/minilm-multilingual-q` (legacy release layout)
4. Crate path after dev fetch (`NEUROMESH_DEV_MODEL_DIR` from `build.rs`)

No HuggingFace auto-download at runtime — run `neuromesh install embed minilm` first.
