# HTTP monitor

`neuromesh monitor` binds **http://127.0.0.1:8765** by default (local only). Change it with `neuromesh port 9000`, `neuromesh monitor --port 9000`, or `NEUROMESH_PORT`. See [cli.md](cli.md#monitor-port). Re-index honors the same file cap as the CLI (`--max-files` / `NEUROMESH_MAX_FILES` / config; default auto, ceiling 50,000 — [cli.md](cli.md#index-file-cap)). Use this for the graph UI and for clients that speak HTTP/SSE instead of stdio MCP.

## UI

- 2D / 3D view of the indexed workspace
- Token and graph density telemetry
- English / Persian chrome

## Useful endpoints

| Method | Path | Notes |
| :--- | :--- | :--- |
| `GET` | `/sse` | MCP-over-SSE for remote / multi-agent setups |
| `POST` | `/mcp` | JSON-RPC MCP messages |
| `GET` | `/api/v1/status` | Health, project, cache-ish stats |
| `GET` | `/api/usage` | Aggregated telemetry + history (`mean_reduction_pct`, `overall_reduction_pct`; same source as `neuromesh usage`) |
| `POST` | `/api/v1/projects/index` | Re-index |
| `POST` | `/api/v1/context/activate` | Packet without an LLM in the middle |
| `POST` | `/api/v1/context/expand` | Expand a fold or inactive node |
| `GET` | `/api/engines` | Effective graph backend + seed engine (v0.8.6) |
| `POST` | `/api/engines` | Save graph backend / seed engine to `nm.config.json` |
| `GET` | `/api/graph-proxy/probe` | Live CBM connect + sample packet (same as `doctor --proxy --probe`) |

**Settings UI** (Galaxy monitor): Graph Backend (Native / Auto / CBM) and Seed Engine presets; **Probe CBM connection** uses the probe endpoint above.

Optional headers on proxy-style routes: `X-NeuroMesh-Mode: max_quality | balanced | max_savings`.

For day-to-day coding in Cursor or Claude, prefer **stdio** (`neuromesh mcp` or `nmx mcp`). See [mcp.md](mcp.md).

Telemetry rows (MCP, CLI, monitor) share `~/.neuromesh/telemetry_history.json` with fields `surface` (`mcp` | `cli` | `monitor`), optional `workspace_path`, `client_id`, and `command`.
