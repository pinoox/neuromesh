# Quality

Claims in this project are supposed to come from commands you can run, not from a padded corpus.

## Gold

`tests/gold_tasks.toml` lists prompts and **path-qualified** gold files. Fixture repos live in `tests/fixtures/` (router, TS store, session, queue, string config, Python SMS, Kotlin SMS including a natural “received SMS stored” prompt, Dart SMS + Flutter widget, C# SMS, Next SMS route, Pinoox `action()` SMS, FastAPI, Rails, Astro, Express, Nest, Angular, Gin, Axum, ASP.NET MapPost + Razor `@page`, SwiftUI, Remix/React Router, Ktor, LESS badge token + SVG `smsInbox` icon), including edit/refactor-style prompts — not only “where is this symbol”.

Thresholds locked in tests:

- recall ≥ **0.8**
- precision ≥ **0.4**
- missed seeds reported as `partial`, or `no_seed_resolved` when **every** identifier missed (empty packet, Grep immediately)
- `expand_fold` restores a registered body without reading the disk
- activation under **150 ms** in the debug gold test on this repo (cargo test is parallel; isolated runs sit nearer 60 ms)
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

From `neuromesh eval` on this workspace (release, 2026-08-24, balanced):

| Task | WS tok | Selected | Packet | vs WS | vs selected | Recall | Prec | Grep | ms |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | 365352 | 32889 | 27381 | 92.5% | 16.7% | 1.00 | 0.60 | **0** | 17 |
| `physarum_usage` | 365352 | 7929 | 4547 | 98.8% | 42.7% | 1.00 | 0.67 | **0** | 8 |

That is “did the packet already hold the files a developer would open”, not a live multi-agent trial. Quote this table; do not invent a global 99% figure.

## Index snapshot

From `neuromesh eval` (release, 2026-08-24) on this repository:

| Metric | Value |
| :--- | ---: |
| Files | 219 (`target/` ignored) |
| Nodes | 1,323 |
| Edges | 2,891 |
| Index time (release) | ~209 ms |

Index file cap is **auto** by default (production sources first, tests last, ceiling 50,000). Override with `neuromesh index --max-files N`. See [cli.md](cli.md#index-file-cap).

## Compact mesh: snapshot load and one-file reindex

The mesh keeps a structural skeleton in RAM. File bodies are not stored in nodes and not written to the snapshot; source is read on demand when a packet is spliced. The snapshot is `graph.bin` (bincode); `graph.json` is still read once for migration.

From `snapshot_load_and_single_file_reindex_beat_full_index` (release, this repo):

| Metric | Value |
| :--- | ---: |
| Files scanned | 247 |
| Nodes | 1,733 |
| Full workspace index | 346 ms |
| Snapshot size | 2.2 MB |
| **Snapshot cold load** | **28 ms** |
| **One-file reindex** (parse + local relink) | **27 ms** |

```bash
cargo test --release -p neuromesh-graph --lib snapshot_load_and_single_file_reindex -- --nocapture
```

The walker compares size + mtime before reading, so an unchanged tree is a metadata walk with zero `read_to_string` calls (`metadata_walk_skips_unchanged_files`). `neuromesh index` prints `Unchanged skip` for those files. Live sync uses an OS watcher (`notify`, 200 ms debounce) instead of a full-tree poll.

Re-ingesting one file re-queues the **inbound** `Calls`/`Imports` edges that pointed at its symbols, so callers keep their edges without a full reindex (`reingest_file_relinks_inbound_calls`).

Fill caps: `max_savings` = 0 extra tokens, `balanced` = 5,000, `max_quality` = 16,000. Packet caps after skeletonization: 6,000 / 12,000 / 24,000. Reduction is versus **this workspace**, not a fake 25k dump.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
