# MCP tools

Transport: **stdio JSON-RPC** (`neuromesh mcp <workspace>`). That is what Cursor, Claude, Codex, OpenCode, MiMo CLI, Antigravity, Kilo Code, Trae, Cline, and similar clients launch. Stdio has **no TCP port** — `--port` on `mcp` does nothing. Background index uses the same file-cap rules as `neuromesh index` (`--max-files`, `NEUROMESH_MAX_FILES`, project-slot `config.json`; default auto, ceiling 50,000). See [cli.md](cli.md#index-file-cap).

**Warning:** Never run `neuromesh mcp` without a workspace path (or `NEUROMESH_WORKSPACE`). Without it the server may bind to your home directory and index unrelated projects. Prefer `neuromesh connect`, which pins an absolute binary path and workspace in MCP config.

Remote / multi-agent: `neuromesh monitor` (optionally `--port 9000` / `neuromesh port 9000`), then SSE and HTTP as in [api.md](api.md).

## Connect

```bash
neuromesh connect           # write project MCP configs + globals for installed apps
neuromesh connect --print   # snippets only
neuromesh connect --project # this repo only
neuromesh connect --global  # also create user-level files
```

The command points each client at **this binary** (absolute path) and the current workspace (`args: ["mcp", "<workspace>"]` plus `NEUROMESH_WORKSPACE`). PATH is not required. It merges a `neuromesh` server into existing files and does not delete other MCP servers.

### Manual

When `neuromesh` is on **PATH** and the IDE runs MCP from your project root (or you set `NEUROMESH_WORKSPACE`), paste this into MCP settings — no `neuromesh connect` required.

**Cursor** — `.cursor/mcp.json`, or **Settings → MCP → Edit config**:

```json
{
  "mcpServers": {
    "neuromesh": {
      "command": "neuromesh",
      "args": ["mcp"]
    }
  }
}
```

Same `mcpServers` shape works for Claude Desktop (`claude_desktop_config.json`), Trae (`.trae/mcp.json`), MiniMax (`.minimax/mcp.json`), and other clients that use that key. [OpenCode](https://opencode.ai/) uses an `mcp` object with `type: "local"` and a `command` array — run `neuromesh connect --print` and map the binary + args. MiMo CLI uses `.mimo-code.json` / `~/.mimo-code/config.json` with an `mcpServers` list. VS Code / Copilot uses `.vscode/mcp.json` with a top-level `servers` object instead — run `neuromesh connect --print` for that shape.

`neuromesh connect` is still preferred when PATH is unreliable: it writes an absolute `command` and pins the workspace via `args: ["mcp", "<path>"]` plus `NEUROMESH_WORKSPACE`.

| Client | Project file | User file (if the app is installed) |
| :--- | :--- | :--- |
| Cursor | `.cursor/mcp.json` | `~/.cursor/mcp.json` |
| VS Code / Copilot | `.vscode/mcp.json` (`servers`) | — |
| Codex | `.codex/config.toml` | `~/.codex/config.toml` |
| [OpenCode](https://opencode.ai/) | `opencode.json` / `.opencode/opencode.jsonc` | `~/.config/opencode/opencode.jsonc` |
| MiMo CLI | `.mimo-code.json` | `~/.mimo-code/config.json` |
| [Antigravity](https://antigravity.google/) | `.agents/mcp_config.json` | `~/.gemini/config/mcp_config.json` |
| Gemini CLI | — | `~/.gemini/settings.json` |
| Kilo Code | `.kilo/kilo.jsonc` (`mcp` + command array) | `kilo.jsonc` in the user config dir |
| Trae | `.trae/mcp.json` | `Trae/User/mcp.json` |
| MiniMax Code | `.minimax/mcp.json` | same `mcpServers` snippet as Cursor |
| Claude Desktop / Code | `.mcp.json` | `claude_desktop_config.json` |
| Windsurf / Cline / Roo | — | their existing MCP settings files |

Stdout is JSON-RPC only (NDJSON). Handshake logs go to stderr so Antigravity and other strict hosts stay happy.

## Agent loop

```
neuromesh_get_context(task_description)
  → if coverage is partial or no_seed_resolved: neuromesh_search_symbols
  → if you need diagnostics: neuromesh_explain_packet(packet_id)
  → if a folded body is required: neuromesh_expand_fold(fold_id)
  → after a successful edit: neuromesh_record_feedback
```

Start with `get_context`. The default packet is **minimal**: `packet_id`, coverage string, selected/packet tokens, skeletonized files, fold descriptors without bodies. `missing` and `next` appear only when coverage is `partial` or `no_seed_resolved` (zero identifiers resolved — do not treat a utility fallback file as the answer). After a good edit, **always** call `neuromesh_record_feedback` — that is the synaptic learning step; without it the next packet does not change.

## Agent rule (recommended)

`neuromesh connect` only registers the MCP server. It does **not** tell the IDE agent to call those tools. Without project instructions, many agents still `Read` / Grep whole files and skip NeuroMesh.

**Full tutorial (every client):** [agent-guide.md](agent-guide.md) — Cursor, VS Code/Copilot, Claude, Codex, OpenCode, MiMo CLI, Antigravity, Kilo, Trae, MiniMax, Gemini CLI, Windsurf, Cline/Roo, Zed, plus a one-shot prompt and smoke test.

Quick Cursor install:

```bash
mkdir -p .cursor/rules
cp /path/to/neuromesh/docs/agent-rule.mdc .cursor/rules/neuromesh.mdc
```

Cursor-ready template: [agent-rule.mdc](agent-rule.mdc). Same body without YAML frontmatter lives in the guide for `AGENTS.md` / `CLAUDE.md` / Copilot instructions.

## Tools

| Tool | Input | Returns |
| :--- | :--- | :--- |
| `neuromesh_get_context` | `task_description`, `prompt`, or `task`; optional `mode`; optional `response_detail` | Compact evidence packet (`minimal` by default) |
| `neuromesh_explain_packet` | `packet_id`; optional `include` | On-demand diagnostics (no fold bodies) |
| `neuromesh_expand_fold` | `fold_id`, `node_id`, or `query`; optional `reason` | Original folded body |
| `neuromesh_get_file_skeleton` | `file_path`, optional `active_symbols` | One skeletonized file + fold descriptors (no bodies) |
| `neuromesh_search_symbols` | `query`, optional `limit` | Ranked hits |
| `neuromesh_get_dependencies` | name or path | Typed neighbors |
| `neuromesh_trace` | `query`, `direction` (`in` / `out` / `both`), `depth` | Call/import chains. Inbound includes `throw new`, `throw $e` after `catch (Type $e)`, and ternary `new Type`. Trace the exception that is actually thrown. |
| `neuromesh_analyze_impact` | `query`, `depth` | Blast radius |
| `neuromesh_get_architecture` | — | Languages, packages, entry points |
| `neuromesh_get_project_memory` | — | Seeded facts |
| `neuromesh_record_feedback` | `task_success`, `touched_nodes` | STDP on that path; persists `base_relevance` to `graph.bin` |
| `neuromesh_get_node_weights` | `query` / `symbol` / `path` | Read `access_count`, `base_relevance`, `learning_bonus` (verify learning) |
| `neuromesh_expand_gap` | `path`, optional `token_cap` | Cheap skeleton for `packet_gaps` paths |
| `neuromesh_get_stats` | — | Node/edge counts |

Aliases exist for older clients (`activate_context`, `expand_context`, `search_context`, `explain_packet`, `get_context_details`). Prefer the `neuromesh_*` names.

## Evidence packet

`neuromesh_get_context` is the product. Default (`response_detail=minimal`) shape:

- `packet_id` — session key for `neuromesh_explain_packet` (LRU, ~10 minutes, 32 packets)
- `coverage` — `claim` (`no_recorded_gap`, `bounded`, `partial`, `no_seed_resolved`), plus `covered`, `skipped`, `sidecar_files`, `unsure`, `packet_gaps`, and optional `semantic_coverage` for style tasks. `no_recorded_gap` only when every attempted seed resolved, `packet_gaps` is empty, no sidecar connector files, and the packet was not budget-truncated. `bounded` means seeds resolved but optional connector/sidecar fill or budget cut was applied — do not Grep unless you need more context.
- `tokens.selected` / `tokens.packet` — raw selected vs skeletonized packet
- `files[]` — `path`, short `why`, optional `sidecar: true` (connector fill, not a task anchor), skeleton `code`, `folds[]` as descriptors (`fold_id`, `symbol`, `signature`, lines, `saved_tokens`) with **no** `original_body`
- `missing` / `next` — only when coverage is incomplete; one search action, not a repeated seed list

`mode`: `balanced` (default, +5,000 fill), `max_savings` (0), `max_quality` (+16,000). Critical tasks (auth / payment / secret) upgrade to max quality. `mode` does not add metadata; `response_detail` does (`minimal` ≤ 256 metadata tokens, `standard` ≤ 750, `diagnostic` on demand).

`neuromesh_explain_packet` returns seeds, selection, budget, physarum, and membrane for a `packet_id`. Pass `include: ["graph"]` for graph stats; otherwise use `neuromesh_get_stats`. The HTTP monitor (`/api/simulate`) still requests `diagnostic` so the VS Code inspector keeps the nested `evidence_packet`.

## Folds

Markers look like:

```
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```

Pass that `fold_id` to `neuromesh_expand_fold` as `fold_id`, `node_id`, or `query`. The full marker line also works. Folds persist for the **MCP session** (same process, same project). They are not cleared on every `get_context`. A new project id wipes the registry. Ids include a short path tag so two files that both fold `write` do not collide. Fold **bodies** are never in `get_context` or `get_file_skeleton`; only `expand_fold` restores them.

## Learning

`neuromesh_record_feedback` updates `base_relevance`, edge pheromones, and episodic memory. **Durable weights** persist in `graph.bin` (episode checkpoints in the snapshot); `neuromesh.json` holds episodic records for recall. The response includes `episode_saved_this_call`, `learning_episodes_in_store`, and `persisted_to: "graph.bin"` (`episodes_recorded` is a per-call 0/1 compat field). Effects apply on the **next** `get_context` in the same MCP process (search ranking, selector fill, episodic recall). Use `neuromesh_get_node_weights` before and after feedback to verify deltas. Learning does not change a packet mid-request; restart the MCP server to load persisted graph state from a prior session.
