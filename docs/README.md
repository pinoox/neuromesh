# NeuroMesh docs

Start with the [root README](../README.md) if you want install and the agent loop.

| Doc | For |
| :--- | :--- |
| [Living systems](nature.md) | Physarum, STDP, exons, osmosis — mapped to crates |
| [Architecture](architecture.md) | Pipeline, tiered retrieval, crate map, runtime guarantees |
| [Graph proxy](graph-proxy.md) | Optional CBM/Graphify backend via MCP stdio |
| [Engines](engines.md) | Default embed (bundled MiniLM) + custom seed engines |
| [MCP tools](mcp.md) | What each tool returns and how the agent should call them |
| [Agent guide](agent-guide.md) | Teach every IDE to prefer NeuroMesh (rules, AGENTS.md, smoke test) |
| [Agent rule](agent-rule.mdc) | Cursor-ready `.mdc` template (same body as the guide) |
| [CLI](cli.md) | Commands you run in a terminal |
| [Quality](quality.md) | Gold harness, `neuromesh eval`, release gates, measured numbers |
| [HTTP monitor](api.md) | Local UI, SSE, management endpoints |
| [Contributing](contributing.md) | Tests, clippy, adding a language |
| [Changelog](CHANGELOG.md) | Version history |

## Measured snapshot (v0.8.6)

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

**Multilingual Express benchmark** (v0.8.6, 60-cell holdout): embedding-primary MiniLM recall target **≥ 0.460**, precision **≥ 0.80**, warm p50 **~10–30 ms**, **0/60 no_seed**. Details: [quality.md](quality.md).

Tiered retrieval: most queries stay **L1**; L2/L3 only on critical gaps. Release gates: `neuromesh eval --release-gates`. Learning dose-response: `neuromesh eval --learning`.

NeuroMesh is a **context engine** (v0.8.6): **`get_context_packet`** with bundled **MiniLM embed** — prompt only, folded evidence packet, optional `retrieval` metadata. Biology metaphors: [nature.md](nature.md).
