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

Multilingual vector seed resolution for natural-language prompts. **On by default** — the agent sends the task as written; the engine embeds the prompt and ANN-searches symbol sketches on the native graph.

| Model | Role | Size (approx) |
| :--- | :--- | :--- |
| `gemma300m_q4` | **Default** — EmbeddingGemma-300M Q4, 100+ languages | ~150–200 MB download |
| `minilm_multilingual_q` | Ultra-light fallback | ~80–120 MB |

```bash
neuromesh config embeddings off             # disable vector path
neuromesh config embeddings gemma300m_q4    # pick model
neuromesh doctor --embed                    # sidecar + sample latency
neuromesh index                             # writes embeddings.bin when enabled
```

Index writes `embeddings.bin` beside `graph.bin`. Query path embeds the prompt once, ANN-searches sketches, then **graph-resolves** hits (no raw file dump). Lexical keyword fallback runs only when embeddings are disabled or the sidecar is missing.

**Metadata:** `retrieval.resolution_tier` (`embedding_primary`, `L1_exact`, `L2_pattern`, `L3_semantic_recovery`), `retrieval.max_embedding_score`, `coverage: no_confident_match` when nothing clears `min_cosine`.

Env: `NEUROMESH_EMBEDDINGS=0` to disable, `NEUROMESH_EMBED_MODEL=gemma300m_q4`

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
