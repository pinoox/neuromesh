

# NeuroMesh

### Ship less context. Ship the right code.

Local-first [MCP](https://modelcontextprotocol.io/) context engine for **Cursor**, **VS Code**, **Claude**, **Codex**, and every MCP client. NeuroMesh indexes your repo into a graph, routes your prompt to the right symbols, and sends a **folded evidence packet** — not thousand-line file dumps.

![Release](https://img.shields.io/github/v/release/pinoox/neuromesh?style=flat-square&color=22c55e)
![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)
![CI](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml/badge.svg)
![MCP](https://img.shields.io/badge/MCP-stdio-5b21b6.svg?style=flat-square)
![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)

Cursor · VS Code · Claude · Codex · OpenCode · MiMo CLI · Antigravity · Kilo · Trae · Windsurf · Zed

[The pain](#the-pain) · [Fold](#dont-delete-fold) · [Galaxy](#3d-neural-galaxy) · [Measured](#what-we-actually-measured) · [Install](#install) · [Connect](#connect) · [Docs](docs/README.md) · [Site](https://pinoox.github.io/neuromesh/)



---

## The pain

You ask a simple question in a large project. The editor copies two or three **thousand-line files** and ships them to the model.

What you pay for:

1. **Tokens you never needed** — dollar cost on every turn
2. **Seconds of fake loading** while the window fills with helpers you will not touch
3. **Lost in the middle** — the model drowns in unrelated bodies and invents bugs

Today’s workarounds all leak in a different place:


| Approach                | What goes wrong                                                     |
| ----------------------- | ------------------------------------------------------------------- |
| Vector RAG              | Chunks smash functions. The shape of the code disappears.           |
| “Just attach the files” | The model sees everything and understands nothing.                  |
| A static code graph     | Better *map* — then it still pastes **full files** into the prompt. |


NeuroMesh is the missing step: **route first, then fold.** The graph is for finding the path. The packet is what the model actually reads.

---

## Don’t delete. Fold.

### Don’t delete the extra code. Fold it.

How does nature pack two metres of DNA into a nucleus without deleting a single letter?  
Not by throwing genes away — by **folding**.

Nature does not delete DNA to fit a nucleus. It **supercoils**.

NeuroMesh treats the syntax tree like a genetic strand in RAM:

- Functions you need stay **expressed** (exons) — real body, real lines  
- The rest collapse to a **one-line reversible intron**:

```c
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```

The agent still sees the *shape* of the file — signatures, imports, neighbors — without paying for every private helper. When a folded body is required, `neuromesh_expand_fold` unsplices it from a registry in memory. Nothing was deleted. Nothing needs a second grep of the disk.

> Structure stays. Tokens sleep. Wake a fold when you need it.

---

## Why it feels different


| What you get | What it means for you |
| ------------ | --------------------- |
| **Smart folding** | Relevant functions stay open; everything else collapses to one-line markers you can expand on demand. |
| **Shortest path routing** | Only the files your task needs — not the whole repo neighborhood. |
| **Learns from your edits** | Call `record_feedback` after a good fix; similar tasks route faster next time. |
| **Safety modes** | Balanced by default; auth and payment tasks automatically get more context. |
| **Live code graph** | Your repo indexed in RAM — functions, imports, and calls, not shredded text chunks. |

Curious about the biology metaphor? **[docs/nature.md](docs/nature.md)**

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


| Platform      | Binary                                            |
| ------------- | ------------------------------------------------- |
| macOS / Linux | `~/.local/bin/neuromesh`                          |
| Windows       | `%LOCALAPPDATA%\Programs\neuromesh\neuromesh.exe` |


Hybrid/deep embeddings (`neuromesh install embed minilm`), CBM proxy, monitor port: [docs/configuration.md](docs/configuration.md).

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

Per-client paths: [docs/mcp.md](docs/mcp.md).

---

## Agent loop

Pass the **user task as written** — any language. Default **`engine: fast`**: graph + server-assisted concept expansion; no keyword tables.

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

## 3D Neural Galaxy

`neuromesh monitor` is a live map of **your** project: packages at a glance, then the file graph, then symbols inside a module.

![Macro constellation of project subsystems in the 3D Neural Galaxy](docs/assets/galaxy-constellation.jpg)

Constellation — packages and subsystems

![3D Neural Galaxy file graph with Physarum slime tubes](docs/assets/galaxy-3d.jpg)

3D galaxy — files and call/import links

![Module zoom showing Core files and function symbols](docs/assets/galaxy-module.jpg)

Module zoom — files and symbols in one area of your codebase

Default URL: [http://127.0.0.1:8765](http://127.0.0.1:8765) · `neuromesh monitor` · port: `neuromesh port`

---

## Tools (MCP)


| Tool                        | Use                                  |
| --------------------------- | ------------------------------------ |
| **`get_context_packet`** | Main entry — folded evidence packet |
| `neuromesh_expand_fold`     | Restore one folded body              |
| `neuromesh_search_symbols`  | Ranked symbol search when seeds miss |
| `neuromesh_trace`           | Call / import chains                 |
| `neuromesh_record_feedback` | Strengthen paths you actually edited |


Full reference: [docs/mcp.md](docs/mcp.md).

---

## Everyday CLI

```bash
neuromesh index              # refresh graph after large changes
neuromesh status             # node / edge counts
neuromesh monitor            # 3D graph UI (see above)
neuromesh doctor --engine    # show retrieval preset
neuromesh config engine hybrid   # opt in to semantic search (needs embed install)
```

Command reference: [docs/cli.md](docs/cli.md).

---

## Languages

Rust, TypeScript, Python, Go, Java, Kotlin, PHP, C#, Dart, Swift, Ruby, and more via **tree-sitter**. Framework overlays for Laravel, Django, Next, Vue, Axum, Rails, Flutter, and others. Details: [docs/architecture.md](docs/architecture.md).

---

## What we actually measured

Savings are **per task**, after folding — not a marketing average. Run `neuromesh eval` on your own repo to see your numbers.

Example from a **650k-token monorepo** (release **v0.9.0**, default `engine: fast`):

| Task (plain language) | Mode | Full repo | Before fold | Packet sent | Saved vs repo | Extra greps | ms |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| Fix the MCP tool handler | balanced | 650,859 | 72,428 | 17,389 | **97.3%** | **0** | 22 |
| Trace graph routing code | balanced | 650,859 | 19,625 | 4,080 | **99.4%** | **0** | 12 |

Index on that project: **340 files · 552 ms**. Methodology and multilingual holdout: [docs/quality.md](docs/quality.md).

---

## Documentation


| Doc                                    | Start here when you want to…                     |
| -------------------------------------- | ------------------------------------------------ |
| [Agent guide](docs/agent-guide.md)     | Wire Cursor / VS Code / Claude to use NeuroMesh  |
| [MCP tools](docs/mcp.md)               | See what each tool returns                       |
| [CLI](docs/cli.md)                     | Commands for install, index, connect, monitor    |
| [Configuration](docs/configuration.md) | Switch engines, proxy, advanced tuning           |
| [Engines](docs/engines.md)             | `fast` vs `hybrid` vs `deep` in one page         |
| [Docs index](docs/README.md)           | Full map                                         |
| [Changelog](docs/CHANGELOG.md)         | What changed in v0.9.0                           |


MIT · [LICENSE](LICENSE)