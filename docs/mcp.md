# MCP tools

Transport: **stdio JSON-RPC** (`neuromesh mcp`). That is what Cursor, Claude, Codex, Antigravity, Kilo Code, Trae, Cline, and similar clients launch. Stdio has **no TCP port** — `--port` on `mcp` does nothing. Background index uses the same file-cap rules as `neuromesh index` (`--max-files`, `NEUROMESH_MAX_FILES`, project-slot `config.json`; default auto, ceiling 50,000). See [cli.md](cli.md#index-file-cap).

Remote / multi-agent: `neuromesh monitor` (optionally `--port 9000` / `neuromesh port 9000`), then SSE and HTTP as in [api.md](api.md).

## Connect

```bash
neuromesh connect           # write project MCP configs + globals for installed apps
neuromesh connect --print   # snippets only
neuromesh connect --project # this repo only
neuromesh connect --global  # also create user-level files
```

The command points each client at **this binary** (absolute path) and the current workspace (`args: ["mcp", "<workspace>"]` plus `NEUROMESH_WORKSPACE`). PATH is not required. It merges a `neuromesh` server into existing files and does not delete other MCP servers.

| Client | Project file | User file (if the app is installed) |
| :--- | :--- | :--- |
| Cursor | `.cursor/mcp.json` | `~/.cursor/mcp.json` |
| VS Code / Copilot | `.vscode/mcp.json` (`servers`) | — |
| Codex | `.codex/config.toml` | `~/.codex/config.toml` |
| [Antigravity](https://antigravity.google/) | `.agents/mcp_config.json` | `~/.gemini/config/mcp_config.json` |
| Kilo Code | `.kilo/kilo.jsonc` (`mcp` + command array) | `kilo.jsonc` in the user config dir |
| Trae | `.trae/mcp.json` | `Trae/User/mcp.json` |
| MiniMax Code | `.minimax/mcp.json` | same `mcpServers` snippet as Cursor |
| Claude Desktop / Code | `.mcp.json` | `claude_desktop_config.json` |
| Windsurf / Cline / Roo | — | their existing MCP settings files |

Stdout is JSON-RPC only (NDJSON). Handshake logs go to stderr so Antigravity and other strict hosts stay happy.

## Agent loop

```
neuromesh_get_context(task_description)
  → if a folded body is required: neuromesh_expand_fold(fold_id)
  → if you need callers: neuromesh_trace
  → after a successful edit: neuromesh_record_feedback
```

Start with `get_context`. `next_actions` say when to `neuromesh_expand_fold`. Grep (`neuromesh_search_symbols`) when `coverage.claim` is `partial` **or** `no_seed_resolved` (zero identifiers resolved — do not treat a utility fallback file as the answer). After a good edit, **always** call `neuromesh_record_feedback` — that is the synaptic learning step; without it the next packet does not change.

## Tools

| Tool | Input | Returns |
| :--- | :--- | :--- |
| `neuromesh_get_context` | `task_description`, `prompt`, or `task`; optional `mode` | Evidence packet |
| `neuromesh_expand_fold` | `fold_id`, `node_id`, or `query` (the field `next_actions` uses); optional `reason` | Original folded body |
| `neuromesh_get_file_skeleton` | `file_path`, optional `active_symbols` | One skeletonized file |
| `neuromesh_search_symbols` | `query`, optional `limit` | Ranked hits |
| `neuromesh_get_dependencies` | name or path | Typed neighbors |
| `neuromesh_trace` | `query`, `direction` (`in` / `out` / `both`), `depth` | Call/import chains. Inbound includes `throw new`, `throw $e` after `catch (Type $e)`, and ternary `new Type`. Trace the exception that is actually thrown. |
| `neuromesh_analyze_impact` | `query`, `depth` | Blast radius |
| `neuromesh_get_architecture` | — | Languages, packages, entry points |
| `neuromesh_get_project_memory` | — | Seeded facts |
| `neuromesh_record_feedback` | `task_success`, `touched_nodes` | STDP on that path |
| `neuromesh_get_stats` | — | Node/edge counts |

Aliases exist for older clients (`activate_context`, `expand_context`, `search_context`). Prefer the `neuromesh_*` names.

## Evidence packet

`neuromesh_get_context` is the product. Typical shape:

- `task` — intent, identifiers, file hints
- `evidence_packet.files[]` — `path`, skeleton, `tokens`, `why`, `line_range`, `folded_symbols`
- `evidence_packet.symbols[]` — name, path, signature, score
- `seeds` — what resolved, what missed
- `coverage` — `no_recorded_gap` or `partial`
- `budget` — `seed_tokens`, `fill_used`, `fill_cap`, `mode`
- `seed_call_coverage` — fraction of seed call targets present in the packet
- `next_actions` — `expand_fold` for sleeping exons; Grep/search only when `coverage` is `partial`
- `physarum_used` / `physarum_ms` / `selection_method` — honest slime-mold telemetry

`mode`: `balanced` (default, +5,000 fill), `max_savings` (0), `max_quality` (+16,000). Critical tasks (auth / payment / secret) upgrade to max quality.

## Folds

Markers look like:

```
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```

Pass that `fold_id` to `neuromesh_expand_fold` as `fold_id`, `node_id`, or `query` (the last is what `next_actions` send). The full marker line also works. Folds persist for the **MCP session** (same process, same project). They are not cleared on every `get_context`. A new project id wipes the registry. Ids include a short path tag so two files that both fold `write` do not collide.
