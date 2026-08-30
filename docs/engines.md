# Engines: graph backend & seed resolution

NeuroMesh separates **where structure comes from** (graph backend) from **how tasks pick starting symbols** (seed engine). Both are configurable from CLI, monitor Settings, or `nm.config.json`.

**v0.8.5 recommendation:** keep **`native`** graph backend and **`semantic_lite`** seed engine (local embeddings on by default). Pass client `keywords`/`expansion` only when you switch to `keywords_expanded`, `keywords`, or `hybrid`. MCP stdio always uses **native** unless you set `proxy_cbm` explicitly.

## Graph backend

| Value | Meaning |
| :--- | :--- |
| `native` | Built-in AST index + `NeuralProjectGraph` (**default**) |
| `auto` | Use CBM/Graphify from IDE MCP configs when found; else native |
| `proxy_cbm` | Always spawn **codebase-memory-mcp** for `get_context_packet` |
| `proxy_graphify` | Graphify adapter (stub — use CBM today) |

### CLI

```bash
neuromesh config graph-backend              # show effective backend
neuromesh config graph-backend auto         # project nm.config.json
neuromesh config graph-backend proxy_cbm -g # global ~/.neuromesh/config.json
neuromesh doctor --proxy                    # list detected MCP servers
neuromesh doctor --proxy --probe            # live CBM connect + sample packet
```

Environment: `NEUROMESH_GRAPH_BACKEND=auto`

### Monitor

**Settings → Graph Backend** — Native / Auto / CBM Proxy. **Save Changes** writes `nm.config.json` and reconnects the proxy without restarting monitor.

**Probe CBM connection** calls `/api/graph-proxy/probe` (same as `doctor --probe`).

### CBM project matching

When using CBM, NeuroMesh calls `list_projects` and matches your workspace directory to CBM's `root_path`. Example: `C:/projects/neuromesh` → project id `neuromesh-repo`.

If the repo is not indexed in CBM yet, run `index_repository` via CBM (or Cursor's codebase-memory MCP) first.

### Runtime behavior

- Only **`get_context_packet`** uses the external graph when a proxy is active.
- v0.8.2 forwards assisted signals to CBM and returns honest proxy `retrieval` metadata.
- `search_symbols`, `trace`, `expand_fold`, galaxy UI, etc. still use the **native** graph.
- With `fallback_native: true` (default), proxy failures fall back to native tiered activation.
- Expect **~6–8× higher latency** on proxy vs native (CBM stdio round-trips per packet).

See [graph-proxy.md](graph-proxy.md) for architecture details.

---

## Local embedding engine (default)

Multilingual vector seed resolution for natural-language prompts. **On by default** — the agent sends the task as written; the engine embeds the prompt once (singleton ONNX session + per-packet cache) and ANN-searches symbol sketches on the native graph.

### Profiles

| Profile | Config | When |
| :--- | :--- | :--- |
| **Interactive (default)** | `minilm_multilingual_q`, singleton warm, `intra_threads: 4` | MCP / IDE daily use |
| **Quality (offline)** | `gemma300m_q4`, re-index overnight | Higher NL quality; not for interactive hot path |
| **Lexical legacy** | `keywords_expanded` + `embeddings.enabled: false` | Match v0.8.3 behavior |

| Model | Role | Size (approx) |
| :--- | :--- | :--- |
| `minilm_multilingual_q` | **Default** — Paraphrase MiniLM L12 v2 Q, multilingual | ~50–80 MB download |
| `gemma300m_q4` | Quality tier — EmbeddingGemma-300M Q4 | ~150–200 MB download |

```bash
neuromesh config embeddings off             # disable vector path
neuromesh config embeddings gemma300m_q4    # quality tier (re-index required)
neuromesh doctor --embed                    # sidecar + cold warm latency
neuromesh doctor --embed --bench            # p50/p95 warm embed (20 queries)
neuromesh index                             # writes embeddings.bin when enabled
```

Index writes `embeddings.bin` beside `graph.bin` (**sidecar v3**). **Re-index after upgrade** — v3 adds directory **module centroids** (v2 added doc-enriched sketches). **Switching models also requires `neuromesh index`** (sidecar stores `model_id`).

Query path: embed prompt once per packet → ANN pool (`ann_top_k: 16`) → insert top **`embed_seed_cap: 4`** seeds → **graph-resolve** hits (no reranker, no raw file dump). Lexical keyword fallback runs only when embeddings are disabled or the sidecar is missing.

### Supplementary MiniLM features (zero extra models)

| Feature | Config | Default | Effect |
| :--- | :--- | :--- | :--- |
| **Semantic prompt cache** | `semantic_cache_enabled`, `semantic_cache_entries`, `semantic_cache_min_cosine` | on, 16, 0.96 | Near-duplicate MCP prompts skip full activation; `retrieval.cache_hit: true` |
| **Optional-file dedup** | `optional_dedup_min_cosine` (`None` = off) | 0.93 | Drop redundant optional files by sidecar cosine; test/mock paths exempt |
| **Module centroids** | `module_cluster_enabled` | on | Index-time directory clusters; small optional routing bonus |
| **Embed intent (General)** | `embed_intent_for_general` | off | Refine rule-based `General` intent via prototype embeddings |

**Metadata:** `retrieval.resolution_tier` (`embedding_primary`, `L1_exact`, `L2_pattern`, `L3_semantic_recovery`), `retrieval.max_embedding_score`, `retrieval.cache_hit`, `coverage: no_confident_match` when nothing clears `min_cosine`.

Env: `NEUROMESH_EMBEDDINGS=0` to disable, `NEUROMESH_EMBED_MODEL=minilm_multilingual_q|gemma300m_q4`, `NEUROMESH_EMBED_THREADS=4` (ONNX intra-op threads; useful on Intel hybrid CPUs), `NEUROMESH_SEMANTIC_CACHE=0`, `NEUROMESH_OPTIONAL_DEDUP=0.93|off`.

---

## Seed resolution engine

| Value | Role |
| :--- | :--- |
| `semantic_lite` | **Default** — embedding-primary + graph resolve |
| `off` | Disable seed resolution |
| `keywords` | Literal keyword match (client/server keywords recommended) |
| `keywords_expanded` | Expanded keywords + aliases |
| `hybrid` | Keywords + semantic-lite passes |

### When to pass keywords

| Engine | Agent should pass `keywords`/`expansion`? |
| :--- | :--- |
| `semantic_lite` (default) | **No** — prompt only |
| `keywords` / `keywords_expanded` / `hybrid` | **Yes** (or enable `auto_extract_keywords`) |

### CLI

```bash
neuromesh config seed-engine                 # show layers (global / project / env)
neuromesh config seed-engine hybrid          # nm.config.json in cwd
neuromesh config seed-engine hybrid --global # ~/.neuromesh/config.json
```

Environment: `NEUROMESH_SEED_ENGINE=hybrid`

### Monitor

**Settings → Seed Resolution Engine** — Semantic Lite / Keywords+ / Hybrid. Saved to `nm.config.json`; takes effect on the next `get_context` call (no restart).

### Layering

1. `~/.neuromesh/config.json` — machine default
2. `<workspace>/nm.config.json` — project override (commit-friendly)
3. Managed project slot `config.json` when using `neuromesh port` persist
4. `NEUROMESH_SEED_ENGINE` — one-shot env override

---

## Quick recipes

**Lexical / assisted mode (prior v0.8.3 behavior)**

```json
{
  "seed_resolution": {
    "engine": "keywords_expanded",
    "auto_extract_keywords": true
  },
  "embeddings": { "enabled": false }
}
```

**Try CBM without changing defaults globally**

```bash
NEUROMESH_GRAPH_BACKEND=auto neuromesh mcp
```

**Verify install**

```bash
neuromesh doctor --embed
neuromesh doctor --proxy --probe
```

Expected: embedding sidecar fresh after index; CBM probe `Status : connected` when configured.
