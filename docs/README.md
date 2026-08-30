# NeuroMesh docs

Start with the [root README](../README.md) if you want install and the agent loop.

| Doc | For |
| :--- | :--- |
| [Living systems](nature.md) | Physarum, STDP, exons, osmosis — mapped to crates |
| [Architecture](architecture.md) | Pipeline, tiered retrieval, crate map, runtime guarantees |
| [Graph proxy](graph-proxy.md) | Optional CBM/Graphify backend via MCP stdio |
| [Engines](engines.md) | Graph backend + seed engine — CLI, monitor, recipes |
| [MCP tools](mcp.md) | What each tool returns and how the agent should call them |
| [Agent guide](agent-guide.md) | Teach every IDE to prefer NeuroMesh (rules, AGENTS.md, smoke test) |
| [Agent rule](agent-rule.mdc) | Cursor-ready `.mdc` template (same body as the guide) |
| [CLI](cli.md) | Commands you run in a terminal |
| [Quality](quality.md) | Gold harness, `neuromesh eval`, release gates, measured numbers |
| [HTTP monitor](api.md) | Local UI, SSE, management endpoints |
| [Contributing](contributing.md) | Tests, clippy, adding a language |
| [Changelog](CHANGELOG.md) | Version history |

## Measured snapshot (v0.8.2)

From `cargo run --release -p neuromesh-cli -- eval` on this repository. Full tables and gates: [quality.md](quality.md).

| Metric | Value |
| :--- | ---: |
| Files · nodes · edges | 340 · 3,161 · 6,795 |
| Workspace tokens | 650,859 |
| Index (release) | 552 ms |
| Index (debug) | ~2.3 s |
| Snapshot cold load (release) | 55 ms |

| Task (balanced) | vs WS | Recall | Grep | ms |
| :--- | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | 97.3% | 1.00 | **0** | 22 |
| `physarum_usage` | 99.4% | 1.00 | **0** | 12 |

**Multilingual Express benchmark** (v0.8.2, 60-cell holdout): native assisted recall **0.426**, precision **0.789**, warm p50 **~35 ms**; proxy_cbm assisted recall **0.578**, precision **0.504**, warm p50 **~230 ms**. Native remains default. Details: [quality.md](quality.md).

Tiered retrieval: most queries stay **L1**; L2/L3 only on critical gaps. Release gates: `neuromesh eval --release-gates`. Learning dose-response: `neuromesh eval --learning`.

NeuroMesh is a **context engine**: one primary call (`get_context_packet`) returns a task-conditioned evidence packet with optional `retrieval` metadata. The biology (slime mold, synapses, gene splice) is the design language — see [nature.md](nature.md).
