# Architecture

NeuroMesh builds a **structural project graph**, then a **task-conditioned packet**. The graph is the nervous system. The packet is the thought.

The names come from living tissue — [nature.md](nature.md) — so contributors have a shared vocabulary. The runtime stays honest: unique edges, real budgets, reversible folds.

```
Prompt
  │
  ▼
Identifiers, paths, intent
  │
  ▼
Unique / import-aware / impl-aware resolve
  │
  ▼
Seed files always ship (skeletonized)
  │
  ▼
Fill callees, usages, imports under fill_cap
  │
  ├─ max_savings: seeds only
  ├─ balanced: +8k extra, soft crate cap
  └─ max_quality: +16k extra
  │
  ▼
Evidence packet → MCP client
  │
  └─ expand_fold restores a body from the registry
```

## Guarantees

1. **Structural honesty.** Import and call edges exist when the target resolves uniquely (same file, imported files, same crate, impl/field, or a single global definition). Several hits in one file are not a fake `Proven` edge. Failures stay `Likely` or unresolved — they are not dropped silently and they are not exploded into every namesake.
2. **Bounded activation.** `get_context` seeds from the prompt and fills a neighborhood under a token cap. It does not score the entire graph on every request.
3. **Reversible folds.** Untargeted function bodies become `[neuromesh:fold]` markers. The original text is registered; `neuromesh_expand_fold` returns it by `fold_id`.
4. **Safe workspace.** Indexing walks up to a git, Cargo, or `package.json` root and refuses `$HOME` and drive roots.
5. **Local.** MCP over stdio. No hosted service, no API key for indexing.

## Crates

| Crate | Role |
| :--- | :--- |
| `neuromesh-parser` | tree-sitter Rust/TS, regex fallbacks, prompt anchors |
| `neuromesh-graph` | Neural mesh: ingest, search, trace, Physarum, STDP synapses |
| `neuromesh-task` | Intent + identifier extraction |
| `neuromesh-context` | Genetic splice (skeletonizer), fold registry, gold harness |
| `neuromesh-index` | Walker, hashes, language from path |
| `neuromesh-memory` | Project facts from manifests and docs |
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

Rust and TypeScript use tree-sitter so `fn` / `impl` ranges and in-function calls are real spans. Other languages keep scoped regex extractors until those two stay green on eval.

## Packet

Selector: required seed files, then optional connectors ranked by outbound calls, inbound usage, imports, **pheromone / STDP weight**, Physarum tubes, and unresolved-call closers. Per-crate fill is a **soft** cap — a high-scoring extra file from the same crate can still enter.

Activator: skeletonize with graph function spans (fold threshold from `ContextChromosome.fold_threshold_lines`), register folds **for the MCP session** (cleared only when the project changes), report budget (`seed_tokens`, `fill_used` / `fill_cap`) and coverage. `next_actions` tell the agent to `expand_fold` or to Grep only when coverage is `partial`.
