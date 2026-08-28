# Quality

Claims in this project are supposed to come from commands you can run, not from a padded corpus.

## Gold

`tests/gold_tasks.toml` lists prompts and **path-qualified** gold files. Fixture repos live in `tests/fixtures/` (router, TS store including `require()` CJS + `tokens.json` CSS import, Vue/JS auth+permission-guard compound task with a `directive/permission` ranking thief and forbidden clipboard/profile decoys, **mini-shop** Vue 3 + Pinia + SCSS price-card / dead-code / checkout stepper, session, queue, string config, Python SMS, Kotlin SMS including a natural “received SMS stored” prompt, Dart SMS + Flutter widget, C# SMS, Next SMS route, Pinoox Pinx `get()->action()` plus `MainController::index` → Twig, Vue/PrimeVue `Dashboard.vue` + React `StatCard`, multi-app `apps/com_shop` vs `com_blog`, Laravel Eloquent + `Schema::create` migration + seeder/factory + SQL + JSON config plus route-only `POST /sms` prompts, FastAPI, Rails, Astro, Express route-only `POST /sms`, Nest, Angular, Gin, Axum, ASP.NET MapPost + Razor `@page`, SwiftUI, Remix/React Router, Ktor, LESS/SCSS/CSS badge tokens + SVG `smsInbox` icon, Zod-like schema core vs `packages/bench` + `locales/` + `v3/` + `v4/mini/` + json-schema decoys, plus a `z.infer` → `core.ts` type-alias task), including edit/refactor-style prompts — not only “where is this symbol”.

Thresholds locked in tests:

- recall ≥ **0.8**
- precision ≥ **0.4** (a `forbidden_files` hit forces precision to 0)
- missed seeds reported as `partial`, or `no_seed_resolved` when **every** identifier missed (empty packet, Grep immediately)
- `expand_fold` restores a registered body without reading the disk
- activation under **250 ms** in the debug gold test on this repo (non-Windows CI; cargo test is parallel; isolated release runs sit nearer 50 ms)
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

`neuromesh eval` prints **workspace / selected / packet** tokens, reduction vs both, recall, precision, **Grep still needed**, and latency. README numbers must come from that table — not from a padded corpus or a global 99% claim.

## Grep after get_context

From `cargo run --release -p neuromesh-cli -- eval` on this workspace (2026-08-28 v0.7.17, balanced):

| Task | WS tok | Selected | Packet | vs WS | vs selected | Recall | Prec | Grep | ms |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | 650859 | 72428 | 17389 | 97.3% | 76.0% | 1.00 | 0.75 | **0** | **22** |
| `physarum_usage` | 650859 | 19625 | 4080 | 99.4% | 79.2% | 1.00 | 0.50 | **0** | **12** |

Debug `neuromesh eval` on the same repo: activation ~157 ms / ~104 ms; full index ~2.3 s.

That is “did the packet already hold the files a developer would open”, not a live multi-agent trial. Quote this table; do not invent a global 99% figure.

## Hot-path optimization (v0.7.17)

Measured with `cargo run --release -p neuromesh-cli -- eval` and `mcp_driver.mjs` (25 warm repeats per prompt, 2026-08-28). Correctness unchanged: same recall/precision/coverage on all benchmark prompts.

### `neuromesh eval` latency (this repo, balanced)

| Task | Before (v0.7.16) ms | After (v0.7.17) ms |
| :--- | ---: | ---: |
| `handle_tool_call_intent` | 44 | **22** |
| `physarum_usage` | 22 | **12** |

`handle_tool_call_intent` max_savings: 30 → **17** ms. Index time unchanged (~550 ms).

### MCP stdio warm p50 (5 prompts summed)

| Workspace | Before ms | After ms | Notes |
| :--- | ---: | ---: | :--- |
| Express (`test-project`) | 112 | ~140 | Run-to-run variance ±30 ms; files/coverage identical |
| NeuroMesh (this repo) | 487 | **487** | Large-repo prompts (selector/registry/feedback) dominate |

Driver: `benchmark/nm_vs_cbm/mcp_driver.mjs`. Artifacts: `mcp_before_*.json`, `mcp_after_*.json`.

## Learning → emission (v0.7.16)

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

`neuromesh eval --learning` runs a dose-response sweep on `tests/fixtures/learning-causal/` and prints reinforcement → bonus → rank → emitted → MRR.

`neuromesh_explain_packet` → `selection.candidates` includes `selected`, `emitted`, `drop_stage`, and `score_breakdown` (`utility_score`, `learned_score`, `negative_penalty`, `final_score`). Use these when `selected: true` but a file is missing from the packet.

Configurable learning thresholds live in `Thresholds` (`penalized_suppression_threshold`, `learning_promotion_min_bonus`, `learning_relevance_cap_unrelated`, `learning_decay_half_life_days`, `max_learned_influence`) — see `neuromesh-core` config defaults.

## Shop fixture (mini-shop)

From the same `neuromesh eval` run (balanced, Vue 3 + Pinia + SCSS):

| Task | Recall | Prec | Packet files (gold hit) |
| :--- | ---: | ---: | :--- |
| `price_card_scss` | 1.00 | 1.00 | tokens, mixins, ProductCard, `_priceCard.scss` |
| `dead_code_gocart` | 1.00 | 1.00 | ui.js, App.vue, CartDrawer, Header |
| `checkout_qty_stepper` | 1.00 | 0.40 | CheckoutView, cart.js (+ connector views) |

## Index snapshot

From `cargo run --release -p neuromesh-cli -- eval` (2026-08-28 v0.7.17) on this repository:

| Metric | Value |
| :--- | ---: |
| Files | 340 (`target/` ignored) |
| Nodes | 3,161 |
| Edges | 6,795 |
| Workspace tokens | 650,859 |
| **Index time (release)** | **552 ms** |
| Index time (debug) | ~2.3 s |

Index file cap is **auto** by default (production sources first, tests last, ceiling 50,000). Override with `neuromesh index --max-files N`. See [cli.md](cli.md#index-file-cap).

## Compact mesh: snapshot load and one-file reindex

The mesh keeps a structural skeleton in RAM. File bodies are not stored in nodes and not written to the snapshot; source is read on demand when a packet is spliced. The snapshot is `graph.bin` (bincode); `graph.json` is still read once for migration.

From `snapshot_load_and_single_file_reindex_beat_full_index` (release, 2026-08-28, this repo):

| Metric | Value |
| :--- | ---: |
| Files scanned | 340 |
| Nodes | 3,132 |
| Full workspace index | 672 ms |
| Snapshot size | 3.8 MB |
| **Snapshot cold load** | **94 ms** |
| **One-file reindex** (parse + local relink) | **83 ms** |

```bash
cargo test --release -p neuromesh-graph --lib snapshot_load_and_single_file_reindex -- --nocapture
```

The walker compares size + mtime before reading, so an unchanged tree is a metadata walk with zero `read_to_string` calls (`metadata_walk_skips_unchanged_files`). `neuromesh index` prints `Unchanged skip` for those files. Live sync uses an OS watcher (`notify`, 200 ms debounce) instead of a full-tree poll.

Re-ingesting one file re-queues the **inbound** `Calls`/`Imports` edges that pointed at its symbols, so callers keep their edges without a full reindex (`reingest_file_relinks_inbound_calls`).

Fill caps: `max_savings` = 0 extra tokens, `balanced` = 5,000, `max_quality` = 16,000. Packet caps after skeletonization: 6,000 / 12,000 / 24,000. Reduction is versus **this workspace**, not a fake 25k dump.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
