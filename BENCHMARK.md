# NeuroMesh measurements

This file used to claim a universal **99.6%** token reduction and synthetic dollar savings. Those numbers were not produced by this engine.

Measure the current workspace instead:

```bash
neuromesh eval
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
cargo test -p neuromesh-context gold_harness_on_fixture_repos -- --nocapture
cargo test -p neuromesh-context live_v04_measurement -- --ignored --nocapture
```

`eval` indexes the working directory and scores gold tasks (`tests/gold_tasks.toml`, or the builtin set) under the real fill caps. It also walks `tests/fixtures/` when that directory exists.

| Mode | Extra tokens on top of seed files |
| :--- | ---: |
| `max_savings` | 0 |
| `balanced` | 8,000 |
| `max_quality` | 16,000 |

Gold files are **path-qualified**. Recall ≥ 0.8 and precision ≥ 0.4 are the locked thresholds. Reduction is reported against **this workspace's token count**, not a padded dump-all baseline.

On two prompts in this repo (`handle_tool_call extract intent`, `Where is Physarum used?`) the gold files are already in the packet (recall 1.0), so Grep is not required to find them after `get_context`.

See [README.md](README.md#measured-quality) for the last measured index of this repository.
