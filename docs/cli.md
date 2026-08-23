# CLI

Binary: `neuromesh` (`neuromesh-cli` crate).

```
neuromesh mcp          # stdio MCP server (what the IDE launches)
neuromesh monitor      # Web UI + SSE on http://127.0.0.1:8765
neuromesh index        # build the graph and seed project memory
neuromesh status       # node / edge counts
neuromesh graph        # graph stats
neuromesh memory       # project facts
neuromesh optimize     # one prompt → print the packet
neuromesh eval         # gold recall / precision / fill on cwd and tests/fixtures
neuromesh benchmark    # same as eval
neuromesh connect      # ready-to-paste MCP JSON
neuromesh doctor       # workspace root, scan, persisted graph, port
neuromesh version
```

## Everyday

```bash
neuromesh doctor
neuromesh index
neuromesh optimize -- "How does handle_tool_call extract intent?"
neuromesh eval
```

`eval` scores `tests/gold_tasks.toml` if present, otherwise the builtin set. It also walks `tests/fixtures/*/gold_tasks.toml`.

The process uses the **current working directory** as the project. Point your MCP config at a command that starts in the repo you care about.
