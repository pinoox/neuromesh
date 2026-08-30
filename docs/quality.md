# Quality

Claims in this project come from commands you can run, not from a padded corpus.

---

## CI verification (2026-08-30)

Full workspace gate (matches [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)):

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo test -p neuromesh-context --features embeddings
cargo test -p neuromesh-embed
cargo test -p neuromesh-graph-proxy
```

| Gate | Result |
| :--- | :--- |
| `cargo fmt --check` | pass |
| `cargo clippy -D warnings` | pass (single `neuromesh` bin — no duplicate `nmx` compile) |
| `cargo test --workspace --features embeddings` | **434 passed**, 1 ignored |
| Wall time (embeddings workspace, debug, Windows) | ~3 min |

Tests by crate (embeddings workspace run):

| Crate | Passed | Ignored |
| :--- | ---: | ---: |
| neuromesh-context | 122 | 1 |
| neuromesh-graph | 98 | 0 |
| neuromesh-mcp | 37 | 0 |
| neuromesh-parser | 54 | 0 |
| neuromesh-core | 18 | 0 |
| neuromesh-cli | 18 | 0 |
| neuromesh-task | 19 | 0 |
| other crates | 86 | 0 |

The ignored test is `gold_harness_on_neuromesh_repo` on platforms where the full-repo gold path is skipped.

**CLI:** one Cargo binary (`neuromesh`); `nmx` is a release/install alias — see [cli.md](cli.md#build-from-source).

---

## Gold harness

Built-in and project-local **gold tasks** pair natural-language prompts with **path-qualified** expected files. Fixture workspaces cover representative stacks and task shapes:

- Web: Vue/React, auth guards, SCSS, checkout flows, dead-code detection
- Backend: Laravel, FastAPI, Rails, Nest, Gin, Axum, ASP.NET, Ktor, Express
- Mobile / cross-platform: Flutter, SwiftUI, Next.js
- Config & schema: JSON/env, SQL, Zod-like cores with decoys
- Refactor / edit-style prompts alongside trace-and-explain tasks

Thresholds locked in CI:

- recall ≥ **0.8**
- precision ≥ **0.4** (forbidden gold file in packet → precision 0)
- missed seeds → `partial` or `no_seed_resolved` when every identifier missed
- `expand_fold` restores a registered body without disk read
- activation under **250 ms** in debug gold test (non-Windows CI)
- skeletonizer folds bodies; fill caps 0 / 5k / 16k; packet caps 6k / 12k / 24k
- graph stores **no file bodies** — source read on demand
- snapshot cold load and one-file reindex ≤ full workspace index time

```bash
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
cargo test -p neuromesh-context gold_harness_on_fixture_repos -- --nocapture
neuromesh eval
```

Fixture smoke: `tests/fixtures/mini-fastify/` (validation, plugin-utils, content-type-parser).

---

## Tiered retrieval & release gates

MCP and `neuromesh packet --json` use **single-pass** L1→L2→L3 escalation.

| Engine | Index | Query default | L3 / ONNX |
| :--- | :--- | :--- | :--- |
| **`fast`** | graph only (~763 ms cold) | server-assisted keywords + graph | lazy file-tier sidecar on first weak-lexical L3; `ort_session_active=false` when L3 never fires |
| **`hybrid`** | graph + hierarchical v6 sidecar | prompt-only MiniLM | file ANN → lazy symbols |
| **`deep`** | graph + flat symbol sidecar | prompt-only MiniLM | full symbol ANN |

```bash
neuromesh eval --release-gates
neuromesh eval --release-gates --engine fast
neuromesh eval --release-gates --engine hybrid
neuromesh eval --release-gates --calibrate
```

### Built-in gold gates (`evaluate_fast` / `evaluate_hybrid`)

| Gate | `fast` | `hybrid` / `deep` |
| :--- | :--- | :--- |
| Assisted recall | ≥ 55% | ≥ 55% |
| Precision | ≥ 73% | ≥ 73% |
| no_seed cells | ≤ 2 | ≤ 2 |
| embedding_primary rate | ≤ 10% | ≥ 40% |
| L3 rate | ≤ 20% | ≤ 15% |
| FSR proxy | < 10% | < 10% |
| Full-workspace fallback | 0 | 0 |

MCP telemetry: `retrieval.embedding_used`, `resolution_tier`, `ort_session_active`.

---

## Fastify external benchmark (test6)

Holdout: **60 cells** (8 questions × 5 languages: en/es/fa/de/zh) on a Fastify clone. MCP stdio `get_context_packet`, **prompt only**.

**Last full re-run:** 2026-08-30 (phase 2 retrieval; release v0.9.0). Artifacts: `C:\projects\benchmark\nm_vs_cbm\test6\team\`.

Corpus: 361 files · 1,318 graph nodes · ~882K workspace tokens.

### Measured results (phase 2)

| Engine | MCP recall | MCP precision | RAM idle | MCP warm p50 | Cold index |
| :--- | ---: | ---: | ---: | ---: | ---: |
| **fast** | **56.7%** (34/60) | **17.4%** | **19 MB** | **54 ms** | **763 ms** |
| hybrid | 58.3% (35/60) | 16.1% | ~630 MB | 31 ms | ~94 s |
| deep | 60.0% (36/60) | 17.1% | ~631 MB | 36 ms | ~58 s |

Per-language MCP recall:

| Lang | fast | hybrid | deep |
| :--- | ---: | ---: | ---: |
| en | 66.7% | 66.7% | 66.7% |
| es | 58.3% | 58.3% | 58.3% |
| fa | 41.7% | 41.7% | 50.0% |
| de | 66.7% | 66.7% | 75.0% |
| zh | 58.3% | 58.3% | 58.3% |

No `no_seed_resolved` in any engine. Fast: zero-embed path on all cells in that run.

> **Phase 2.1:** fast index is instant again (no ONNX at index); L3 builds sidecar lazily. Re-run test6 after shipping 2.1.

### Holdout gates (`ReleaseGateReport::evaluate_fastify_holdout`)

| Gate | `fast` | `hybrid` | `deep` |
| :--- | :--- | :--- | :--- |
| Recall | ≥ **57%** | ≥ **60%** | ≥ **62%** |
| MCP precision | ≥ **15%** | ≥ **15%** | ≥ **15%** |
| no_seed | ≤ **1** | **0** | **0** |
| embedding_primary | ≤ 10% | ≥ 40% | ≥ 40% |
| Full-workspace fallback | 0 | 0 | 0 |

```bash
# External driver (release binary + embed rebuild per engine)
cd C:\projects\benchmark\nm_vs_cbm\test6
node nm_mcp_recall.mjs <release-neuromesh> <fastify-workspace> team/nm_mcp_<engine>_recall.json
```

---

## Historical: Express 60-cell (v0.8.x)

| mode | recall | precision | no_seed | warm p50 |
| :--- | ---: | ---: | ---: | ---: |
| native raw + server auto-extract (v0.8.3) | **0.460** | **0.811** | 0/60 | ~34 ms |
| native assisted (v0.8.2) | 0.431 | 0.790 | 0/60 | ~35 ms |
| native raw (v0.8.2) | 0.333 | 0.622 | 11/60 | ~48 ms |

Re-run: `node test3/mcp_driver_v2.mjs <release-neuromesh> <express-workspace> <outdir> 6 raw native`

---

## Grep after get_context

From `neuromesh eval` on the NeuroMesh workspace (balanced, release):

| Task | WS tok | Selected | Packet | vs WS | vs selected | Recall | Prec | Grep | ms |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | 650859 | 72428 | 17389 | 97.3% | 76.0% | 1.00 | 0.75 | **0** | **22** |
| `physarum_usage` | 650859 | 19625 | 4080 | 99.4% | 79.2% | 1.00 | 0.50 | **0** | **12** |

Debug eval: activation ~157 ms / ~104 ms; full index ~2.3 s.

---

## Hot-path optimization (v0.7.17)

| Task | Before ms | After ms |
| :--- | ---: | ---: |
| `handle_tool_call_intent` | 44 | **22** |
| `physarum_usage` | 22 | **12** |

MCP warm p50 (5 prompts): Express ~140 ms; NeuroMesh repo **487 ms**.

---

## Learning → emission (v0.7.17)

Agents must call `neuromesh_record_feedback` after a successful edit.

```bash
cargo test -p neuromesh-context learning_to_emission
cargo test -p neuromesh-context reinforced_file_promotes
cargo test -p neuromesh-context learning_does_not_leak
cargo test -p neuromesh-context deterministic_packet_same_state
cargo test -p neuromesh-context catastrophic_learning
```

---

## Shop-style fixture (Vue + Pinia + SCSS)

| Task | Recall | Prec | Packet files (gold hit) |
| :--- | ---: | ---: | :--- |
| `price_card_scss` | 1.00 | 1.00 | tokens, mixins, ProductCard, price-card SCSS |
| `dead_code_gocart` | 1.00 | 1.00 | ui.js, App.vue, CartDrawer, Header |
| `checkout_qty_stepper` | 1.00 | 0.40 | CheckoutView, cart store (+ connector views) |

---

## Index snapshot (NeuroMesh workspace)

| Metric | Value |
| :--- | ---: |
| Files | 340 |
| Nodes | 3,161 |
| Edges | 6,795 |
| Workspace tokens | 650,859 |
| **Index time (release)** | **552 ms** |
| Index time (debug) | ~2.3 s |

Index file cap: **auto** (ceiling 50,000). Override: `neuromesh index --max-files N`.

---

## Compact mesh: snapshot load and one-file reindex

| Metric | Value |
| :--- | ---: |
| Snapshot size | 3.9 MB |
| Full workspace index | 790 ms |
| **Snapshot cold load** | **55 ms** |
| **One-file reindex** | **80 ms** |

```bash
cargo test --release -p neuromesh-graph --lib snapshot_load_and_single_file_reindex -- --nocapture
```

Fill caps: `max_savings` = 0, `balanced` = 5,000, `max_quality` = 16,000. Packet caps: 6k / 12k / 24k.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
