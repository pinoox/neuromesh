# CLI

Single Cargo binary: **`neuromesh`**. **`nmx`** is a packaging alias (symlink on Unix, hard link or copy on Windows) — not a second compile target.

## Install (recommended)

Pre-built release binaries — no Rust toolchain required. Default **`engine: fast`** needs no embed warm at startup; hybrid/deep use bundled MiniLM.

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/pinoox/neuromesh/main/install.sh | bash

# Windows (PowerShell)
irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex
```

Then: `neuromesh doctor` → `neuromesh connect` → `neuromesh index`. Advanced presets and sidecar: [configuration.md](configuration.md).

```
neuromesh mcp          # stdio MCP server (what the IDE launches)
nmx mcp                # same binary, short alias
neuromesh monitor      # Web UI + SSE (default http://127.0.0.1:8765)
neuromesh port         # show or persist the monitor port
neuromesh index        # build the graph and seed project memory
neuromesh status       # node / edge counts
neuromesh usage        # MCP token telemetry (`--all`, `--limit N`)
neuromesh graph        # graph stats
neuromesh memory       # project facts
neuromesh optimize     # one prompt → print the packet
neuromesh packet       # JSON packet for benchmarks (`--json`, `--engine`, `--keywords`)
neuromesh eval         # gold recall / precision / fill on cwd and tests/fixtures
neuromesh eval --learning   # dose-response learning benchmark (learning-causal fixture)
neuromesh benchmark    # same as eval
neuromesh store        # managed home vs trusted local `.neuromesh`
neuromesh config       # retrieval engine + settings (global or nm.config.json)
neuromesh connect      # write MCP configs (or `--print` snippets)
neuromesh doctor       # workspace root, scan, skipped extensions, graph, port
neuromesh version
```

## Usage telemetry

MCP tool calls (`get_context_packet`, skeleton, search, expand, trace, stats, …) append to `~/.neuromesh/telemetry_history.json`. The MCP `initialize` handshake writes one `mcp_session` row per process. That file is the source of truth. The monitor UI reads the same file (and also accepts `POST /api/telemetry/record` when `neuromesh monitor` is running).

```bash
neuromesh usage              # this project
neuromesh usage --all        # every project on disk
neuromesh usage --limit 50   # more recent rows
```

Rows appear when an agent **connects** (`initialize` → one `mcp_session` row) or **calls a NeuroMesh MCP tool**. Saving a file in Cursor, switching the editor theme, or restarting the IDE does not add a row. A chat turn that the agent answers without calling a NeuroMesh tool also does not. `neuromesh mcp` has no HTTP port; if the monitor is down, `usage` still prints the file. If the IDE workspace name differs from the directory where you run the CLI, use `neuromesh usage --all`. Mean reduction uses packet rows only (search/session 0-token rows are counted as requests, not as 0% savings).

## Data directory

By default NeuroMesh does **not** write `<workspace>/.neuromesh`. Graph, memory, and per-project port/max-files live in a stable home slot:

```
~/.neuromesh/projects/<folder>-<hash>/
  graph.bin
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

## Retrieval engine & config

NeuroMesh v0.9.0 uses one **retrieval engine** preset: `fast` | `hybrid` | `deep`. Default **`fast`** — graph index + query-side lexical expansion; pass the prompt only.

| Layer | File | Scope |
| :--- | :--- | :--- |
| Global | `~/.neuromesh/config.json` | all workspaces |
| Project | `nm.config.json` in repo root | commit-friendly per repo |
| Managed slot | `~/.neuromesh/projects/…/config.json` | port / max-files (via `neuromesh port`) |
| Env | `NEUROMESH_ENGINE` | one-shot override |
| MCP | `engine` param on `get_context_packet` | per call |

```bash
neuromesh config                              # effective settings for cwd
neuromesh config engine get                   # show engine + override layers
neuromesh config engine hybrid                # opt-in MiniLM sidecar
neuromesh index --mode hybrid                 # graph + embed rebuild
```

## Packet (JSON benchmark path)

Same activation path as MCP `get_context_packet`, for scripts and CI:

```bash
neuromesh packet --json --query "Where is HTTPAdapter.send?"
neuromesh packet --json --engine hybrid --query "How does redirect work?"
```

Stdout is one JSON object: coverage, files, tokens, `seed_resolution`, client signals. Human-readable output when `--json` is omitted.

Example `nm.config.json` (copy from `nm.config.example.json`):

```json
{
  "retrieval": { "engine": "fast" },
  "packet_header": { "enabled": true }
}
```

Engines: **`fast`** (default) · **`hybrid`** (MiniLM sidecar) · **`deep`** (max quality + dedup).

Graph backend (`graph_backend.backend`): `native` (default) · `auto` · `proxy_cbm` · `proxy_graphify`. See [graph-proxy.md](graph-proxy.md).

```bash
neuromesh config graph-backend auto
neuromesh doctor --proxy
neuromesh doctor --proxy --probe    # live CBM connect + sample packet
```

Monitor **Settings → Retrieval Engine / Graph Backend** saves `nm.config.json` and reconnects the proxy. See [configuration.md](configuration.md) · [engines.md](engines.md).

## Everyday

```bash
neuromesh doctor
neuromesh doctor --embed              # embedding sidecar + cold warm
neuromesh doctor --embed --bench      # p50/p95 warm embed latency
neuromesh embed prefetch              # warm bundled MiniLM (HF fallback if missing)
neuromesh index
neuromesh optimize -- "How does handle_tool_call extract intent?"
neuromesh eval
neuromesh eval --learning
neuromesh eval --release-gates                    # gates for effective engine
neuromesh eval --release-gates --engine fast        # zero-embed gate matrix
neuromesh eval --release-gates --engine hybrid      # embed-primary gates
neuromesh eval --calibrate        # dev/holdout calibration from test3 JSON
```

`eval` scores `tests/gold_tasks.toml` if present, otherwise the builtin set. It also walks `tests/fixtures/*/gold_tasks.toml`.

`eval --learning` indexes `tests/fixtures/learning-causal/`, sweeps reinforcement levels on `PromoCodeInput`, and prints bonus → rank → emitted → MRR. See [quality.md](quality.md#learning--emission-v0715).

`eval --release-gates` runs the v0.9.0 tiered holdout matrix per engine (`--engine fast|hybrid|deep`; recall ≥55%, precision ≥73%, no_seed ≤2).

The process uses the **current working directory** as the project. `neuromesh connect` writes that path into each client's MCP config so the IDE does not have to guess.

## Connect MCP clients

Default is **portable** — one global install works for every project (workspace auto-detected). Use `nmx` anywhere you use `neuromesh`.

```bash
neuromesh connect --global --agent-rules   # recommended once per machine
neuromesh connect --print                  # snippets only
neuromesh connect --dry-run                # list target files
neuromesh connect --project                # this repo only
neuromesh connect --pinned                 # legacy: absolute binary + workspace pin
neuromesh smoke                            # quick graph + telemetry check
neuromesh doctor --mcp                     # MCP workspace env diagnostics
```

See [mcp.md](mcp.md#connect).

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

## Build from source

```bash
cargo build --release -p neuromesh-cli --features embeddings
# optional short alias in target/release (hard link / symlink — not a second compile):
./scripts/link-cli-alias.sh          # Unix
./scripts/link-cli-alias.ps1         # Windows
```

Release zips and `install.sh` / `install.ps1` ship both `neuromesh` and `nmx` as the same binary (symlink or hard link). `cargo install … --bin neuromesh` installs only `neuromesh`; add `nmx` with the link script or a shell alias.
