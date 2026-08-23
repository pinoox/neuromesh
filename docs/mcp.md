# MCP tools

Transport: **stdio JSON-RPC** (`neuromesh mcp`). That is what Cursor, Claude, Cline, and similar clients launch.

Remote / multi-agent: `neuromesh monitor`, then SSE and HTTP as in [api.md](api.md).

## Agent loop

```
neuromesh_get_context(task_description)
  → if a folded body is required: neuromesh_expand_fold(fold_id)
  → if you need callers: neuromesh_trace
  → after a successful edit: neuromesh_record_feedback
```

Start with `get_context`. Grep only when `coverage.claim` is `partial` or a seed is listed in `seeds_missed`.

## Tools

| Tool | Input | Returns |
| :--- | :--- | :--- |
| `neuromesh_get_context` | `task_description` or `prompt`, optional `mode` | Evidence packet |
| `neuromesh_expand_fold` | `fold_id` or `node_id`, optional `reason` | Original folded body |
| `neuromesh_get_file_skeleton` | `file_path`, optional `active_symbols` | One skeletonized file |
| `neuromesh_search_symbols` | `query`, optional `limit` | Ranked hits |
| `neuromesh_get_dependencies` | name or path | Typed neighbors |
| `neuromesh_trace` | `query`, `direction` (`in` / `out` / `both`), `depth` | Call/import chains |
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
- `next_actions` — expand a fold, trace, or search

`mode`: `balanced` (default), `max_savings`, `max_quality`. Critical tasks (auth / payment / secret) upgrade to max quality.

## Folds

Markers look like:

```
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```

Pass that `fold_id` to `neuromesh_expand_fold`. The body comes from the in-memory registry of the **current** `get_context` session — call expand in the same MCP process, after get_context, not as a cold start.
