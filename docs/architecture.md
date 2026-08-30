# Architecture

NeuroMesh builds a **structural project graph**, then a **task-conditioned packet**. The graph is the nervous system. The packet is the thought.

**v0.8.6 default:** **native graph + bundled MiniLM embeddings** — prompt-only routing; no client keywords. Lexical seed engines and CBM proxy are opt-in.

```
Prompt (any language)
  │
  ├─ graph_backend = proxy_cbm? ──► CBM search_graph + snippets
  │                                      │
  └─ native (default) ◄──────────────────┘ fallback_native
      │
      ▼
MiniLM query embed (bundled ONNX, singleton + per-packet cache)
  │
      ▼
ANN on embeddings.bin sidecar → graph-resolve hits
  │
      ▼
QueryPlan (intent + concepts)
  │
      ▼
Tiered retrieval (L1 embed-primary → L2 patterns → L3 recovery)
  │
      ▼
Seed files always ship (skeletonized)
  │
      ▼
Fill callees, usages, imports under fill_cap
  │
      ▼
Evidence packet → MCP client
  │
      └─ expand_fold restores a body from the registry
```

## Embed-primary routing (v0.8.6)

Release binaries include **MiniLM multilingual Q** weights (`models/minilm-multilingual-q/`). At runtime:

1. **Index** — symbol sketches → `embeddings.bin` (sidecar v3: doc-enriched sketches + module centroids).
2. **Query** — embed prompt once (semantic LRU for near-duplicates).
3. **ANN** — top hits from sidecar (`ann_top_k: 16`, insert cap `embed_seed_cap: 4`).
4. **Graph resolve** — unique symbol → file seeds; no raw file dump.
5. **Escalate** — L2 pattern expand, then L3 recovery with lower `recovery_min_cosine` (0.38) if critical gaps remain.

| Tier | Role | When |
| :--- | :--- | :--- |
| **L1** | Embedding-primary seeds | Always; early exit when sufficient |
| **L2** | Pattern templates (`trace_routing`, `trace_middleware`, …) | Critical gaps only |
| **L3** | Bounded semantic recovery (max 2 seeds) | Still critical after L2 |

**Metadata:** `retrieval.resolution_tier` (`embedding_primary`, `L1_exact`, `L2_pattern`, `L3_semantic_recovery`), `retrieval.embedding_used`, `retrieval.cache_hit`, `coverage.claim`.

## Custom seed engines (opt-in)

Advanced users can set `seed_resolution.engine` in `nm.config.json`:

| Engine | Use case |
| :--- | :--- |
| *(default — embeddings ON, no override)* | **Embed-primary** — prompt only |
| `keywords` / `keywords_expanded` | Lexical + optional `auto_extract_keywords` |
| `hybrid` | Embed + lexical (Arabic-heavy projects) |
| `off` | Disable seed resolution |

The default embed path uses the embedding seed resolver internally — no `seed_resolution` block required for normal use.

## Graph proxy (optional)

When `graph_backend` is `proxy_cbm` or `auto` finds CBM, only **`get_context_packet`** uses the external MCP server. Folding, `search_symbols`, and `trace` stay native. **`native` + embed remains the recommended default.**

See [graph-proxy.md](graph-proxy.md) and crate `neuromesh-graph-proxy`.

## Guarantees

1. **Structural honesty.** Import and call edges exist when the target resolves uniquely.
2. **Bounded activation.** Seeds from the prompt; fill under a token cap — not whole-repo scoring.
3. **Reversible folds.** `[neuromesh:fold]` markers; `neuromesh_expand_fold` restores bodies from registry.
4. **Safe workspace.** Indexing refuses `$HOME` and drive roots; auto file cap with production-first ordering.
5. **Local.** MCP over stdio. No hosted service for indexing.
6. **MCP resilience.** Panicking tools return `-32603` without killing stdio.
7. **Bundled model.** Release tarballs ship MiniLM weights — no HuggingFace download at install.

## Crates

| Crate | Role |
| :--- | :--- |
| `neuromesh-embed` | MiniLM via fastembed `UserDefinedEmbeddingModel`, query cache, semantic LRU |
| `neuromesh-parser` | Language registry, tree-sitter queries, regex fallbacks |
| `neuromesh-graph` | Neural mesh: ingest, search, trace, Physarum, STDP, **embeddings.bin** sidecar |
| `neuromesh-task` | Intent + identifier extraction |
| `neuromesh-context` | Genetic splice, fold registry, **tiered retrieval** (`retrieval/`), gold harness |
| `neuromesh-index` | Walker, hashes, language from path |
| `neuromesh-memory` | Project facts from manifests and docs |
| `neuromesh-graph-proxy` | External graph backends (CBM) — opt-in |
| `neuromesh-mcp` | MCP JSON-RPC 2.0 over stdio |
| `neuromesh-cli` | `mcp`, `monitor`, `index`, `eval`, `doctor`, `connect`, `embed prefetch` |
| `neuromesh-router` | Osmotic QualityGate (mode vs critical tasks) |
| `neuromesh-cache` | Mycelial / hyphal prefetch |
| `neuromesh-api` | Local monitor HTTP / SSE |
| `neuromesh-core` | Shared types, `EmbeddingConfig`, budgets |

See also [nature.md](nature.md) for the biological metaphor map.
