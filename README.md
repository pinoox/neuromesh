<div align="center">

# NeuroMesh

### Ship less context. Ship the right code.

Local-first [MCP](https://modelcontextprotocol.io/) context engine for **Cursor**, **VS Code**, **Claude**, **Codex**, and every MCP client. NeuroMesh indexes your repo into a graph, routes your prompt to the right symbols, and sends a **folded evidence packet** — not thousand-line file dumps.

[![Release](https://img.shields.io/github/v/release/pinoox/neuromesh?style=flat-square&color=22c55e)](https://github.com/pinoox/neuromesh/releases/latest)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![CI](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml/badge.svg)](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml)
[![MCP](https://img.shields.io/badge/MCP-stdio-5b21b6.svg?style=flat-square)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

[Install](#install) · [Connect](#connect) · [Agent loop](#agent-loop) · [Docs](docs/README.md) · [Site](https://pinoox.github.io/neuromesh/)

</div>

---

## Why NeuroMesh

You ask one question. The editor attaches two **massive files**. You pay for helpers you never touch, wait for fake loading, and the model still misses the function you meant.

NeuroMesh **routes first, then folds**: a structural graph finds the path; the packet is what the model reads. Unused bodies collapse to one-line reversible markers — wake them with `neuromesh_expand_fold` when needed.

---

## Install

**Pre-built binary** (no Rust required). v0.9.0 defaults to **`engine: fast`** — instant graph index, no ONNX warm at startup.

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/pinoox/neuromesh/main/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex
```

Then from your **project root**:

```bash
neuromesh doctor          # verify binary and workspace
neuromesh connect         # write MCP configs (Cursor, VS Code, Claude, …)
neuromesh index           # build the graph (<30s typical)
```

Restart your IDE so MCP picks up the new server. Re-run the installer to **update** — then `neuromesh -V` should show **v0.9.0**.

| Platform | Binary |
| :--- | :--- |
| macOS / Linux | `~/.local/bin/neuromesh` |
| Windows | `%LOCALAPPDATA%\Programs\neuromesh\neuromesh.exe` |

Build from source, hybrid/deep embeddings, CBM proxy, monitor port, file caps: [docs/configuration.md](docs/configuration.md).

---

## Connect

NeuroMesh speaks **MCP over stdio** — what your IDE launches in the background.

```bash
neuromesh connect --global --agent-rules   # recommended once per machine
```

That registers the server **and** copies the agent rule so the IDE actually calls NeuroMesh instead of raw `Read` / Grep.

**Manual** (when `neuromesh` is on PATH) — paste into `~/.cursor/mcp.json`:

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

Workspace is auto-detected from IDE env vars. Per-client paths and troubleshooting: [docs/mcp.md](docs/mcp.md).

---

## Agent loop

Pass the **user task as written** — any language. No keyword tables on the default fast engine.

```
get_context_packet(query / task_description / prompt / task)
  → check coverage.claim and retrieval.resolution_tier
  → neuromesh_search_symbols or neuromesh_expand_gap if seeds missed
  → neuromesh_expand_fold when a folded body is required
  → neuromesh_trace for callers and blast radius
  → neuromesh_record_feedback after a successful edit
```

Teach every IDE: [docs/agent-guide.md](docs/agent-guide.md) · Cursor template: [docs/agent-rule.mdc](docs/agent-rule.mdc).

---

## What the model sees

Signatures, imports, and neighbors stay visible. Private helpers fold to one line:

```c
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```

Each packet includes `coverage`, token counts, skeleton `code`, and fold ids (no bodies until expanded).

---

## Tools (MCP)

| Tool | Use |
| :--- | :--- |
| **`get_context_packet`** | Main entry — folded evidence packet |
| `neuromesh_expand_fold` | Restore one folded body |
| `neuromesh_search_symbols` | Ranked symbol search when seeds miss |
| `neuromesh_trace` | Call / import chains |
| `neuromesh_record_feedback` | Strengthen paths you actually edited |

Full reference: [docs/mcp.md](docs/mcp.md).

---

## Everyday CLI

```bash
neuromesh index              # refresh graph after large changes
neuromesh status             # node / edge counts
neuromesh monitor            # 3D graph UI → http://127.0.0.1:8765
neuromesh eval               # token savings + recall on gold tasks
neuromesh doctor --engine    # show retrieval preset
```

Command reference: [docs/cli.md](docs/cli.md).

---

## Languages

Rust, TypeScript, Python, Go, Java, Kotlin, PHP, C#, Dart, Swift, Ruby, and more via **tree-sitter**. Framework overlays for Laravel, Django, Next, Vue, Axum, Rails, Flutter, and others. Details: [docs/architecture.md](docs/architecture.md).

---

## Documentation

| Doc | For |
| :--- | :--- |
| [Docs index](docs/README.md) | Full map |
| [Agent guide](docs/agent-guide.md) | Teach Cursor / VS Code / Claude to use NeuroMesh |
| [MCP tools](docs/mcp.md) | Tool inputs and packet shape |
| [CLI](docs/cli.md) | Terminal commands |
| [Configuration](docs/configuration.md) | Engines, proxy, env vars, advanced tuning |
| [Quality](docs/quality.md) | Benchmarks and release gates |
| [Architecture](docs/architecture.md) | Pipeline and crate map |
| [Changelog](docs/CHANGELOG.md) | v0.9.0 |

MIT · [LICENSE](LICENSE)
