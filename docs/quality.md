# Quality

Claims in this project are supposed to come from commands you can run, not from a padded corpus.

## Gold

`tests/gold_tasks.toml` lists prompts and **path-qualified** gold files. Fixture repos live in `tests/fixtures/` (router, TS store, session, queue, string config, Python SMS, Kotlin SMS including a natural “received SMS stored” prompt, Dart SMS, C# SMS, Next SMS route, Pinoox `action()` SMS), including edit/refactor-style prompts — not only “where is this symbol”.

Thresholds locked in tests:

- recall ≥ **0.8**
- precision ≥ **0.4**
- missed seeds reported as `partial`, not an empty silent packet
- `expand_fold` restores a registered body without reading the disk
- activation under **150 ms** in the debug gold test on this repo (cargo test is parallel; isolated runs sit nearer 60 ms)

```bash
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
cargo test -p neuromesh-context gold_harness_on_fixture_repos -- --nocapture
neuromesh eval
```

`neuromesh eval` prints **workspace / selected / packet** tokens, reduction vs both, recall, precision, **Grep still needed**, and latency. README numbers must come from that table — not from a padded corpus or a global 99% claim.

## Grep after get_context

From `neuromesh eval` on this workspace (debug, 2026-08-23, balanced):

| Task | WS tok | Selected | Packet | vs WS | vs selected | Recall | Prec | Grep | ms |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | 268124 | 26017 | 17422 | 93.5% | 33.0% | 1.00 | 0.50 | **0** | 24 |
| `physarum_usage` | 268124 | 7882 | 4476 | 98.3% | 43.2% | 1.00 | 0.50 | **0** | 19 |

That is “did the packet already hold the files a developer would open”, not a live multi-agent trial. Quote this table; do not invent a global 99% figure.

## Index snapshot

From `cargo test -p neuromesh-graph indexes_real_neuromesh_repo_with_usable_graph -- --nocapture` on this repository:

| Metric | Value |
| :--- | ---: |
| Files | 159 (`target/` ignored) |
| Nodes | 972 |
| Edges | 2,001 |
| Resolved calls | 558 |
| Resolved imports | 571 |
| `search_symbols("handle_tool_call")` | &lt; 1 ms, exact hit |
| Neighbors of `handle_tool_call` | 28 |
| Index time (debug) | ~1,202 ms |

Fill caps: `max_savings` = 0 extra tokens, `balanced` = 5,000, `max_quality` = 16,000. Reduction is versus **this workspace**, not a fake 25k dump.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
