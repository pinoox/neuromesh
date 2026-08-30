# Engines: retrieval presets & graph backend

NeuroMesh v0.9.0 uses one **retrieval engine** preset instead of scattered embedding/seed flags.

## v0.9.0 default (`engine: fast`)

| Layer | Default | What you do |
| :--- | :--- | :--- |
| **Graph** | `native` | `neuromesh index` — AST graph only (<30s typical) |
| **Retrieval** | **`fast`** | Prompt only — query-side lexical expansion + graph hops |
| **Embeddings** | **off** at index and MCP | No ONNX until you opt in |
| **RAM** | **<80 MB** MCP typical | Zero-embed runtime |

```bash
neuromesh index                         # fast (graph only)
neuromesh config engine hybrid          # opt-in MiniLM sidecar
neuromesh index --mode hybrid           # graph + embed rebuild
neuromesh embed rebuild                 # refresh sidecar after hybrid switch
neuromesh doctor --engine               # preset + ONNX skip status
neuromesh eval --release-gates --engine fast
```

### Retrieval engine presets

| `engine` | Index | Query | RAM (typical) | Use when |
| :--- | :--- | :--- | :--- | :--- |
| **`fast`** (default) | graph + lexical | server-assisted keywords + graph | <80 MB | Most repos; instant index |
| **`hybrid`** | graph + incremental sidecar | MiniLM ANN + graph (Phase A Int8/two-stage) | ~250 MB | Obfuscated naming, multilingual NL |
| **`deep`** | graph + full sidecar | hybrid + dedup + module centroids + L3 embed | ~450 MB | Large refactors, max recall |

```json
{
  "retrieval": { "engine": "fast" }
}
```

Env: `NEUROMESH_ENGINE=fast|hybrid|deep`

Legacy granular flags (`two_stage_enabled`, `optional_dedup_min_cosine`, `module_cluster_enabled`, `intra_threads`) are **derived from the preset**. Prefer `neuromesh config engine <preset>`.

---

## Hybrid / deep: MiniLM sidecar

Multilingual vector seed resolution when `engine` is `hybrid` or `deep`.

| Item | Value |
| :--- | :--- |
| **Model** | `minilm_multilingual_q` — Paraphrase MiniLM L12 v2 Q |
| **Dimensions** | 384 (matryoshka) |
| **Weights** | Bundled in release (`models/minilm-multilingual-q/`) |
| **Sidecar** | **v6 hierarchical** — tier-0 file vectors + lazy tier-1 symbols (`embeddings.bin`) |

### Hierarchical index (v6, hybrid/deep)

Cold `neuromesh embed rebuild` embeds **one passage per file** (~250 MiniLM passes) instead of every symbol (~8000). Symbol vectors are **lazy**: first query that hits a file batch-embeds up to 64 symbols and persists incrementally.

Query flow: **file ANN** (top 4) → **lazy symbol embed** → **symbol subset ANN** + coarse lexical pool → full-ANN fallback. Physarum bridging is unchanged.

Safety: concurrent MCP queries serialize sidecar writes via a per-workspace lock; `embeddings.bin` is replaced atomically (temp + rename).

File passages preserve full docstrings (≤480 chars) and complete signatures (≤16 lines, no mid-signature chop). Rebuild required when upgrading from sidecar v4/v5.

Query path (hybrid/deep): embed prompt once → hierarchical ANN → graph-resolve → fold packet.

| Feature | `hybrid` | `deep` |
| :--- | :--- | :--- |
| Two-stage ANN | on | on |
| Optional-file dedup | off | on (0.93) |
| Module centroids | off | on |
| Optimization mode | balanced | max_quality |

---

## Graph backend

| Value | Meaning |
| :--- | :--- |
| `native` | Built-in AST index (**default**) |
| `auto` | CBM from IDE MCP when found; else native |
| `proxy_cbm` | Always spawn codebase-memory-mcp for `get_context_packet` |

```bash
neuromesh config graph-backend native
neuromesh doctor --proxy --probe
```

---

## Layering

Effective config merges: global `~/.neuromesh/config.json` → project `nm.config.json` → env (`NEUROMESH_ENGINE`) → MCP per-call overrides.

See [graph-proxy.md](graph-proxy.md) · [cli.md](cli.md) · [agent-guide.md](agent-guide.md) · [quality.md](quality.md).
