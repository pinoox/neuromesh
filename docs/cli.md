# CLI

Binary: `neuromesh` (`neuromesh-cli` crate).

```
neuromesh mcp          # stdio MCP server (what the IDE launches)
neuromesh monitor      # Web UI + SSE (default http://127.0.0.1:8765)
neuromesh port         # show or persist the monitor port
neuromesh index        # build the graph and seed project memory
neuromesh status       # node / edge counts
neuromesh usage        # MCP token telemetry (`--all`, `--limit N`)
neuromesh graph        # graph stats
neuromesh memory       # project facts
neuromesh optimize     # one prompt → print the packet
neuromesh eval         # gold recall / precision / fill on cwd and tests/fixtures
neuromesh benchmark    # same as eval
neuromesh store        # managed home vs trusted local `.neuromesh`
neuromesh connect      # ready-to-paste MCP JSON
neuromesh doctor       # workspace root, scan, skipped extensions, graph, port
neuromesh version
```

## Usage telemetry

MCP tool calls (`neuromesh_get_context`, skeleton, search, expand) append to `~/.neuromesh/telemetry_history.json`. That file is the source of truth. The monitor UI reads the same file (and also accepts `POST /api/telemetry/record` when `neuromesh monitor` is running).

```bash
neuromesh usage              # this project
neuromesh usage --all        # every project on disk
neuromesh usage --limit 50   # more recent rows
```

Rows appear when an agent **calls a NeuroMesh MCP tool**. Saving a file in Cursor, switching the editor theme, or restarting the IDE does not add a row. `neuromesh mcp` has no HTTP port; if the monitor is down, `usage` still prints the file.

## Data directory

By default NeuroMesh does **not** write `<workspace>/.neuromesh`. Graph, memory, and per-project port/max-files live in a stable home slot:

```
~/.neuromesh/projects/<folder>-<hash>/
  graph.json
  neuromesh.json
  config.json
```

Telemetry stays in `~/.neuromesh/telemetry_history.json`. Override the home root with `NEUROMESH_HOME`.

```bash
neuromesh store                 # print mode + path for cwd
neuromesh store local           # trust THIS repo's .neuromesh
neuromesh store managed         # back to the home slot (default)
```

Or in `~/.neuromesh/config.json`:

```json
{
  "project_store": "managed",
  "trust_local": ["c:/projects/neuromesh"]
}
```

`"project_store": "local"` is the old behavior for every workspace. `NEUROMESH_STORE=local` is a one-shot. Leftover in-repo `.neuromesh` folders are copied into the managed slot once, then ignored until you trust them.

## Everyday

```bash
neuromesh doctor
neuromesh index
neuromesh optimize -- "How does handle_tool_call extract intent?"
neuromesh eval
```

`eval` scores `tests/gold_tasks.toml` if present, otherwise the builtin set. It also walks `tests/fixtures/*/gold_tasks.toml`.

The process uses the **current working directory** as the project. Point your MCP config at a command that starts in the repo you care about.

## Monitor port

Default is **8765**. Change it without editing JSON by hand:

```bash
neuromesh port                 # print host + effective port
neuromesh port 9000            # write the project slot under ~/.neuromesh/projects/
neuromesh monitor --port 9000  # one run (`-p 9000` or `--port=9000`)
```

Priority: `--port` / `-p` → env `NEUROMESH_PORT` → project slot `config.json` → `~/.neuromesh/config.json` → 8765.

`neuromesh start` honors the same flag. VS Code / Cursor setting `neuromesh.port` must match the process that is actually listening.

`neuromesh mcp` does **not** bind a port (stdio only). Remote MCP is the monitor: `neuromesh monitor --port 9000`, then `http://127.0.0.1:9000/sse`.

## Index file cap

Default is **auto**. After a walk, production sources (`src/`, not `tests/` / `test/`) are indexed first. The cap grows to that production count (minimum 6,000) and never past **50,000**. Tests are queued last, so a Symfony-scale tree is not truncated in the middle of `src/`.

```bash
neuromesh index --max-files auto     # persist auto (default)
neuromesh index --max-files 20000    # persist a hard limit
neuromesh index --max-files=20000    # same
neuromesh doctor --max-files 20000   # one scan only, does not save
```

`auto` and `0` mean auto-grow. Priority: `--max-files` → env `NEUROMESH_MAX_FILES` → project slot `config.json` → `~/.neuromesh/config.json` → auto.

`neuromesh index --max-files …` writes `max_files` next to the monitor port in the project data dir (`neuromesh store` prints it). `neuromesh monitor --max-files 20000` is one run only (same idea as `--port`).

`index` and `doctor` print `File cap: auto → N (ceiling 50000)` and `Truncated` when files were omitted. Re-index after changing the cap.
