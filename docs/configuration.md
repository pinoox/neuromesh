# Configuration & advanced setup

v0.9.0 uses one **retrieval engine** preset instead of removed v0.8 flags (`config seed-engine`, `config embeddings`, `NEUROMESH_SEED_ENGINE`, `NEUROMESH_EMBEDDINGS`).

For install and the daily agent loop, start with the [README](../README.md). This doc covers presets, sidecar, proxy, env vars, and operational tuning.

---

## Retrieval engine presets

| `engine` | Index | Query | RAM (typical) | Use when |
| :--- | :--- | :--- | :--- | :--- |
| **`fast`** (default) | graph only | server-assisted keywords + graph | <80 MB | Most repos; instant index |
| **`hybrid`** | graph + hierarchical v6 | file ANN → lazy symbols + graph | ~250 MB | Obfuscated naming, multilingual NL |
| **`deep`** | graph + **all symbols** | flat symbol ANN + dedup + centroids | ~450 MB | Large refactors, max recall |

```bash
neuromesh config engine get              # show effective preset + override layers
neuromesh config engine hybrid           # opt-in MiniLM sidecar
neuromesh index --mode hybrid            # graph + embed rebuild
neuromesh embed rebuild                  # refresh sidecar after hybrid/deep switch
neuromesh doctor --engine                # preset + ONNX skip status
neuromesh eval --release-gates --engine fast|hybrid|deep
```

### Config files

| Layer | File | Scope |
| :--- | :--- | :--- |
| Global | `~/.neuromesh/config.json` | all workspaces |
| Project | `nm.config.json` in repo root | commit-friendly per repo |
| Managed slot | `~/.neuromesh/projects/…/config.json` | port / max-files |
| Env | `NEUROMESH_ENGINE` | one-shot override |
| MCP | `engine` param on `get_context_packet` | per call |

Example `nm.config.json` (copy from `nm.config.example.json`):

```json
{
  "retrieval": { "engine": "fast" },
  "packet_header": { "enabled": true }
}
```

Env: `NEUROMESH_ENGINE=fast|hybrid|deep`

Granular flags (`two_stage_enabled`, `optional_dedup_min_cosine`, `module_cluster_enabled`, `intra_threads`) are **derived from the preset** — do not set them manually unless you maintain a fork.

---

## Hybrid / deep: MiniLM sidecar

When `engine` is `hybrid` or `deep`:

| Item | Value |
| :--- | :--- |
| **Model** | `minilm_multilingual_q` — Paraphrase MiniLM L12 v2 Q |
| **Dimensions** | 384 |
| **Weights** | Bundled in release (`models/minilm-multilingual-q/`) |

### Hybrid — hierarchical sidecar (v6)

| Item | Value |
| :--- | :--- |
| **Sidecar** | `embeddings.bin` — tier-0 file vectors + lazy tier-1 symbols |

Cold `neuromesh embed rebuild` embeds **one passage per file** (~250 MiniLM passes). Symbol vectors are **lazy**: first query that hits a file batch-embeds up to 64 symbols and persists incrementally.

Query flow: **file ANN** (top 4) → **lazy symbol embed** → **symbol subset ANN** + coarse lexical pool → full-ANN fallback.

### Deep — full symbol sidecar

| Item | Value |
| :--- | :--- |
| **Sidecar** | `embeddings.bin` — **every symbol** embedded at rebuild (no file tier, no lazy tier) |

Cold `neuromesh embed rebuild` runs MiniLM on **all symbol passages** (slower rebuild, maximum recall at query time). Query uses flat two-stage ANN over the full symbol matrix + module centroids and optional-file dedup.

| Feature | `hybrid` | `deep` |
| :--- | :--- | :--- |
| Index shape | hierarchical v6 (file + lazy symbol) | flat (all symbols) |
| Cold rebuild | ~250 file passes | all symbols (~8000+ typical) |
| Two-stage ANN | on | on |
| Optional-file dedup | off | on (0.93) |
| Module centroids | off | on |
| Optimization mode | balanced | max_quality |

Safety (hybrid lazy writes): concurrent MCP queries serialize sidecar writes; `embeddings.bin` is replaced atomically (temp + rename).

Sidecar v4/v5 requires `neuromesh embed rebuild` after upgrading to v0.9.0.

```bash
neuromesh doctor --embed              # sidecar status + cold warm
neuromesh doctor --embed --bench      # p50/p95 embed latency
neuromesh embed prefetch              # warm bundled MiniLM (HF fallback if missing)
```

Release tarballs include MiniLM weights; `embed prefetch` only warms or fetches fallback weights.

---

## Graph backend (CBM proxy)

Optional external graph for **`get_context_packet`** only. Folding, `search_symbols`, and `trace` stay native.

| Value | Meaning |
| :--- | :--- |
| `native` | Built-in AST index (**default**) |
| `auto` | CBM from IDE MCP when found; else native |
| `proxy_cbm` | Always spawn codebase-memory-mcp for packets |

```bash
neuromesh config graph-backend native
neuromesh config graph-backend auto
neuromesh doctor --proxy
neuromesh doctor --proxy --probe    # live CBM connect + sample packet
```

Monitor **Settings → Graph Backend** saves `nm.config.json` and reconnects. See [graph-proxy.md](graph-proxy.md).

On proxy, `retrieval.claim` is conservative (`partial` / `bounded` / `no_seed_resolved`) — never treat silence as completeness.

---

## Packet modes

| Mode | Extra tokens on top of seeds | Use |
| :--- | ---: | :--- |
| `max_savings` | 0 | Tiny, obvious edits |
| `balanced` | 5,000 | Default |
| `max_quality` | 16,000 | Refactors, auth, critical paths |

Seeds are never truncated to fake a small packet. Auth / payment tasks auto-upgrade to `max_quality`.

Pass `mode` on `get_context_packet` or set defaults in config.

---

## Data directory & store mode

By default NeuroMesh does **not** write `<workspace>/.neuromesh`. Graph and memory live in a managed home slot:

```
~/.neuromesh/projects/<folder>-<hash>/
  graph.bin
  neuromesh.json
  config.json
```

```bash
neuromesh store                 # print mode + path for cwd
neuromesh store local           # trust THIS repo's .neuromesh
neuromesh store managed         # back to home slot (default)
```

Override home root: `NEUROMESH_HOME`.

In `~/.neuromesh/config.json`:

```json
{
  "project_store": "managed",
  "trust_local": ["c:/projects/my-app"]
}
```

---

## Monitor port

Default **8765**. MCP stdio has **no TCP port** — only the monitor binds HTTP/SSE.

```bash
neuromesh port                 # print effective port
neuromesh port 9000            # persist for this project slot
neuromesh monitor --port 9000  # one run only
```

Priority: `--port` / `-p` → `NEUROMESH_PORT` → project slot `config.json` → `~/.neuromesh/config.json` → 8765.

Remote MCP: `neuromesh monitor --port 9000` → `http://127.0.0.1:9000/sse`. See [api.md](api.md).

VS Code / Cursor: setting `neuromesh.port` must match the running monitor.

---

## Index file cap

Default **auto**: production sources first, tests last, ceiling **50,000** files.

```bash
neuromesh index --max-files auto
neuromesh index --max-files 20000
neuromesh doctor --max-files 20000   # scan only, does not save
```

Priority: `--max-files` → `NEUROMESH_MAX_FILES` (`auto` / `0` = auto) → project slot → auto.

---

## Connect options

```bash
neuromesh connect --global --agent-rules   # global MCP + Cursor rule (recommended)
neuromesh connect --print                  # snippets only
neuromesh connect --project                # this repo only
neuromesh connect --pinned                 # legacy: absolute binary + workspace in args
```

**Portable (default):** `command: "neuromesh"`, `args: ["mcp"]` — workspace from IDE env.

**Pinned:** use when PATH or auto-detection fails — absolute binary, `args: ["mcp", "<workspace>"]`, `NEUROMESH_WORKSPACE`.

Client config paths: [mcp.md](mcp.md#connect).

---

## Build from source

```bash
# Requires rustup 1.80+
git clone https://github.com/pinoox/neuromesh.git
cd neuromesh
cargo build --release --bin neuromesh --features embeddings

cargo install --git https://github.com/pinoox/neuromesh.git neuromesh-cli --bin neuromesh --features embeddings
```

---

## Update / uninstall

Re-run the installer or `cargo install --force --git …`. Restart the IDE so MCP does not keep an old process.

| How installed | Binary |
| :--- | :--- |
| `install.sh` | `~/.local/bin/neuromesh` |
| `install.ps1` | `%LOCALAPPDATA%\Programs\neuromesh\neuromesh.exe` |
| `cargo install` | `~/.cargo/bin/neuromesh` |

`which neuromesh` / `where.exe neuromesh` — delete duplicates or `cargo uninstall neuromesh-cli`. Remove the `neuromesh` block from MCP configs to disconnect.

---

## Migration from v0.8.x

| Removed (v0.9.0) | Replacement |
| :--- | :--- |
| `neuromesh config seed-engine …` | `neuromesh config engine fast\|hybrid\|deep` |
| `neuromesh config embeddings …` | `neuromesh config engine hybrid\|deep` + `embed rebuild` |
| `NEUROMESH_SEED_ENGINE` | `NEUROMESH_ENGINE` |
| `NEUROMESH_EMBEDDINGS` | `NEUROMESH_ENGINE=hybrid\|deep` |
| Client `keywords` / `expansion` on default fast | Pass prompt only; server expands concepts |
| Sidecar v4/v5 | `neuromesh embed rebuild` for v6 hierarchical |

---

## Layering summary

Effective config merges: global `~/.neuromesh/config.json` → project `nm.config.json` → env → MCP per-call overrides.

See also [engines.md](engines.md) (preset overview) · [cli.md](cli.md) · [quality.md](quality.md) · [graph-proxy.md](graph-proxy.md).
