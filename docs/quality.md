# Quality

Claims in this project are supposed to come from commands you can run, not from a padded corpus.

## Gold harness

Built-in and project-local **gold tasks** pair natural-language prompts with **path-qualified** expected files. Fixture workspaces cover representative stacks and task shapes — not only “where is this symbol”:

- Web: Vue/React components, auth guards, SCSS tokens, checkout flows, dead-code detection
- Backend: Laravel Eloquent + migrations, FastAPI, Rails, Nest, Gin, Axum, ASP.NET, Ktor, Express routes
- Mobile / cross-platform: Flutter widgets, SwiftUI, Next.js routes
- Config & schema: JSON/env config, SQL, Zod-like type cores with benchmark/locale decoys
- Refactor / edit-style prompts alongside trace-and-explain tasks

Thresholds locked in CI:

- recall ≥ **0.8**
- precision ≥ **0.4** (a forbidden gold file in the packet forces precision to 0)
- missed seeds reported as `partial`, or `no_seed_resolved` when **every** identifier missed (empty packet → Grep immediately)
- `expand_fold` restores a registered body without reading the disk
- activation under **250 ms** in the debug gold test on the main repo (non-Windows CI; parallel `cargo test`; isolated release runs sit nearer 50 ms)
- skeletonizer folds **bodies** from graph/tree-sitter spans; seed callees stay exons; fill caps stay 0 / 5k / 16k extra tokens
- after skeletonization, the packet itself is capped (6k / 12k / 24k) by dropping optional files then reducing per-file exons; gold still measures **file-path** recall
- each seed file keeps at most **4** open bodies (optional files **1**); the skeleton is a window (imports + enclosing type + top spans), not the whole file
- symbol `NodeId` includes the enclosing type so inner-class methods are distinct spans
- the graph stores **no file bodies**: a loaded snapshot has `content = None` on every node, and source is read on demand for skeleton/fold
- snapshot cold load and a single-file reindex must each stay at or under a full workspace index

```bash
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
cargo test -p neuromesh-context gold_harness_on_fixture_repos -- --nocapture
neuromesh eval
```

`neuromesh eval` prints **workspace / selected / packet** tokens, reduction vs both, recall, precision, **Grep still needed**, and latency. Published numbers must come from that table — not from a padded corpus or a global 99% claim.

## Tiered retrieval & release gates (v0.8.2)

MCP and `neuromesh packet --json` use **single-pass** L1→L2→L3 escalation. L2 pattern expand and L3 semantic recovery run only when **critical gaps** remain after the sufficiency check.

```bash
neuromesh eval --release-gates              # multi-metric gate report
neuromesh eval --release-gates --calibrate  # dev-split threshold tuning
```

Optional: run the multilingual MCP benchmark driver against any indexed Express (or similar) workspace and compare JSON summaries release-over-release.

| Gate | Target |
| :--- | :--- |
| Assisted recall (holdout) | ≥ 55% |
| L3 rate | ≤ 15% |
| FSR proxy | &lt; 10% |
| Full-workspace fallback | 0 |

`false_sufficiency_rate` is **`null`** when no `task_success` labels exist (CLI eval without agent simulation). FSR **proxy** uses `likely_sufficient` + gold recall &lt; 0.5. **Proxy v0.8.2+** never emits fixed sufficiency/confidence scores — treat proxy `retrieval.claim` as conservative (`partial` / `bounded` only).

## Multilingual MCP benchmark (v0.8.3)

Holdout matrix: **60 cells** (10 languages × 6 Express-oriented tasks). MCP stdio `get_context_packet` with **raw** args (prompt only — server auto-extracts keywords/expansion).

Build a **release** binary before measuring; debug builds skew latency.

| mode | recall | precision | no_seed | warm p50 |
| :--- | ---: | ---: | ---: | ---: |
| native raw + server auto-extract (v0.8.3) | **0.460** | **0.811** | 0/60 | ~34 ms |
| native assisted (client keywords, v0.8.2) | 0.431 | 0.790 | 0/60 | ~35 ms |
| native raw (no keywords, v0.8.2) | 0.333 | 0.622 | 11/60 | ~48 ms |

**Interpretation**

- **Server-side assisted default** (v0.8.3): raw MCP calls match or exceed v0.8.2 client-assisted recall/precision with **zero no_seed** across all languages. Opt out via `auto_extract_keywords=false` or `NEUROMESH_AUTO_EXTRACT_KEYWORDS=0`.
- Re-run: `node test3/mcp_driver_v2.mjs <release-neuromesh> <express-workspace> <outdir> 6 raw native`

## Multilingual MCP benchmark (v0.8.2, historical)

| mode | recall | precision | no_seed | warm p50 |
| :--- | ---: | ---: | ---: | ---: |
| native assisted | **0.426** | **0.789** | 0/60 | ~35 ms |
| native raw | 0.333 | 0.622 | 11/60 | ~48 ms |
| proxy_cbm assisted | 0.578 | 0.504 | 0/60 | ~230 ms |
| proxy_cbm raw | 0.448 | 0.311 | 4/60 | ~230 ms |

**Interpretation (v0.8.2)**

- **Native assisted** required client-supplied keywords/expansion for NL prompts.
- **Native raw** missed on NL middleware without client keywords (11/60 no-seed) — fixed in v0.8.3 server auto-extract.

Re-run the same driver with `NEUROMESH_GRAPH_BACKEND=native|proxy_cbm` and raw vs assisted keyword/expansion args to reproduce.

## Grep after get_context

From `neuromesh eval` on the NeuroMesh workspace (balanced, release):

| Task | WS tok | Selected | Packet | vs WS | vs selected | Recall | Prec | Grep | ms |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | 650859 | 72428 | 17389 | 97.3% | 76.0% | 1.00 | 0.75 | **0** | **22** |
| `physarum_usage` | 650859 | 19625 | 4080 | 99.4% | 79.2% | 1.00 | 0.50 | **0** | **12** |

Debug `neuromesh eval` on the same workspace: activation ~157 ms / ~104 ms; full index ~2.3 s.

That measures “did the packet already hold the files a developer would open”, not a live multi-agent trial. Quote this table; do not invent a global 99% figure.

## Hot-path optimization (v0.7.17)

Measured with release `neuromesh eval` and an MCP stdio driver (25 warm repeats per prompt). Correctness unchanged: same recall/precision/coverage on benchmark prompts.

### `neuromesh eval` latency (main repo, balanced)

| Task | Before ms | After ms |
| :--- | ---: | ---: |
| `handle_tool_call_intent` | 44 | **22** |
| `physarum_usage` | 22 | **12** |

`handle_tool_call_intent` max_savings: 30 → **17** ms. Index time unchanged (~550 ms).

### MCP stdio warm p50 (5 prompts summed)

| Corpus size | Before ms | After ms | Notes |
| :--- | ---: | ---: | :--- |
| Small (Express sample) | 112 | ~140 | Run-to-run variance ±30 ms; files/coverage identical |
| Large (NeuroMesh repo) | 487 | **487** | Selector/registry/feedback prompts dominate |

Compare before/after JSON artifacts from the same driver configuration when validating regressions.

## Learning → emission (v0.7.17)

Feedback changes **which files are emitted** in both directions: penalized hop-expanded files drop out; reinforced files that **match the current query focus terms** are prepended into optional emission via `ensure_learned_emission`. Default promotion floor: `learning_promotion_min_bonus` **14** (covers +8 strong reinforcement ≈ 17 bonus). Unrelated queries still get at most `learning_relevance_cap_unrelated` (default **0.35**) of the learned score in ranking — they do not inject new files into the packet.

Learning is **not passive**: repeated `get_context` alone does nothing; agents must call `neuromesh_record_feedback` after a successful edit (`task_success` + `touched_nodes`).

Causal routing is gated in CI:

```bash
cargo test -p neuromesh-context learning_to_emission
cargo test -p neuromesh-context reinforced_file_promotes
cargo test -p neuromesh-context learning_does_not_leak
cargo test -p neuromesh-context deterministic_packet_same_state
cargo test -p neuromesh-context catastrophic_learning
```

`neuromesh eval --learning` runs a dose-response sweep on the learning-causal fixture and prints reinforcement → bonus → rank → emitted → MRR.

`neuromesh_explain_packet` → `selection.candidates` includes `selected`, `emitted`, `drop_stage`, and `score_breakdown` (`utility_score`, `learned_score`, `negative_penalty`, `final_score`). Use these when `selected: true` but a file is missing from the packet.

Configurable learning thresholds live in `Thresholds` (`penalized_suppression_threshold`, `learning_promotion_min_bonus`, `learning_relevance_cap_unrelated`, `learning_decay_half_life_days`, `max_learned_influence`) — see `neuromesh-core` config defaults.

## Shop-style fixture (Vue + Pinia + SCSS)

From the same `neuromesh eval` run (balanced):

| Task | Recall | Prec | Packet files (gold hit) |
| :--- | ---: | ---: | :--- |
| `price_card_scss` | 1.00 | 1.00 | tokens, mixins, ProductCard, price-card SCSS |
| `dead_code_gocart` | 1.00 | 1.00 | ui.js, App.vue, CartDrawer, Header |
| `checkout_qty_stepper` | 1.00 | 0.40 | CheckoutView, cart store (+ connector views) |

## Index snapshot

From release `neuromesh eval` on the NeuroMesh workspace:

| Metric | Value |
| :--- | ---: |
| Files | 340 (build artifacts ignored) |
| Nodes | 3,161 |
| Edges | 6,795 |
| Workspace tokens | 650,859 |
| **Index time (release)** | **552 ms** |
| Index time (debug) | ~2.3 s |

Index file cap is **auto** by default (production sources first, tests last, ceiling 50,000). Override with `neuromesh index --max-files N`. See [cli.md](cli.md#index-file-cap).

## Compact mesh: snapshot load and one-file reindex

The mesh keeps a structural skeleton in RAM. File bodies are not stored in nodes and not written to the snapshot; source is read on demand when a packet is spliced. The snapshot is a binary graph blob; a legacy JSON migration path is read once when present.

From the snapshot load benchmark (release, NeuroMesh workspace):

| Metric | Value |
| :--- | ---: |
| Files scanned | 340 |
| Nodes | 3,161 |
| Full workspace index | 790 ms |
| Snapshot size | 3.9 MB |
| **Snapshot cold load** | **55 ms** |
| **One-file reindex** (parse + local relink) | **80 ms** |

```bash
cargo test --release -p neuromesh-graph --lib snapshot_load_and_single_file_reindex -- --nocapture
```

The walker compares size + mtime before reading, so an unchanged tree is a metadata walk with zero full-file reads. `neuromesh index` prints `Unchanged skip` for those files. Live sync uses an OS watcher (200 ms debounce) instead of a full-tree poll.

Re-ingesting one file re-queues the **inbound** `Calls`/`Imports` edges that pointed at its symbols, so callers keep their edges without a full reindex.

Fill caps: `max_savings` = 0 extra tokens, `balanced` = 5,000, `max_quality` = 16,000. Packet caps after skeletonization: 6,000 / 12,000 / 24,000. Reduction is versus **the indexed workspace**, not a fake fixed dump.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
