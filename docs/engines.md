# Engines: graph backend & seed resolution

NeuroMesh separates **where structure comes from** (graph backend) from **how tasks pick starting symbols** (seed resolution). Both are configurable from CLI, monitor Settings, or `nm.config.json`.

## v0.8.6 default (recommended)

| Layer | Default | What you do |
| :--- | :--- | :--- |
| **Graph** | `native` | Nothing — built-in AST index |
| **Embeddings** | **ON** + **bundled MiniLM** | Nothing — prompt-only; weights in release tarball |
| **Seed assist** | Embed-primary | **Do not** pass client `keywords`/`expansion` |

Install → `neuromesh index` → `neuromesh embed rebuild` → `get_context_packet(prompt)`. Graph index is fast; embedding sidecar builds once (incremental on later edits).

```bash
neuromesh doctor --embed              # sidecar + bundled model path
neuromesh doctor --embed --bench      # p50/p95 warm embed latency
neuromesh embed rebuild               # build/refresh embeddings.bin after graph index
neuromesh embed prefetch              # warm bundled weights (install does this)
```

---

## Local embedding engine (MiniLM)

Multilingual vector seed resolution for natural-language prompts.

| Item | Value |
| :--- | :--- |
| **Model** | `minilm_multilingual_q` — Paraphrase MiniLM L12 v2 Q |
| **Dimensions** | 384 (matryoshka) |
| **Weights** | **Bundled** in release (`models/minilm-multilingual-q/`); dev: `scripts/fetch-minilm-model.sh` |
| **Runtime** | fastembed `UserDefinedEmbeddingModel` — no HuggingFace download when bundled |

Query path: embed prompt once → ANN on `embeddings.bin` → graph-resolve → fold packet.

### Supplementary features

| Feature | Config | Default |
| :--- | :--- | :--- |
| Semantic prompt cache | `semantic_cache_*` | on, 16 entries, 0.96 cosine |
| Optional-file dedup | `optional_dedup_min_cosine` | off (`max_quality` uses 0.93) |
| Module centroids | `module_cluster_enabled` | off at index (`max_quality` applies bonus when present) |
| Graph-first index | `index_on_build` | **false** — run `neuromesh embed rebuild` |
| Two-stage ANN | `two_stage_enabled` / `coarse_pool_max` | on / 400 (union coarse pool, full-scan fallback) |
| Sidecar format | v5 Int8 | 4× smaller than v4 f32 |
| L3 recovery floor | `recovery_min_cosine` | 0.38 (primary `min_cosine` 0.45) |

Env: `NEUROMESH_EMBEDDINGS=0` disables vectors; `NEUROMESH_EMBED_THREADS=2`; `NEUROMESH_EMBED_INDEX_BATCH=128`; `NEUROMESH_SEMANTIC_CACHE=0`.

---

## Graph backend

| Value | Meaning |
| :--- | :--- |
| `native` | Built-in AST index (**default**) |
| `auto` | CBM from IDE MCP when found; else native |
| `proxy_cbm` | Always spawn codebase-memory-mcp for `get_context_packet` |

```bash
neuromesh config graph-backend native      # recommended
neuromesh config graph-backend auto      # CBM when installed
neuromesh doctor --proxy --probe
```

Only **`get_context_packet`** uses an external graph when proxy is active. Trace, fold, and search stay native.

---

## Custom seed engines (opt-in)

Use these only when you explicitly want lexical keyword assist instead of (or in addition to) embed-primary:

| Value | Role |
| :--- | :--- |
| *(default)* | **Embed-primary** — bundled MiniLM, prompt only |
| `keywords` | Literal keyword match |
| `keywords_expanded` | Expanded keywords + aliases + `auto_extract_keywords` |
| `hybrid` | **MiniLM embed + lexical** (Arabic-heavy opt-in) |
| `off` | Disable seed resolution |

### When to pass client keywords

| Setup | Pass `keywords`/`expansion`? |
| :--- | :--- |
| **Default (embed ON, no engine override)** | **No** |
| `keywords` / `keywords_expanded` / `hybrid` | **Yes** (or enable `auto_extract_keywords`) |

```bash
neuromesh config seed-engine hybrid          # opt-in: embed + lexical
neuromesh config seed-engine keywords_expanded # legacy lexical mode
neuromesh config seed-engine get             # show effective layers
```

Environment: `NEUROMESH_SEED_ENGINE=hybrid` · `NEUROMESH_AUTO_EXTRACT_KEYWORDS=1` (lexical engines only)

Monitor **Settings → Seed Resolution Engine** saves `nm.config.json`.

### Example: lexical override

```json
{
  "embeddings": { "enabled": true },
  "seed_resolution": {
    "engine": "keywords_expanded",
    "auto_extract_keywords": true
  }
}
```

---

## Layering

Effective config merges: global `~/.neuromesh/config.json` → project `nm.config.json` → env → MCP per-call overrides.

See [graph-proxy.md](graph-proxy.md) · [cli.md](cli.md) · [agent-guide.md](agent-guide.md).
