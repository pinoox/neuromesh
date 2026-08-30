# Engines: graph backend & seed resolution

NeuroMesh separates **where structure comes from** (graph backend) from **how tasks pick starting symbols** (seed engine). Both are configurable from CLI, monitor Settings, or `nm.config.json`.

**v0.8.2 recommendation:** keep **`native`** graph backend and **`keywords_expanded`** (or `hybrid`) seed engine. MCP stdio always uses **native + server-side assisted** keywords unless you set `proxy_cbm` explicitly. Use `auto` in monitor only when you want CBM sidecar detection.

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

## Seed engine

| Value | Role |
| :--- | :--- |
| `off` | Disable seed resolution |
| `keywords` | Literal keyword match |
| `keywords_expanded` | Expanded keywords (**default**) |
| `semantic_lite` | Lightweight semantic hints |
| `hybrid` | Keywords + semantic-lite passes |

### CLI

```bash
neuromesh config seed-engine                 # show layers (global / project / env)
neuromesh config seed-engine hybrid          # nm.config.json in cwd
neuromesh config seed-engine hybrid --global # ~/.neuromesh/config.json
```

Environment: `NEUROMESH_SEED_ENGINE=hybrid`

### Monitor

**Settings → Seed Resolution Engine** — Keywords+ / Hybrid / Semantic Lite. Saved to `nm.config.json`; takes effect on the next `get_context` call (no restart).

### Layering

1. `~/.neuromesh/config.json` — machine default
2. `<workspace>/nm.config.json` — project override (commit-friendly)
3. Managed project slot `config.json` when using `neuromesh port` persist
4. `NEUROMESH_SEED_ENGINE` — one-shot env override

---

## Quick recipes

**Try CBM without changing defaults globally**

```bash
NEUROMESH_GRAPH_BACKEND=auto neuromesh mcp
```

**Project uses CBM when available, native otherwise**

```json
{
  "graph_backend": { "backend": "auto", "fallback_native": true },
  "seed_resolution": { "engine": "hybrid" }
}
```

**Verify install**

```bash
neuromesh doctor --proxy --probe
```

Expected: `Status : connected`, CBM tools listed, sample packet (0+ files depending on query and index).
