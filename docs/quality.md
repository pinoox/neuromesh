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

Fill caps: `max_savings` = 0 extra tokens, `balanced` = 5,000, `max_quality` = 16,000. Reduction is versus **this workspace**, not a fake 25k dump.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
