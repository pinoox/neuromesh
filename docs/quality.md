# Quality

Claims in this project are supposed to come from commands you can run, not from a padded corpus.

## Gold

`tests/gold_tasks.toml` lists prompts and **path-qualified** gold files. Five small fixture repos live in `tests/fixtures/` (router, TS store, session, queue, string config), including edit/refactor-style prompts — not only “where is this symbol”.

Thresholds locked in tests:

- recall ≥ **0.8**
- precision ≥ **0.4**
- missed seeds reported as `partial`, not an empty silent packet
- `expand_fold` restores a registered body without reading the disk
- activation under **50 ms** in the debug gold test on this repo

```bash
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
cargo test -p neuromesh-context gold_harness_on_fixture_repos -- --nocapture
neuromesh eval
```

## Grep after get_context

On this workspace (debug, 2026-08-23), two real prompts already contained their gold files in the packet:

| Prompt | Recall | Grep still needed |
| :--- | ---: | ---: |
| How does `handle_tool_call` extract intent? | 1.0 | **0** |
| Where is Physarum used? | 1.0 | **0** |

That is “did the packet already hold the files a developer would open”, not a live multi-agent trial.

## Index snapshot

From `cargo test -p neuromesh-graph indexes_real_neuromesh_repo_with_usable_graph -- --nocapture` on this repository:

| Metric | Value |
| :--- | ---: |
| Files | 156 (`target/` ignored) |
| Nodes | 956 |
| Edges | 1,958 |
| Resolved calls | 558 |
| Resolved imports | 571 |
| `search_symbols("handle_tool_call")` | &lt; 1 ms, exact hit |
| Neighbors of `handle_tool_call` | 28 |
| Index time (debug) | ~1,202 ms |

Fill caps: `max_savings` = 0 extra tokens, `balanced` = 8,000, `max_quality` = 16,000. Reduction is versus **this workspace**, not a fake 25k dump.

Token savings from skeletonization are **per file and per task**. There is no universal 99% claim.
