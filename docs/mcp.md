# MCP tools

Transport: **stdio JSON-RPC** (`neuromesh mcp <workspace>`). **v0.9.0 default:** **`engine: fast`** — graph index + query-side lexical expansion; pass the prompt only to `get_context_packet`. Opt in to **`hybrid`** / **`deep`** for bundled MiniLM embed-primary.

That is what Cursor, Claude, Codex, OpenCode, MiMo CLI, Antigravity, Kilo Code, Trae, Cline, and similar clients launch. Stdio has **no TCP port** — `--port` on `mcp` does nothing. Background index uses the same file-cap rules as `neuromesh index` (`--max-files`, `NEUROMESH_MAX_FILES`, project-slot `config.json`; default auto, ceiling 50,000). See [cli.md](cli.md#index-file-cap).

**Warning:** Never run `neuromesh mcp` without a workspace path (or `NEUROMESH_WORKSPACE`) **unless** the IDE sets `WORKSPACE_FOLDER_PATHS` / `VSCODE_CWD` or sends a workspace root in MCP `initialize`. Without any of those, the server may bind to your home directory and index unrelated projects. For a one-time global install, use the simple config below; `neuromesh connect --global` writes it to `~/.cursor/mcp.json`.

Remote / multi-agent: `neuromesh monitor` (optionally `--port 9000` / `neuromesh port 9000`), then SSE and HTTP as in [api.md](api.md).

## Connect

```bash
neuromesh connect --global --agent-rules   # recommended: MCP + Cursor agent rule
neuromesh connect --global       # MCP only (one global config for Cursor)
neuromesh connect --print        # snippets only
neuromesh connect --project      # this repo only (usually unnecessary)
neuromesh connect --pinned       # legacy: absolute binary + workspace in args
```

`--agent-rules` copies [agent-rule.mdc](agent-rule.mdc) into `.cursor/rules/neuromesh.mdc` so the IDE agent prefers NeuroMesh tools over raw `Read` / Grep.

Default `connect` writes a **portable** config: `command: "neuromesh"`, `args: ["mcp"]`. NeuroMesh detects the active project from IDE env vars (`WORKSPACE_FOLDER_PATHS`, `VSCODE_CWD`, …) and from MCP `initialize` (`rootUri`, `workspaceFolders`). No per-project workspace registration required.

`--pinned` keeps the old behavior: absolute binary path, `args: ["mcp", "<workspace>"]`, and `NEUROMESH_WORKSPACE` for hosts where PATH or auto-detection is unreliable.

### Manual

When `neuromesh` is on **PATH**, paste this into **global** MCP settings (`~/.cursor/mcp.json`) — works for every project without registering a workspace:

```json
{
  "mcpServers": {
    "neuromesh": {
      "type": "stdio",
      "command": "neuromesh",
      "args": ["mcp"]
    }
  }
}
```

Or run `neuromesh connect --global` once. Use `--pinned` only when PATH is unreliable or auto-detection fails in your IDE.

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
| Zed | — | `~/.config/zed/settings.json` (`context_servers.neuromesh`) |
| JetBrains | `.idea/mcp.json` | — |
| Windsurf / Cline / Roo | — | their existing MCP settings files |

Stdout is JSON-RPC only (NDJSON). Handshake logs go to stderr so Antigravity and other strict hosts stay happy.

## Agent loop

```
get_context_packet(query / task_description / prompt / task)
  → if coverage is partial or no_seed_resolved: neuromesh_search_symbols
  → if you need diagnostics: neuromesh_explain_packet(packet_id)
  → if a folded body is required: neuromesh_expand_fold(fold_id)
  → after a successful edit: neuromesh_record_feedback
```

Start with `get_context_packet`. Pass the **prompt as written** — any language. Optional: `path_hints`, `entity_types`, `mode`. Do **not** send `keywords` / `expansion` on **`engine: fast`** (server auto-expands) or on **`engine: hybrid|deep`** (MCP ignores them; MiniLM handles NL). The deprecated alias `neuromesh_get_context` still works for one release.

## Agent rule (recommended)

`neuromesh connect` only registers the MCP server. It does **not** tell the IDE agent to call those tools. Without project instructions, many agents still `Read` / Grep whole files and skip NeuroMesh.

**Full tutorial (every client):** [agent-guide.md](agent-guide.md) — Cursor, VS Code/Copilot, Claude, Codex, OpenCode, MiMo CLI, Antigravity, Kilo, Trae, MiniMax, Gemini CLI, Windsurf, Cline/Roo, Zed, plus a one-shot prompt and smoke test.

Quick Cursor install:

```bash
neuromesh connect --agent-rules    # from repo root (or use --global for MCP only)
# or manually:
mkdir -p .cursor/rules
cp /path/to/neuromesh/docs/agent-rule.mdc .cursor/rules/neuromesh.mdc
```

Cursor-ready template: [agent-rule.mdc](agent-rule.mdc). Same body without YAML frontmatter lives in the guide for `AGENTS.md` / `CLAUDE.md` / Copilot instructions.

## Tools

| Tool | Input | Returns |
| :--- | :--- | :--- |
| **`get_context_packet`** | `query`, `task_description`, `prompt`, or `task`; optional `path_hints`, `entity_types`, `intent`, `engine`, `mode`, `response_detail` | Compact evidence packet (`minimal` by default) + `task.seed_resolution` telemetry |
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

Aliases exist for older clients (`neuromesh_get_context`, `activate_context`, `expand_context`, `search_context`, `explain_packet`, `get_context_details`). Prefer **`get_context_packet`** and the `neuromesh_*` expansion tools.

## Evidence packet

`get_context_packet` is the product. Default (`response_detail=minimal`) shape:

- `packet_id` — session key for `neuromesh_explain_packet` (LRU, ~10 minutes, 32 packets)
- `coverage` — `claim` (`no_recorded_gap`, `bounded`, `partial`, `no_seed_resolved`), plus `covered`, `skipped`, `sidecar_files`, `unsure`, `packet_gaps`, and optional `semantic_coverage` for style tasks. `no_recorded_gap` only when every attempted seed resolved, `packet_gaps` is empty, no sidecar connector files, and the packet was not budget-truncated. `bounded` means seeds resolved but optional connector/sidecar fill or budget cut was applied — do not Grep unless you need more context.
- `tokens.selected` / `tokens.packet` — raw selected vs skeletonized packet
- `files[]` — `path`, short `why`, optional `sidecar: true` (connector fill, not a task anchor), skeleton `code`, `folds[]` as descriptors (`fold_id`, `symbol`, `signature`, lines, `saved_tokens`) with **no** `original_body`
- `missing` / `next` — only when coverage is incomplete; one search action, not a repeated seed list

`mode`: `balanced` (default, +5,000 fill), `max_savings` (0), `max_quality` (+16,000). Critical tasks (auth / payment / secret) upgrade to max quality. `mode` does not add metadata; `response_detail` does (`minimal` ≤ 256 metadata tokens, `standard` ≤ 750, `diagnostic` on demand).

### Retrieval metadata (v0.9.0)

Present on **all** detail levels when tiered activation runs. Default **`engine: fast`** sets `retrieval.resolution_tier` to **`lexical_primary`**. With **`hybrid`** / **`deep`**, expect **`embedding_primary`** when MiniLM ANN resolves seeds. `retrieval.cache_hit: true` means a near-duplicate prompt reused the semantic LRU (fresh `packet_id`). `minimal` uses a compact block; `standard` and `diagnostic` include full latency and confidence fields.

**Native** example:

```json
"retrieval": {
  "retrieval_level": "L1",
  "sufficiency_score": 0.72,
  "confidence": 0.68,
  "claim": "partial",
  "levels_attempted": ["L1"],
  "latency_ms": { "L1": 18 },
  "critical_gaps": [],
  "eligible_for_early_exit": false,
  "next_action": "neuromesh_search_symbols",
  "suggested_keywords": ["Router", "middleware"]
}
```

**Proxy** (`graph_backend: proxy_cbm`, v0.8.2+): `retrieval_level` is `"proxy"`. Confidence and sufficiency are **computed** from matched vs expected keywords (never hardcoded). `claim` is `no_seed_resolved`, `partial`, or `bounded` — never `likely_sufficient` on proxy v1.

```json
"retrieval": {
  "retrieval_level": "proxy",
  "claim": "bounded",
  "confidence": 0.2,
  "sufficiency_score": 0.15,
  "critical_gaps": ["next"],
  "suggested_keywords": ["next"],
  "graph_backend": "proxy_cbm",
  "provider": "cbm"
}
```

Treat `claim` as a **decision signal**, not ground truth. Prefer acting on `partial` over assuming sufficiency. L3 is rare — only when critical gaps persist after L2.

`neuromesh_explain_packet` returns seeds, selection, budget, physarum, and membrane for a `packet_id`. `selection.candidates` lists ranked files with `selected`, `emitted`, `drop_stage`, `score_breakdown`, and `learning_bonus` — use this when debugging why feedback did or did not change the emitted packet. Pass `include: ["graph"]` for graph stats; otherwise use `neuromesh_get_stats`. The HTTP monitor (`/api/simulate`) still requests `diagnostic` so the VS Code inspector keeps the nested `evidence_packet`.

## Folds

Markers look like:

```
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```

Pass that `fold_id` to `neuromesh_expand_fold` as `fold_id`, `node_id`, or `query`. The full marker line also works. Folds persist for the **MCP session** (same process, same project). Folds from the current activation stay resolvable; older activations are LRU-trimmed (cap 2000) so long sessions do not grow without bound. A new project id wipes the registry. Ids include a short path tag so two files that both fold `write` do not collide. Fold **bodies** are never in `get_context` or `get_file_skeleton`; only `expand_fold` restores them.

## Learning

`neuromesh_record_feedback` updates `base_relevance`, edge pheromones, and episodic memory. **Durable weights** persist in `graph.bin` (episode checkpoints in the snapshot); `neuromesh.json` holds episodic records for recall. The response includes `episode_saved_this_call`, `learning_episodes_in_store`, and `persisted_to: "graph.bin"` (`episodes_recorded` is a per-call 0/1 compat field).

Effects apply on the **next** `get_context_packet` in the same MCP process: search ranking, selector fill, unified scoring, and the **emission pipeline** (which files actually ship after fill/packet caps). Negative feedback lowers `base_relevance` and can suppress penalized files from the optional set. Use `neuromesh_get_node_weights` before and after feedback to verify deltas. When a file shows `selected: true` but is missing from `files[]`, call `neuromesh_explain_packet` and check `emitted` / `drop_stage` on `selection.candidates`.

Learning does not change a packet mid-request; restart the MCP server to load persisted graph state from a prior session. Positive reinforcement (`learning_bonus ≥ learning_promotion_min_bonus`, default **14**) can **add** focus-matched files to the next packet; it does not inject heavily reinforced files into unrelated queries. Negative reinforcement drops penalized hop-expanded files. Benchmark locally with `neuromesh eval --learning`.
