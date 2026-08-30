# Architecture

NeuroMesh builds a **structural project graph**, then a **task-conditioned packet**. The graph is the nervous system. The packet is the thought.

**v0.9.0 default:** **`engine: fast`** — native graph + query-side lexical expansion; **no ONNX** at index or MCP startup. Opt in to **`hybrid`** (hierarchical sidecar v6) or **`deep`** (full symbol embed). CBM graph proxy remains opt-in.

```
Prompt (any language)
  │
  ├─ graph_backend = proxy_cbm? ──► CBM search_graph + snippets
  │                                      │
  └─ native (default) ◄──────────────────┘ fallback_native
      │
      ▼
retrieval.engine preset
  │
  ├─ fast (default) ──► QueryPlan + lexical/graph seeds
  │
  ├─ hybrid ──► MiniLM query embed → hierarchical ANN (file → lazy symbol)
  │
  └─ deep ──► MiniLM query embed → flat symbol ANN (all symbols at rebuild)
      │
      ▼
Tiered retrieval (L1 → L2 patterns → L3 recovery)
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

## Fast engine routing (v0.9.0 default)

Default **`engine: fast`** builds the AST graph only (`neuromesh index`). At query time:

1. **QueryPlan** — intent + server-assisted concept expansion from the prompt.
2. **Graph seeds** — lexical + structural resolution (no embed sidecar required).
3. **Escalate** — L2 pattern expand, then L3 lexical recovery if critical gaps remain.

Expect `retrieval.resolution_tier` **`lexical_primary`** on NL prompts unless the repo owner set `engine: hybrid` or `deep`.

## Hybrid / deep embed routing (opt-in)

When `engine` is **`hybrid`**, install **MiniLM multilingual Q** first (`neuromesh install embed minilm`):

1. **Index** — hierarchical sidecar v6: file tier at rebuild; symbol tier lazy on query.
2. **Query** — embed prompt once (semantic LRU for near-duplicates).
3. **ANN** — file ANN (top 4) → lazy symbol embed → symbol subset ANN + coarse pool fallback.
4. **Graph resolve** — unique symbol → file seeds; no raw file dump.
5. **Escalate** — L2 pattern expand, then L3 recovery with lower `recovery_min_cosine` (0.38) if critical gaps remain.

When `engine` is **`deep`**, the same model embeds **every symbol** at rebuild (flat sidecar). Query uses full two-stage symbol ANN plus module centroids and optional-file dedup — no file tier or lazy embed.

| Tier | Role | When |
| :--- | :--- | :--- |
| **L1** | Embedding-primary seeds | Always; early exit when sufficient |
| **L2** | Pattern templates (`trace_routing`, `trace_middleware`, …) | Critical gaps only |
| **L3** | Bounded semantic recovery (max 2 seeds) | Still critical after L2 |

**Metadata:** `retrieval.resolution_tier` (`embedding_primary`, `L1_exact`, `L2_pattern`, `L3_semantic_recovery`), `retrieval.embedding_used`, `retrieval.cache_hit`, `coverage.claim`.

## Retrieval engine presets

Set `retrieval.engine` in `nm.config.json` or via `neuromesh config engine`:

| `engine` | Use case |
| :--- | :--- |
| **`fast` (default)** | Zero-embed — graph + query-side lexical expansion |
| **`hybrid`** | MiniLM sidecar + graph (multilingual NL, obfuscated naming) |
| **`deep`** | Max quality + dedup + module centroids |

Seed resolution strategy is **derived from the preset** — no separate `seed_resolution.engine` knob.

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
7. **On-demand embed model.** Release tarballs are binary-only; hybrid/deep require `neuromesh install embed minilm` (no HuggingFace auto-download at runtime).

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

See also [nature.md](nature.md) for the biological metaphor map · [configuration.md](configuration.md) for presets and tuning.
