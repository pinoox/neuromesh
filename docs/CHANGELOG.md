# Changelog

All notable user-facing changes live here. The README stays a product guide, not a version diary.

## Unreleased

### Embedding performance (v0.8.5+)

- **Singleton embedder** — `Embedder::lazy_global` replaces per-query `try_new`; MCP background warm on startup; index warm after sidecar rebuild.
- **Per-packet query cache** — one ONNX inference per `get_context_packet` (seed path + confidence gate share vector).
- **Default model → MiniLM** — `minilm_multilingual_q`, 384-dim matryoshka; symmetric `query:` / `passage:` prefixes; Gemma remains opt-in quality tier.
- **ONNX threads** — `embeddings.intra_threads` (default 4); override with `NEUROMESH_EMBED_THREADS`.
- **CLI** — `neuromesh doctor --embed --bench` (p50/p95 warm latency).

### Embedding-primary default (v0.8.5)

- **Default seed engine** — `semantic_lite` with local **MiniLM multilingual Q** (`embeddings.enabled: true`). Prompt-only MCP/CLI — no client keywords required.
- **Keyword assist gated** — `auto_extract_keywords` runs only when seed engine is `keywords`, `keywords_expanded`, or `hybrid`.
- **Embedding confidence gate** — L1/L2 early exit blocked when resolved seeds have low prompt–sketch cosine; escalates to L2/L3 even on `partial`/`bounded`.
- **Honest low-confidence signal** — new coverage claim `no_confident_match` when embedding recovery finds nothing above `min_cosine`.
- **Metadata** — `retrieval.resolution_tier`, `retrieval.max_embedding_score`, per-seed `embedding_score`.
- **CLI default feature** — `embeddings` compiled in by default (`cargo build -p neuromesh-cli`).

## 0.8.4 — 2026-08-30

### L3 local embedding engine (optional)

- **`neuromesh-embed` crate** — fastembed-rs + ONNX Runtime; default model **EmbeddingGemma-300M Q4** (multilingual NL, ~150–200 MB download); fallback **MiniLM multilingual Q**.
- **L3-only** — vector ANN runs inside `semantic_lite` when L1/L2 still have critical gaps; max 2 recovery seeds; graph resolve after ANN (no file dump).
- **`embeddings.bin` sidecar** — index-time symbol sketches; invalidated on graph generation / file-hash change.
- **Config** — `embeddings` block in `config.json` / `nm.config.json`; `NEUROMESH_EMBEDDINGS=1`, `NEUROMESH_EMBED_MODEL=gemma300m_q4`.
- **CLI** — `neuromesh config embeddings on|off`, `neuromesh doctor --embed`.
- **Cargo feature** — `embeddings` (default off); build with `cargo build -p neuromesh-cli --features embeddings`.
- **MCP** — `retrieval.embedding_used` when L3 vector recovery fires.

## 0.8.3 — 2026-08-30

### Server-side assisted default

- **`get_context_packet`** auto-extracts English code `keywords` and related `expansion` from every prompt server-side (`auto_extract_keywords=true` default). Rule-based pipeline: query-intent packs → alias code seeds → embedded symbols → alias concepts. No LLM required.
- **FILL-ONLY-MISSING** — client-supplied keywords/expansion are never overwritten; only empty sides are populated.
- **Opt-out** — `auto_extract_keywords` MCP arg, `seed_resolution.auto_extract_keywords` in config, or `NEUROMESH_AUTO_EXTRACT_KEYWORDS=0`.
- **Tests** — 60-cell Express matrix inference gate (≥2 gold keyword hits), generic-repo regression, partial-fill precedence.
- **Docs** — MCP descriptors/protocol, agent-guide, quality benchmark table updated after re-run.

## 0.8.2 — 2026-08-30

### CBM proxy stabilization (hotfix)

- **B1** — Parse CBM `search_graph` `{cols, rows}` JSON (stock v0.8.1 returned 0 files on every proxy query).
- **B2** — Forward `keywords`, `expansion`, and identifiers to CBM via enriched `query` + `semantic_query`.
- **B3** — Honest proxy retrieval metadata (no fixed 0.55/0.65); `critical_gaps`, `suggested_keywords`, conservative confidence.
- **B4** — Drop phantom `Route` hits with empty `file` (no more `path: unknown` inflation).
- **Fix** — Keep MCP child process alive (`McpStdioClient` holds `Child` handle).
- **Tests** — `parse_cbm_cols_rows`, Route filter, frozen Express fixture under `neuromesh-graph-proxy/tests/fixtures/`.

### Native NL → seed (middleware)

- **`alias_seed_queries`** — NL middleware/routing prompts inject code seeds (`app.use`, `next`, …) without client keywords.
- Expanded FA/AR middleware alias terms; `TraceMiddleware` intent detects `لوله`, `خط أنابيب`, `next()`.
- **Docs** — architecture, graph-proxy, engines, quality (test3 v0.8.2 benchmark), site i18n updated for 0.8.2.

## 0.8.1 — 2026-08-29 (graph proxy + monitor)

### Graph proxy (CBM / Graphify)

- **`graph_backend` config** — `native` (default), `auto`, `proxy_cbm`, `proxy_graphify` in `Config` / `nm.config.json`. Env: `NEUROMESH_GRAPH_BACKEND`.
- **`neuromesh-graph-proxy` crate** — MCP stdio client, IDE config auto-detect, CBM adapter (`search_graph` + `get_code_snippet`).
- **`get_context_packet`** uses proxy when configured; **`fallback_native: true`** (default) keeps native tiered activation on failure.
- **CLI** — `neuromesh config graph-backend`, `neuromesh config seed-engine`, `neuromesh doctor --proxy`, `neuromesh doctor --proxy --probe` (live CBM connect).
- **Monitor** — Settings cards for graph backend + seed engine; `/api/engines`, `/api/graph-proxy/probe`; hot reconnect on save.
- **CBM project match** — `list_projects` + `root_path` → workspace (e.g. `neuromesh-repo` for `C:/projects/neuromesh`).
- **Docs** — [graph-proxy.md](graph-proxy.md), [engines.md](engines.md).

Concept graph retrieval: **Intent → Concept → Graph** without embedding in L1/L2.

- **Single-pass escalation.** One activation pass with incremental L1→L2→L3 (`escalate.rs`); L2/L3 only when **critical gaps** remain — no triple full re-activate per query.
- **Concept index.** Code-derived concept dictionary at graph ingest (`Middleware`, `Router`, `Session`, …) persisted beside `name_to_nodes`.
- **Query intent.** Rule-based `QueryPlan` (`trace_routing`, `trace_middleware`, `trace_auth`, …) maps prompts to expected concepts and edge types.
- **Concept seeds.** L1 seeds from concept index + static alias clusters before lexical fallback; multilingual NL via alias → concept → symbol.
- **L2 patterns.** Intent-driven bounded graph expansion (`patterns.rs`, max 6 files / 1 hop).
- **Stricter sufficiency.** Conservative `likely_sufficient` (`task_role ≥ 0.67`, `dependency ≥ 0.5`, zero critical gaps); default claim `partial` when uncertain.
- **Release gates.** FSR proxy (`likely_sufficient` + recall &lt; 0.5); `false_sufficiency_rate: null` when no `task_success` labels; `neuromesh eval --calibrate` on dev split.
- **Harness.** Removed invalid mini-express redirect gold; `retrieval` metadata on **all** MCP detail levels (`minimal` compact, `standard`/`diagnostic` full).
- **Token budget.** L1 selected cap **2K** (was 4K); L3 max **2** semantic recovery seeds.
- **Benchmark script.** `scripts/benchmark-v080.ps1` accepts `-Compare test3` for baseline diff.

Modular seed resolution, tiered retrieval, and MCP tool rename.

- **Tiered retrieval (L1→L2→L3).** Cost-aware orchestrator with conservative sufficiency early exit; `activate_tiered` used by MCP and `packet --json`. Runtime metadata: `retrieval_level`, `sufficiency_score`, `confidence`, `claim`, `critical_gaps`, `suggested_keywords`. Hard ban on full-workspace fallback.
- **Sufficiency model.** Production estimate separate from eval metrics; FSR tracking in `neuromesh eval --release-gates`. Benchmark suite A–F in `benchmark_suite` + `scripts/benchmark-v080.ps1`.
- **Seed engines.** Pluggable strategies (`off`, `keywords`, `keywords_expanded`, `semantic_lite`, `hybrid`) with weighted multi-signal ranking. MCP primary tool renamed to **`get_context_packet`** (`neuromesh_get_context` remains a deprecated alias). Dense `@nm:stack` / `@nm:seeds` / `@nm:flow` micro-header on the first file skeleton.
- **L1 multilingual pipeline.** Minimal alias clusters, Unicode normalize in signature extraction, `extract_embedded_code_tokens`, graph-aware lexical fallback. Brownfield-safe semantic_lite/hybrid.
- **Impact retrieval.** `retrieve_impact_context` via existing graph trace (callers/callees/tests/config).
- **CLI.** `neuromesh config seed-engine`, `eval --release-gates`, `packet --json` with `retrieval` block.
- **`nmx` CLI alias**, portable global MCP connect, unified telemetry/status.

## 0.7.17 — 2026-08-28

Hot-path optimization and stability hardening (activation + MCP stdio).

- **Graph path index.** Normalized `path_index` beside `file_to_nodes`; `file_id_for_path` is a hash lookup instead of a full-map scan with per-entry string allocation.
- **Per-file reads.** `nodes_in_file` / `file_node_paths` replace `get_all_nodes()` clones in `function_spans_for_file` and `resolve_file_path_noun`.
- **Learning index memo.** `file_learning_boost_index` is rebuilt once per mesh revision and shared via `Arc`; `file_min_base_relevance` uses `path_index` under a single read lock.
- **Sort keys.** Selector and activator precompute path tiebreak keys instead of allocating inside `sort_by` comparators.
- **MCP panic isolation.** Each JSON-RPC request runs in `tokio::spawn`; a panicking tool returns `-32603` and the stdio loop keeps serving.
- **Fold registry bounds.** Folds are LRU-trimmed per activation (`MAX_RETAINED_FOLDS` 2000); symbol/prefix lookups use indexes instead of scanning all folds.

### Fixed (benchmark regressions v0.7.16)

- **Seed extraction.** Lowercase member access (`app.handle`, `app.listen`, `Loader.init`) is extracted again; English stopwords (`does`, etc.) are no longer fallback seeds.
- **Dotted seed resolution.** `app.handle` resolves via owner hints (`application.js`) and member names become fold priority symbols so `handle`/`listen` stay open.
- **Compound cluster coverage.** A cluster is covered only when every significant term resolves — resolving `next` alone no longer blocks `middleware`/`pipeline` cluster seeding.
- **Call-graph tasks.** `callers and callees` prompts skip physarum, learning promotion, and wide neighborhood fill; optional files cap at direct trace neighbors (depth 1).
- **Focus-scoped learning.** Reinforced files enter emission only when they match the current query's focus terms; removed `learning_bonus ≥ 28` unrelated bypass that leaked files across tasks (e.g. Zod `parse.ts` into optional-modifier queries).
- **Technology detection.** `next()` in Express prompts no longer misclassifies as React/Next.js.

## 0.7.16 — 2026-08-28

Close the positive learning→emission loop (audit v0.7.15 follow-up).

- **Positive promotion.** `EmissionPipeline::ensure_learned_emission` prepends heavily reinforced files into the optional emission queue before materialize (focus match or `learning_bonus ≥ 28`).
- **Selector swap.** `promote_high_learning_into_emitted` uses `learning_promotion_min_bonus` (default **14**, was hard-coded **18**) so +8 reinforcement (~17 bonus) can enter the packet; displacement no longer caps victims at utility ≤ 20.
- **Threshold.** `Thresholds.learning_promotion_min_bonus` in config (serde default 14).
- **CI.** `positive_learning_unrelated_high_bonus_enters_emission`, `ensure_learned_emission_prepends_focused_file`; `routes.py` must show `emitted: true`.

## 0.7.15 — 2026-08-28

Adaptive context routing: learning now drives emission with full observability and benchmark harness.

- **Emission pipeline.** `EmissionPipeline` tracks `emitted` / `drop_stage` per file through filters, penalized suppression, learning rerank, fill cap, and packet cap. `rank_candidates` refreshed after materialization.
- **Unified score.** `compute_unified_file_score` merges utility, semantic, graph, learned (focus-aware + decay), pheromone, and penalty into `ContextScoreBreakdown`.
- **Explainability.** `RankCandidateView` adds `emitted`, `drop_stage`, `score_breakdown` for `explain_packet` diagnostics.
- **Configurable thresholds.** `penalized_suppression_threshold`, `learning_relevance_cap_unrelated`, `learning_decay_half_life_days`, `max_learned_influence`.
- **Learning eval.** `neuromesh eval --learning` dose-response sweep; `learning_eval` module with MRR, NDCG@K, Learning Gain, Emission Gain.
- **CI tests.** `LearningToEmissionCausalTest` (T5 + Kosha), determinism (4-run identical), catastrophic learning on unrelated queries, generalization, persistence reload.
- **Fixture.** `tests/fixtures/learning-causal/` for reinforcement benchmarks.

Complete the learning→emission loop and close v4 mini decoy gaps from benchmark reports.

- **Emission.** `promote_high_learning_into_emitted` swaps high-bonus learned files into the emitted optional set (displaces low-utility slots). Heavily reinforced files bypass per-crate caps (`learning_bonus ≥ 28`).
- **Feedback semantics.** `access_count` increments only on successful reinforcement; `node_learning_bonus` applies a demerit when `base_relevance < 1.0`. Penalized files show `penalized:` reasons in `selection.candidates`.
- **Seed demotion.** `file_min_base_relevance` demotes seeds when any symbol on the path is penalized (not only the file node).
- **v4/mini decoy.** `is_alt_surface_path` penalizes `mini` / `lite` / `slim` / `light` paths in activation scoring and decoy filters; mini-schema gold tasks forbid `v4/mini/schemas.ts`.
- **Tests.** Negative-feedback bonus regression, routes.py emission after saturation, alt-surface scoring.

## 0.7.11 — 2026-08-28

Learning weights now change `get_context` packet selection, not just persisted graph state.

- **Learning → routing.** `file_learning_boost` aggregates symbol + file reinforcement; learned files enter optional fill via focus-term match or high-bonus saturation; penalized seed files (`base_relevance < 0.55`) leave the required set.
- **Activation scoring.** `ActivationScorer` adds a `learning_lift` from `access_count` / `base_relevance` so `inactive_hints` and fallback scores reflect feedback.
- **Explain diagnostics.** `neuromesh_explain_packet` → `selection.candidates` lists `{path, score, learning_bonus, reason, selected}` for before/after feedback comparison.
- **Tests.** Selector acceptance tests for promote (PromoCodeInput), demote (App.vue), and kosha-style saturation (+50 on `routes.py` / `schema.py`).
- **Docs & landing.** MCP client lists now include OpenCode, MiMo CLI, and Gemini CLI in [README](../README.md), [mcp.md](mcp.md), [agent-guide.md](agent-guide.md), and the GitHub Pages site. OpenCode and MiMo CLI setup sections added to the agent guide.
- **Perf.** `file_learning_boost_index` builds learning scores in one graph pass; fixes gold harness latency regression on Linux CI (>200ms) from per-candidate full-graph scans.

## 0.7.10 ظ¤ 2026-08-28

Honest bounded coverage, physarum sidecar cap, and clearer learning feedback fields.

- **Coverage `bounded`.** `no_recorded_gap` only when seeds resolve, gaps are empty, no sidecar connector files, and the packet was not budget-truncated. Tasks with physarum/utility fill now report `bounded` instead of false-complete.
- **Sidecar files.** Optional connector fill (`physarum_tube`, `utility:*`) is capped at 3 files and marked `sidecar: true` in packet output; `coverage.sidecar_files` lists them.
- **Learning feedback clarity.** `record_feedback` returns `episode_saved_this_call`, `learning_episodes_in_store`, and `persisted_to: graph.bin`. `episodes_recorded` kept for compatibility (per-call 0/1).

## 0.7.9 ظ¤ 2026-08-28

durable learning, parser relink, and Vue trace on stale snapshots.

- **Learning persistence (real).** `record_feedback` saves `graph.bin` via `workspace_root` (not MCP `cwd`). Episode IDs are checkpointed in the snapshot (`applied_learning_episodes`). Replay on startup applies only episodes missing from the checkpoint, then persists. MCP flushes graph state on graceful stdio shutdown.
- **Parser epoch relink.** `GRAPH_PARSER_EPOCH` (3) in snapshots; loading an older epoch clears `file_hashes` so the next index re-parses all files (Vue CALL edges on upgraded installs without manual `--force`).
- **Windows routing fix.** `tighten_focused_view_selection` normalizes `\` paths so `cart.js` is kept on checkout stepper tasks (gold `checkout_qty_stepper`).
- **Benchmark fixture.** `ProductGrid.vue` `@view` binding corrected to `ui.openProduct` (action lives in `ui.js`).

## 0.7.8 ظ¤ 2026-08-28

RETEST v0.7.7 follow-up: cross-session learning, Vue trace edges, and honest fuzzy trace.

- **Learning persistence.** `graph.bin` snapshot digest now includes `access_count`, `base_relevance`, and edge pheromone weights so `save_persisted` after `record_feedback` is not skipped as ظ£structurally unchangedظإ. Episodes replay on MCP startup when graph nodes are still cold (`warmup_project_learning`).
- **Vue trace / dead-code.** Pinia `actions` methods become `Function` symbols; template `@click` / `@view` and `<script setup>` store calls emit `Calls` edges; store file hints are extension-agnostic (`stores/ui` resolves `ui.js`). `trace` inbound callers work for `goCheckout` and similar.
- **Trace honesty.** `TraceResult.origin_reliable` and `match_reason: "fuzzy"` when the seed is not an exact/prefix hit ظ¤ agents must not treat fuzzy origins as dead-code proof.
- **Pinia alias resolve.** Store alias `ui` maps to `useUiStore::action` during call linking.

## 0.7.7 ظ¤ 2026-08-28

Benchmark-driven routing, honest coverage, and a falsifiable learning loop (Vue shop fixture).

- **Shop gold fixture.** `tests/fixtures/mini-shop/` (Vue 3 + Pinia + SCSS) with gold tasks for price-card SCSS (T6), dead-code `goCart` (T7), and checkout `setQty` (T5). Runs in `gold_harness_on_fixture_repos`.
- **Style routing.** Unified SCSS file hints (`tokens.scss` / `mixins.scss` with and without `_`), `_priceCard.scss` seeds, StyleToken search seeds, and cart/promo noise filtering so style tasks do not ship cart components.
- **View seeds.** `related_concepts` become seeds; checkout/stepper prompts inject `CheckoutView` without `checkout` ظèâ `cart` false positives; focused checkout tasks tighten optional fill.
- **Coverage honesty.** `CoverageReport` adds `covered`, `skipped`, `semantic_coverage`, and `packet_gaps` (with optional `line`). `no_recorded_gap` only when seeds and packet gaps are both empty.
- **Structural proof.** `structural_evidence` includes `exact_line`, `who_reads`, `callers_count`, and `is_dead`; bug-hunt tasks can emit `bug_line` gaps (e.g. duplicate discount in `total()`).
- **Vue template ظْ store.** `@click` / `@submit` handlers (`ui.goCart()`) emit `Calls` edges to Pinia actions for dead-code and caller tracing.
- **Learning loop.** `record_feedback` resolves human names, persists `base_relevance` to `graph.bin`, saves episodes, reinforces callee edges, and recalls successful episodes on the next `get_context`. `neuromesh_get_node_weights` exposes deltas for verification.
- **New MCP tools.** `neuromesh_expand_gap` (cheap skeleton for gap paths) and `neuromesh_get_node_weights` (observability).
- **Selector / skeleton.** `learning_bonus` boosts file fill from `access_count` / `base_relevance`; files ظëج60 lines fold more aggressively.
- **Docs / ops.** README and [mcp.md](mcp.md) warn against `mcp` without a workspace path; [mcp.md](mcp.md) documents learning timing and new tools.

## 0.7.6 ظ¤ 2026-08-27

Agent setup guide, HTTP route seeds, Pinx/Vue overlays, Laravel/SQL/JSON, and tighter TS seed ranking.

- **Agent guide (all clients).** [agent-guide.md](agent-guide.md) walks connect ظْ instructions for Cursor, VS Code/Copilot, Claude, Codex, Antigravity, Kilo, Trae, MiniMax, Windsurf, Cline/Roo, and Zed, plus a universal paste block, one-shot prompt, and smoke test. Cursor template remains [agent-rule.mdc](agent-rule.mdc); [mcp.md](mcp.md#agent-rule-recommended) links the tutorial.
- **Route identifiers.** `POST /sms`, `/api/v1/sms`, and `https://example.com/sms` (path only) are identifiers ظ¤ never file hints ظ¤ so `/sms` cannot steal every SMS fixture via `resolve_file_hint`. GitHub-style `/org/repo` URLs stay out.
- **Api path aliases.** An `Api` node named `POST /sms` is also indexed as `/sms` for exact search; the last segment (`sms`) is not aliased.
- **Routeظْhandler edges.** Laravel `Route::post('/sms', [SmsController::class, 'store'])`, Axum `.route(..., post(store))`, FastAPI `@app.post`ظْ`def`, and Express named handlers emit `Calls` from `POST /sms` to the handler. Gold: `mini-laravel` / `mini-express` route-only prompts.
- **Pinx `get()/post()` routes.** `get('/')->action([MainController::class, 'index'])->name('home')` is an `Api` node, not only the older `action([Class, method])` form. `collection('/api')`, `action('home', [Class, method])`, `render()` / `view()`, `app.php` package/theme/pinx, and `vite()` entry hints are overlayed too.
- **Single-app vs multi-app.** Root `app.php` + `bin/pinx` is Pinx single-app; `apps/com_*` with nested `app.php` is multi-app. Theme `package.json` under `theme/` is scanned for Vue, PrimeVue, PrimeUIX, Pinia, and React.
- **Vue kebab-case and React `FC`.** `<data-table>` becomes `DataTable`; `const StatCard: FC = () =>` is a `Component`. Gold: `mini-pinoox` dashboard/StatCard, `mini-pinoox-platform` shop vs blog.
- **Laravel is a real stack, not only `Route::get`.** Eloquent `Model` / `$table` / `belongsTo` become `DbModel` nodes; `Schema::create`, seeders, factories, `Route::resource` / `match`, and Blade `@include` overlay too. Gold: `mini-laravel` store/route/migration/seeder/SQL/JSON.
- **SQL and JSON are parsed.** `CREATE TABLE` / views / routines are `DbModel` symbols; `config/*.json` and `package.json` scripts are `Config` (dependency maps are skipped). Lockfiles stay out of the walk.
- **JS/TS modules and stylesheets.** `require()` / `import()` / CSS+JSON side-effect imports; nested SCSS `@include` / `@function` / `@keyframes`; comma-nested classes in CSS/SCSS/Less. Gold: `mini-store` theme+CJS, `mini-styles` SCSS+CSS.
- **Tighter name matches beat decorated twins.** `safeParse` outranks `parseSimpleObject` / `parseNestedObject` for the identifier `parse`; substring score now scales with how much of the symbol name is the identifier.
- **Bench, locale, and legacy paths are decoys.** `bench/`, `locales/` / `i18n/`, and `v3/` / `compat/` / `legacy/` are penalized like tests unless the prompt is about them. `to-json-schema` loses to `core/parse` on parse/validate questions.
- **Type aliases seed.** `z.infer` extracts `infer` and stems to `output` / `input`, so generic questions hit `export type output<T>` in `core.ts` instead of a runtime schema helper.
- **Natural phrasing still seeds.** `parse()` and `z.object` are identifiers; `parsing` stems to `parse`, so "how does parsing work" is not an empty packet.

CI gold harness: migration vs SQL decoys, and a slightly looser debug latency gate.

- **Schema decoys are prompt-kinded.** A "migration" question no longer seeds `database/sql/*.sql`; a `.sql` / `CREATE TABLE` prompt no longer seeds `migrations/`. Seeder and factory twins follow the same split.
- **Migration short name.** `2024_ظخ_create_sms_messages_table` also registers `create_sms_messages_table` so that identifier seeds the PHP file.
- **Debug gold latency.** Non-Windows gold harness allows **&lt; 200 ms** (was 150) so Linux CI debug builds do not flake at ~157 ms.

## 0.7.4 ظ¤ 2026-08-27

Compound-task quality, Pinoox ViewظْTwig, and complete MCP usage telemetry.

- **Cluster seeds pick the router guard, not a UI helper.** A "router permission guard" clause still splits, but the noun `permission` now prefers `src/permission.js` / store permission modules over `directive/permission`. Clipboard and profile decoys are forbidden in gold.
- **Pinoox ViewظْTwig is a walkable `Calls` edge.** `View::render('hello')` attaches to the rendering method, binds `theme/{theme}/hello.twig` by file path before the stem `hello` can steal another symbol, and `get_context` / `trace` on `MainController::index` ship the template without the prompt saying `twig`.
- **MCP usage is complete.** Handshake writes one `mcp_session` row; trace, deps, stats, explain, architecture, impact, and feedback append too. Mean reduction ignores 0-token rows so search/session do not drag the %. Monitor `GET /api/usage` reports the token-weighted overall %, and telemetry POST works without a Tokio handle.
- **Explicit MCP workspace stays put.** `neuromesh mcp <dir>` and initialize `rootUri` no longer walk up to a parent git/`Cargo.toml` root, so a fixture like `mini-auth` is not mixed with the rest of the repo.
- **`Type::method` seeds resolve, templates beat namesake helpers.** `MainController::index` binds the method (not a missed seed), and `hello.twig` outranks `Greeter.hello()` so the decoy stays out of the packet.

## 0.7.3 ظ¤ 2026-08-26

Workspace confinement, first-query readiness, and compact MCP packets.

- **Workspace confinement.** `get_file_skeleton` and `read_source` refuse absolute paths, `../` traversal, and symlink escape. `neuromesh index` / start / monitor / eval / connect refuse home and filesystem roots in under 100ms. CLI `Processed Tokens` is the current run; `Workspace Tokens` is the graph total.
- **Index readiness.** Cold MCP `get_context` waits for the first index (or returns `indexing_in_progress`) instead of an empty `no_seed_resolved` packet. `neuromesh_get_stats` includes `index_state`, `generation`, and `ready`.
- **Coverage honesty.** Imperative verbs (`Modify`, `Refactor`, ظخ) are not seeds. Equivalent file-path forms count as a hit. Unknown `mode` is a tool error. `.env` is skipped; `.env.example` and siblings are indexed.
- **Pinoox ViewظْTwig.** `View::render('hello')` links `theme/{theme}/hello.twig` with a `Calls` edge so the template ships with the controller.
- **No fold bodies on the wire.** `get_file_skeleton` and `get_context` return `FoldDescriptor` (id, symbol, signature, lines, saved tokens). `original_body` stays in the session registry and returns only from `neuromesh_expand_fold`.
- **Minimal `get_context` by default.** Response is `packet_id`, `coverage` (`no_recorded_gap` | `partial` | `no_seed_resolved`), `tokens`, skeletonized `files`, `missing`/`next` only when coverage is incomplete. `mode` still picks files; `response_detail` (`minimal` | `standard` | `diagnostic`) picks metadata.
- **`neuromesh_explain_packet`.** Fetch seeds, selection, budget, physarum, and membrane for a `packet_id` from a 32-slot / 10-minute LRU (no source bodies). Unknown or expired ids are a tool error.
- **Compact MCP wire.** Tool `content[].text` is minified JSON of the same object as `structuredContent` (not pretty-printed). HTTP `/api/simulate` still requests `diagnostic` so the VS Code inspector is unchanged.

## 0.7.1 ظ¤ 2026-08-26

Compound-task coverage is honest: each topical cluster seeds independently, and a named half that misses is `partial` ظ¤ never a silent `no_recorded_gap`.

- **Cluster seeds.** `including` / `and how` / `as well as` split the prompt. A clause with no camelCase identifier (e.g. "router permission guard") still tries those nouns against the graph, so `src/permission.js` ships with the login module instead of being omitted while `coverage.claim` says complete.
- **False-complete coverage.** If that second cluster resolves nothing, `seeds_missed` is non-empty and `claim` is `partial` (Grep in `next_actions`). `unresolved` stays graph call/import gaps ظ¤ it is not a list of missing task nouns.
- **Usage from IDE chat.** `neuromesh_expand_fold` now appends a telemetry row (it previously only recorded the inactive-node path). Handshake / chatting without a tool call still does not. Rows use unique request ids so two calls in the same millisecond are not dropped.

## 0.7.0 ظ¤ 2026-08-26

Accurate cheaper packets: task-matched methods stay open, windows replace whole-file skeletons, and a hard packet cap cuts cost.

- **Task exons.** Skeletonization scores each function against the prompt (`nullSafe` + `TypeAdapter` ظْ `NullSafeTypeAdapter.write`, `serialized` ظْ `write`). The closest match stays open instead of folding the exact body an agent needs to diagnose.
- **Stable fold ids.** Markers are unique across files (`fold_write_4_<tag>`), so a later `JsonWriter.write` cannot overwrite `TypeAdapter.write` in the session registry.
- **`expand_fold` accepts `query`.** `next_actions` already pass `query`; the tool now reads `fold_id`, `node_id`, or `query`, including the full `[neuromesh:fold:ظخ]` marker. Prefix lookup still finds the printed id.
- **Ranked `next_actions`.** Expand suggestions prefer high-scoring folds, not the first three in packet order.
- **Windowed packets.** Each file keeps at most K open bodies (seed `K=4`, optional `K=1`), ranked by task score. The skeleton emits imports, the enclosing type, those exons, and fold markers for sibling methods in the same type ظ¤ not the rest of the file.
- **Packet cap.** After skeletonization, balanced packets are capped at 12k tokens (6k / 24k for max_savings / max_quality). Optional files drop first; then seed K shrinks 4ظْ2. The top-scored method stays open. Fill caps stay 0 / 5k / 16k extra tokens.
- **Qualified symbol ids.** `NodeId` is `sym:{path}:{parent}.{name}` when the symbol has an enclosing type, so `TypeAdapter.write` and `NullSafeTypeAdapter.write` are distinct spans.

## 0.6.9 ظ¤ 2026-08-25

Compact incremental mesh, managed store, usage telemetry, and multi-client MCP connect.

- **No file bodies in the mesh.** File nodes keep path, hash, mtime, and token cost. Source is read on demand for skeletonization, `expand_fold`, and `neuromesh_get_file_skeleton`, so the in-RAM graph no longer holds N copies of the workspace.
- **Binary snapshot.** The persisted graph is `graph.bin` (bincode, bodies stripped). An existing `graph.json` is still read once for migration. Cold load on this repo is **28 ms** against a **346 ms** full index; a one-file reindex is **27 ms** (`docs/quality.md`).
- **Compact graph store.** Nodes and edges live in slot vectors with `u32` adjacency; `NodeId`/`EdgeId` are `Arc<str>`, so lookups, neighborhood walks, and Physarum tubes stop cloning whole maps. Spreading activation walks the adjacency arrays under one read lock, and ingesting a file takes a single write lock instead of one per symbol.
- **Prefix symbol index.** `search_symbols` resolves prefixes through a sorted name index instead of scanning every symbol name.
- **Real incremental index.** The walker compares size + mtime first and reads only changed files; `neuromesh index` reports `Unchanged skip`. Live sync (CLI `start`/`monitor`/`mcp` and the MCP handshake) uses an OS watcher (`notify`, 200 ms debounce) instead of a 150 ms full-tree poll. Hashing is now really Blake3.
- **Inbound relink.** Re-ingesting a file re-queues the inbound `Calls`/`Imports` edges that pointed at its old symbols, so callers no longer lose edges until the next full reindex.
- **`neuromesh usage`.** Print MCP token telemetry from `~/.neuromesh/telemetry_history.json` (`--all`, `--limit N`). The file is the source of truth so stats show even when the monitor is down. Duplicate `request_id`s are ignored; the monitor reloads the file on each usage fetch.
- **Managed store.** Graph, memory, and per-project config default to `~/.neuromesh/projects/<name>-<hash>/`. A workspace `.neuromesh` folder is not trusted. Opt in with `neuromesh store local` or `trust_local` in `~/.neuromesh/config.json`. Existing in-repo files are copied into the managed slot once, then ignored.
- **MCP clients.** `neuromesh connect` writes stdio configs (absolute binary + `NEUROMESH_WORKSPACE`) for Cursor, VS Code, Codex, Antigravity, Kilo Code, Trae, MiniMax, Claude, Windsurf, Cline/Roo. Handshake accepts Windows `file://` URIs, `prompt`/`task`/`input` tool args, and returns tool errors as `isError` so picky agents keep going.

## 0.6.3 ظ¤ 2026-08-25

Inbound throw edges for PHP rethrow and ternary `new Type`.

- **Inbound throws.** `throw $e` after `catch (Type $e)`, catch unions, and ternary `throw ظخ new Type` become inbound `Calls` edges. Symfony matchers throw `ResourceNotFoundException`, not `RouteNotFoundException` ظ¤ trace the type that is actually constructed or caught.

## 0.6.2 ظ¤ 2026-08-24

Scale search on large repos, auto index cap, and `--max-files`.

- **Scale search.** Exact class/interface names outrank fuzzy `Http`/`Kernel` tokens. `neuromesh_get_context` uses `coverage.claim = no_seed_resolved` when every identifier misses, and does not ship a utility fallback file.
- **Index file cap.** Default is **auto**: grow to every production source (then tests), ceiling 50,000. `neuromesh index --max-files 20000` (or `auto`) persists like `neuromesh port`. Env `NEUROMESH_MAX_FILES`. `index` / `doctor` print the applied cap and warn on truncation.

## 0.6.1 ظ¤ 2026-08-24

Language registry, tree-sitter queries, framework overlays, parallel index, and thinner packets.

- **MSRV.** Even/odd uses `seed & 1` instead of `u64::is_multiple_of` (Rust 1.87). Workspace `rust-version` is 1.80. `rust-toolchain.toml` pins stable + rustfmt/clippy. Ubuntu `apt` rustc 1.75 still cannot parse lockfile v4 ظ¤ use rustup.
- **`task` alias.** `neuromesh_get_context` accepts `task_description`, `prompt`, or `task`. An empty prompt is a JSON-RPC error, not a silent empty packet.
- **Generic languages.** PHP/Go/Java/Kotlin/C#/C/C++ extract functions and calls. `.kt` / `.kts` are indexed (`fun`, `object`, `data class`, imports). `throw new X`, `catch (X`, Kotlin `catch (e: X)`, and PHP `X $param` become inbound `Calls` edges.
- **Doctor skipped files.** `neuromesh doctor` (and `index`) report unsupported extensions so a Kotlin-only repo is no longer a silent empty scan.
- **Query extractors.** Rust and TypeScript parsing is driven by tree-sitter queries (`src/queries/*.scm`) behind a language registry. Regex remains the fallback. Gold on this repo must stay green.
- **Wave 3 framework overlays.** Android Activity/Compose/BroadcastReceiver, Spring mappings, Django `urls.py`, Next `app/` routes, Laravel `Route::`, Pinoox `action()`, Symfony `#[Route]`, WordPress REST, React/Vue/Svelte/Twig/Electron/Tauri/Vite/Prime UI become `Component`/`Api`/`Config` from layout and annotations ظ¤ no compiler. Stack facts come from manifests (`pinoox/pincore`, `react`, `vite`, Shopfa mentions). Gold: `mini-kotlin` ظ£How is a received SMS stored?ظإ, `mini-next`, `mini-pinoox`.
- **Index speed.** Workspace ingest parses files in parallel (rayon) and reuses a tree-sitter parser per thread. Hash skip is unchanged. `neuromesh index` uses the same ingest path as MCP.
- **Thinner packets.** Function spans follow the real tree-sitter body (Dart signature+body siblings, Kotlin `fun`, TS `const fn = () =>`). Folds replace the **body**, not the signature, so the file map stays; a parent that contains a seed exon is not folded. Fill caps are unchanged.
- **Wave 5 overlays.** Express `app.post`, Nest `@Controller`/`@Post`, Angular `@Component` + `path:`, Gin/Echo `.POST`, Axum `.route(..., post(`. Gold: `mini-express`, `mini-nest`, `mini-angular`, `mini-gin`, `mini-axum`. Prompt ظ£how does store use ظخظإ keeps the lowercase method name so Astro/Express pages seed.
- **Wave 6 overlays.** ASP.NET `MapPost`/`[HttpPost]` + Razor `@page`/`@code` (`.cshtml`/`.razor` indexed as HTML), SwiftUI `struct: View`, Remix `app/routes/` + React Router `createBrowserRouter`, Ktor `post("/sms")`. Gold: `mini-aspnet`, `mini-swiftui`, `mini-remix`, `mini-ktor`.
- **Stylesheets and SVG.** `.less` is indexed. CSS/SCSS extract class/id selectors and `--custom-properties` (SCSS still gets `$var` / `@mixin`; LESS gets `@var` and `.mixin()`). `.svg` uses the HTML extractor so `<symbol id>` / `id=` and `<use href="#ظخ">` become components. Gold: `mini-styles`.
- **Eval honesty.** `neuromesh eval` prints fixture dirs with an empty scan instead of skipping them. README numbers from release eval (2026-08-24): 219 files, 1,323 nodes, 2,891 edges, ~209 ms index.
- **Monitor galaxy.** 2D clicks open nodes (they used to pan instead). 3D picking chooses the front-most blob, pauses spin on hover, and ignores giant label hit-boxes. Nodes render as Physarum slime; **Play slime** grows, streams, and prunes tubes.
- **Monitor header.** Drop the extra Projects & Switch button ظ¤ click the active-project chip to open the switcher. Compact one-line labels.

## 0.5.2 ظ¤ 2026-08-23

Monitor port is a first-class CLI setting, not a hardcoded 8765.

- **`neuromesh port`.** Print the effective port, or persist it with `neuromesh port 9000` (managed project slot, or `<cwd>/.neuromesh` if trusted).
- **One-shot override.** `neuromesh monitor --port 9000` (`-p`, `--port=`) and the same flag on `start`. Env `NEUROMESH_PORT` wins over files.
- **Clients follow.** `doctor`, `connect`, and telemetry POST use the loaded host/port. VS Code / Cursor still uses Settings ظْ `neuromesh.port` ظ¤ keep it in sync.

## 0.5.1 ظ¤ 2026-08-23

Accuracy first, then faster index, then a thinner default packet.

- **Seed ranking.** `search_symbols` and `pick_dominant_candidate` prefer exact case, Class/Function/Component, and a path that repeats the symbol name (`Searcher` ظْ `searcher/mod.rs`). Test/bench/example paths are penalized so a lowercase field twin does not steal the seed.
- **Hybrid resolve.** After `resolve_ranked`, activate also checks a high-score search hit. If case or path agrees and the ids differ, the search hit becomes the seed ظ¤ so a confident but wrong ranked pick no longer ships a thin, wrong packet.
- **Index skip.** Walker ignores `benches`, `examples`, `testdata`, and extra caches (`.tox`, `.mypy_cache`, `.pytest_cache`). `tests/` stays indexed; fill still treats test/bench/example as noise.
- **Balanced fill.** Extra connector cap is 5,000 tokens (was 8,000). Gold on this repo still passes.
- **Seed callees stay exons.** Functions the seed actually calls keep their bodies; their files are required so the answer is not folded away.
- **MCP handshake.** Stdio initialize no longer hangs, so the monitor and Cursor can start the session.

## 0.5.0 ظ¤ 2026-08-23

The agent loop is real: **get_context ظْ expand_fold**, Grep only when coverage is `partial`.

- **Folds.** Skeletonization registers each `[neuromesh:fold]` body. `neuromesh_expand_fold` restores it by `fold_id` from the registry (no disk re-read).
- **Smarter fill.** Soft crate caps, giant files skeletonized instead of dropped, unresolved-call closers scored. Each callee file is scored once so a large `match` does not drown the packet. Seed callees stay exons so the function that answers the question is not folded.
- **Packets.** Every file includes `path`, `why`, `line_range`, `folded_symbols`, and `seed_call_coverage`.
- **Parse.** tree-sitter for Rust and TypeScript behind the same `AstAnalysisResult`; regex remains the fallback. Impl- and field-aware resolve (`self.activator.activate` ظْ `ContextActivator::activate`). Ambiguous calls stay `Likely` instead of vanishing.
- **Skeletonizer** prefers parser/graph function spans over brace counting.
- **Gold.** Path-qualified files, five fixture repos under `tests/fixtures/`, recall ظëح 0.8 **and** precision ظëح 0.4. `neuromesh eval` prints workspace / selected / packet tokens, reduction, and Grep-still-needed.
- **Hot path.** Neighborhood Physarum tubes after two+ seeds (skip huge subgraphs; stats `active` only when used). Selector reads pheromone. Folds persist for the MCP session. Mycelium prefetches packet neighbors.

## 0.4.0

Seed-then-fill packets. Seeds always ship; connectors fill under a real extra-token cap (`max_savings` 0 ┬╖ `balanced` 8k ┬╖ `max_quality` 16k). Coverage claims (`no_recorded_gap` / `partial`). QualityGate honors the requested mode unless the task is critical.

## 0.3.0

Two-pass structural graph: extract symbols, imports, and scoped calls, then unique-resolve edges after every file exists. Ranked search. Safe workspace discovery (git/Cargo root; refuse home and drive roots).
