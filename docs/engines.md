# Engines: retrieval presets

NeuroMesh v0.9.0 uses one **retrieval engine** preset: `fast` | `hybrid` | `deep`.

## Default (`engine: fast`)

| Layer | Default |
| :--- | :--- |
| **Graph** | `native` — `neuromesh index` builds AST graph only |
| **Retrieval** | query-side lexical expansion + graph hops |
| **Embeddings** | off at index and MCP startup |
| **Agent** | pass the prompt only — no client keywords |

```bash
neuromesh index
neuromesh config engine get
```

## When to opt in

| Preset | Choose when |
| :--- | :--- |
| **`hybrid`** | Multilingual NL, obfuscated symbol names, need embedding-primary recall |
| **`deep`** | Large refactors, max recall — **full symbol embed** at rebuild (no file tier) |

```bash
neuromesh config engine hybrid
neuromesh index --mode hybrid
neuromesh embed rebuild
```

## Graph backend

Separate from retrieval engine — controls whether **`get_context_packet`** uses native graph or an optional CBM proxy. Default **`native`**. See [graph-proxy.md](graph-proxy.md).

---

**Full reference** (sidecar v6, env vars, modes, migration from v0.8): [configuration.md](configuration.md).
