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
| `POST` | `/api/v1/projects/index` | Re-index |
| `POST` | `/api/v1/context/activate` | Packet without an LLM in the middle |
| `POST` | `/api/v1/context/expand` | Expand a fold or inactive node |

Optional headers on proxy-style routes: `X-NeuroMesh-Mode: max_quality | balanced | max_savings`.

For day-to-day coding in Cursor or Claude, prefer **stdio** (`neuromesh mcp`). See [mcp.md](mcp.md).
