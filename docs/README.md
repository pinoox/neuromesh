# NeuroMesh docs

Start with the [README](../README.md) for install and the agent loop.

| Doc | For |
| :--- | :--- |
| [Agent guide](agent-guide.md) | Teach every IDE to prefer NeuroMesh (rules, AGENTS.md, smoke test) |
| [MCP tools](mcp.md) | What each tool returns and how the agent should call them |
| [CLI](cli.md) | Commands you run in a terminal |
| [Configuration](configuration.md) | Engine presets, proxy, env vars, monitor, file cap, build from source |
| [Engines](engines.md) | Quick overview of `fast` / `hybrid` / `deep` |
| [Architecture](architecture.md) | Pipeline, tiered retrieval, crate map |
| [Quality](quality.md) | Gold harness, `neuromesh eval`, release gates, measured numbers |
| [Graph proxy](graph-proxy.md) | Optional CBM backend via MCP stdio |
| [HTTP monitor](api.md) | Local UI, SSE, management endpoints |
| [Living systems](nature.md) | Physarum, STDP, exons — mapped to crates |
| [Agent rule](agent-rule.mdc) | Cursor-ready `.mdc` template |
| [Contributing](contributing.md) | Tests, clippy, adding a language |
| [Changelog](CHANGELOG.md) | Version history |

## v0.9.0 in one line

**`get_context_packet`** with default **`engine: fast`** — graph routing, prompt only, folded evidence packet. Opt in to **`hybrid`** / **`deep`** after `neuromesh install embed minilm`. Details: [configuration.md](configuration.md) · [quality.md](quality.md).
