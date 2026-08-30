# Architecture

NeuroMesh builds a **structural project graph**, then a **task-conditioned packet**. The graph is the nervous system. The packet is the thought.

The names come from living tissue — [nature.md](nature.md) — so contributors have a shared vocabulary. The runtime stays honest: unique edges, real budgets, reversible folds.

```
Prompt
  │
  ├─ graph_backend = proxy_cbm? ──► CBM search_graph + snippets (v0.8.2)
  │                                      │
  └─ native (default) ◄──────────────────┘ fallback_native
      │
      ▼
QueryPlan (intent + concepts) — v0.8.2
  │
  ▼
Identifiers, paths, alias expansion, alias code seeds (middleware/routing NL)
  │
  ▼
Unique / import-aware / impl-aware resolve
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
  ├─ learning rerank + bidirectional emission (v0.7.17+)
  │
  ├─ max_savings: seeds only
  ├─ balanced: +5k extra, soft crate cap
  └─ max_quality: +16k extra
  │
  ▼
Evidence packet → MCP client
  │
  └─ expand_fold restores a body from the registry
```

## Tiered retrieval (v0.8.2)

**North star:** MSC via graph — no embedding in L1/L2; embedding optional L3 only.

```
Query → QueryUnderstanding → QueryPlan (intent + concepts)
  → symbol/alias/concept seeds → L1 fast match → sufficiency
  → (critical gaps only) L2 pattern expand → L3 semantic recovery (max 2 seeds)
  → MSC packet or controlled partial
```

| Tier | Engine | Hops | When |
| :--- | :--- | ---: | :--- |
| **L1** | `keywords_expanded` + concept index | 1 | Always; early exit when sufficient |
| **L2** | Pattern templates (`trace_routing`, `trace_middleware`, …) | 1 | Critical gaps only |
| **L3** | `semantic_lite` recovery seeds | 2 | Still critical after L2; max 2 seeds |

Single-pass escalation (`escalate.rs`) — no triple full re-activate per query. Runtime metadata on every MCP detail level (`retrieval_level`, `claim`, `critical_gaps`, `suggested_keywords`).

**Concept index** (`neuromesh-graph/concept_index.rs`): built at ingest from naming heuristics (`*Middleware`, `*Router`, `auth*`, `*Session`, …). Static alias clusters map NL → concept; code index maps concept → symbols.

**Sufficiency** is conservative: `likely_sufficient` requires high task-role coverage, dependency coverage, and zero critical gaps. Default when uncertain: `partial`. FSR proxy in `neuromesh eval --release-gates`. Proxy packets (`retrieval_level: "proxy"`) never stamp fixed scores — confidence/sufficiency are computed from matched vs expected keywords (cap ~0.45).

## Graph proxy (optional, v0.8.2)

When `graph_backend` is `proxy_cbm` or `auto` finds CBM, only **`get_context_packet`** uses the external MCP server. NeuroMesh forwards the full task context (`query` + `semantic_query` from keywords/expansion), parses CBM `{cols, rows}` JSON, filters empty-file Route hits, and shapes honest `retrieval` metadata. Other tools (`search_symbols`, `trace`, `expand_fold`) always use the native graph. **`native` remains the default** — test3 Express benchmark: native assisted beats proxy on precision (~0.79 vs ~0.50) and latency (~35 ms vs ~230 ms warm p50).

See [graph-proxy.md](graph-proxy.md) and crate `neuromesh-graph-proxy`.

## Guarantees

1. **Structural honesty.** Import and call edges exist when the target resolves uniquely (same file, imported files, same crate, impl/field, or a single global definition). Several hits in one file are not a fake `Proven` edge. Failures stay `Likely` or unresolved — they are not dropped silently and they are not exploded into every namesake.
2. **Bounded activation.** `get_context` seeds from the prompt and fills a neighborhood under a token cap. It does not score the entire graph on every request.
3. **Reversible folds.** Untargeted function bodies become `[neuromesh:fold]` markers. The original text is registered; `neuromesh_expand_fold` returns it by `fold_id`.
4. **Safe workspace.** Indexing walks up to a git, Cargo, or `package.json` root and refuses `$HOME` and drive roots. The file cap is **auto** (production sources first, tests last, ceiling 50,000) unless `--max-files` / `NEUROMESH_MAX_FILES` sets a limit.
5. **Local.** MCP over stdio. No hosted service, no API key for indexing.
6. **MCP resilience** (v0.7.17+). Each JSON-RPC request runs in an isolated task; a panicking tool returns `-32603` without killing the stdio session.
7. **Bounded fold store** (v0.7.17+). Folds from the current activation stay resolvable; older activations are LRU-trimmed (cap 2000) so long sessions do not grow without bound.

## Crates

| Crate | Role |
| :--- | :--- |
| `neuromesh-parser` | Language registry, tree-sitter queries, regex fallbacks, prompt anchors |
| `neuromesh-graph` | Neural mesh: ingest, search, trace, Physarum, STDP synapses, **concept index** |
| `neuromesh-task` | Intent + identifier extraction |
| `neuromesh-context` | Genetic splice (skeletonizer), fold registry, **tiered retrieval** (`retrieval/`), gold harness |
| `neuromesh-index` | Walker, hashes, language from path |
| `neuromesh-memory` | Project facts from manifests and docs |
| `neuromesh-graph-proxy` | External graph backends (CBM/Graphify) via MCP stdio — proxy packet + honest metadata |
| `neuromesh-mcp` | MCP JSON-RPC 2.0 over stdio |
| `neuromesh-cli` | `mcp`, `monitor`, `index`, `eval`, `doctor`, `connect` |
| `neuromesh-router` | Osmotic QualityGate (mode vs critical tasks) |
| `neuromesh-cache` | Mycelial / hyphal prefetch |
| `neuromesh-api` | Local monitor HTTP / SSE |
| `neuromesh-core` | Shared types (`NodeId`, `ContextView`, budgets) |

`get_context` resolves seeds, runs neighborhood Physarum when two or more seeds exist (capped subgraph, &lt; 20ms SLA), then fills remaining connectors under the token budget and skeletonizes. `get_stats` only marks Physarum active when that tube path ran. See [nature.md](nature.md).

## Index

1. Walk the workspace (skip `target/`, `node_modules/`, …).
2. Parse each file into symbols, imports, exports, and calls.
3. Ingest nodes.
4. `finalize_links`: resolve pending `Imports` then `Calls`.

Rust, TypeScript, Python, Go, Java, Kotlin, PHP, C#, Dart, Swift, and Ruby use tree-sitter **queries** (`function` / `class` / `import` / `call`) so spans stay real; regex is the fallback if a grammar fails to load. Grammars must stay on tree-sitter **ABI 13–14** (do not take crates that ship language version 15). Framework overlays (Android, Spring, Django, FastAPI, Next, Nuxt, Laravel, Pinoox, Symfony, WordPress, React, Vue router, SvelteKit, Astro, Twig, Electron, Tauri, Vite, Prime UI, Rails, Flutter, Express, Nest, Angular, Gin/Echo, Axum, ASP.NET, SwiftUI, Remix/React Router, Ktor) add `Api`/`Component`/`Config` nodes from annotations and file layout; unknown annotations are a soft miss. `.svelte`, `.astro`, `.twig`, `.cshtml`, and `.razor` are indexed (Vue-like SFC / HTML fallback; Razor `@code` is parsed as C#). `.css` / `.scss` / `.less` extract selectors, custom properties, and preprocessor tokens. `.svg` shares the HTML extractor (`id`, `<symbol>`, `<use href="#…">`). JavaScript uses the TypeScript query grammar. C / C++ share the generic regex parser. Vue has its own extractor. New languages plug in through `LanguageSpec`, not a growing engine `match`. Import hints from `composer.json` PSR-4 and `go.mod` are rewritten to workspace paths before unique-resolve. Parse runs in parallel per file; tree-sitter `Parser` is reused per thread; unchanged files still skip ingest by hash.

## Packet

Selector: required seed files, then optional connectors ranked by outbound calls, inbound usage, imports, **pheromone / STDP weight**, Physarum tubes, and unresolved-call closers. Per-crate fill is a **soft** cap — a high-scoring extra file from the same crate can still enter. Compound prompts split on `including` / `and how` / `as well as`; each cluster that has no code identifier still tries distinctive nouns against the graph, preferring files whose stem and sibling nouns (`guard`, `router`) match over a nested UI helper (`directive/permission`). A cluster with zero hits is `coverage.claim = partial`, not `no_recorded_gap`. Overlay file hints that look like paths (`theme/default/hello.twig`) bind the file before the template stem is resolved as a symbol.

Activator: skeletonize with graph function spans (fold threshold from `ContextChromosome.fold_threshold_lines`). Spans use the tree-sitter function **body**, not just the name. Each file keeps a ranked top-K of those bodies (seed `K=4`, optional `K=1`); the skeleton is a window of imports, enclosing type, exons, and fold markers — not the rest of the file. After splice, a packet cap (6k / 12k / 24k) drops optional files then reduces K; the top-scored method stays open. Fill caps stay real extra-token budgets (0 / 5k / 16k). **EmissionPipeline** records which candidates were selected vs emitted and why (`drop_stage`) after post-filters, learning rerank, fill cap, and packet cap. Register folds **for the MCP session** (project change clears the store; per-activation LRU trim keeps the current packet resolvable). MCP `get_context` defaults to a **minimal** packet (`packet_id`, coverage string, skeletons, fold descriptors without `original_body`); diagnostics live in `neuromesh_explain_packet`. `coverage` of `partial` or `no_seed_resolved` is the signal to Grep.
