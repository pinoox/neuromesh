# NeuroMesh docs

Start with the [root README](../README.md) if you want install and the agent loop.

| Doc | For |
| :--- | :--- |
| [Living systems](nature.md) | Physarum, STDP, exons, osmosis — mapped to crates |
| [Architecture](architecture.md) | Pipeline, crate map, runtime guarantees |
| [MCP tools](mcp.md) | What each tool returns and how the agent should call them |
| [CLI](cli.md) | Commands you run in a terminal |
| [Quality](quality.md) | Gold harness, `neuromesh eval`, measured numbers |
| [HTTP monitor](api.md) | Local UI, SSE, management endpoints |
| [Contributing](contributing.md) | Tests, clippy, adding a language |
| [Changelog](CHANGELOG.md) | Version history |

NeuroMesh is a **context engine**: one primary call (`neuromesh_get_context`) returns a task-conditioned evidence packet. The biology (slime mold, synapses, gene splice) is the design language — see [nature.md](nature.md).
