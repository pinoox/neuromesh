# Graph proxy backends (CBM / Graphify)

NeuroMesh can delegate **indexing and graph queries** to an external MCP server while keeping **folding, packet assembly, and MCP tools** in NeuroMesh.

Default is **`native`** — unchanged built-in graph (`neuromesh index`, `NeuralProjectGraph`, tiered retrieval). **Native assisted is the recommended path** for precision and latency; proxy is an optional sidecar when CBM is already indexed in your IDE.

## v0.8.2 stabilization

Stock v0.8.1 proxy was broken (0 files on every query). v0.8.2 fixes:

| Fix | What |
| :--- | :--- |
| **B1** | Parse CBM `search_graph` `{cols, rows}` JSON (not only `groups`/`results`) |
| **B2** | Forward MCP `keywords`, `expansion`, and identifiers → CBM `query` + `semantic_query` |
| **B3** | Honest proxy `retrieval` metadata — computed confidence/sufficiency (cap ~0.45), `critical_gaps`, `suggested_keywords` |
| **B4** | Drop phantom Route hits with empty `file` (no `path: unknown` inflation) |

Express test3 benchmark (60 cells, 10 langs × 6 tasks): proxy assisted recall **0.578** vs raw **0.448**; native assisted still wins on precision (**0.789** vs **0.504**) and warm p50 (**~35 ms** vs **~230 ms**).

## Backends

| `graph_backend.backend` | Behavior |
| :--- | :--- |
| `native` | Built-in graph only (default) |
| `auto` | Use CBM or Graphify from IDE MCP configs when found; else native |
| `proxy_cbm` | codebase-memory-mcp via MCP stdio |
| `proxy_graphify` | Graphify (adapter stub — use CBM today) |

## Configure

```bash
neuromesh config graph-backend auto          # project nm.config.json
neuromesh config graph-backend proxy_cbm -g  # global ~/.neuromesh/config.json
neuromesh doctor --proxy                     # list detected MCP servers
```

Environment override: `NEUROMESH_GRAPH_BACKEND=auto`

Example `nm.config.json`:

```json
{
  "graph_backend": {
    "backend": "auto",
    "fallback_native": true
  }
}
```

## Auto-detect

When `backend` is `auto` or `proxy_cbm`, NeuroMesh scans:

- `.cursor/mcp.json`, `~/.cursor/mcp.json`
- `.vscode/mcp.json`, `.mcp.json`
- `.codex/config.toml`

Servers whose command/name matches `cbm`, `codebase-memory`, or `graphify` are candidates. CBM is preferred over Graphify.

## CBM project id

CBM indexes repos under a **project name** (not always the folder name). NeuroMesh calls `list_projects` and matches your workspace path to CBM's `root_path`:

| Workspace | CBM project id |
| :--- | :--- |
| `C:/projects/neuromesh` | `neuromesh-repo` |

Index the repo in CBM first (`index_repository`). Verify with `neuromesh doctor --proxy --probe`.

## Runtime

- **Assisted mode** — MCP `keywords` and `expansion` are forwarded to CBM (`query` + `semantic_query`); assisted ≠ raw on proxy (v0.8.2+).
- **Honest metadata** — Proxy packets use computed confidence/sufficiency (cap ~0.45), not fixed 0.55/0.65.
- **Route filtering** — Empty-file Route hits are dropped (no phantom `unknown` paths).
- **Probe query** — `doctor --probe` uses a realistic middleware sample (`Router middleware app.use next pipeline`).

1. NeuroMesh spawns the external MCP process (stdio); child process stays alive for the session
2. `get_context_packet` builds a `ProxySearchContext` from the task signature and calls `search_graph` + `get_code_snippet` on CBM
3. Results are shaped into an evidence packet (folding on proxy snippets is minimal in v0.9)
4. If proxy fails and `fallback_native: true`, native tiered activation runs unchanged

Other tools (`neuromesh_search_symbols`, `trace`, `expand_fold`) still use the **native** graph unless you stay on `native` backend only.

## Monitor

Galaxy UI **Settings** includes Graph Backend (Native / Auto / CBM) and Seed Engine presets. **Probe CBM connection** hits the same path as `doctor --probe`.

## Links

- [codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp)
- [Graphify](https://github.com/Graphify-Labs/graphify)
