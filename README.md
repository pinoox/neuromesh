

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

## Inspired by living systems


| Nature                    | In the engine                        | For the agent                                                                 |
| ------------------------- | ------------------------------------ | ----------------------------------------------------------------------------- |
| **DNA supercoiling**      | Genetic skeletonizer                 | Fold unused bodies; keep the map of the file                                  |
| **Physarum (slime mold)** | Steiner / shortest connecting tissue | Don’t flood the prompt — grow the cheapest path between seeds                 |
| **Synapses & STDP**       | Pheromone edges + `record_feedback`  | Paths you actually edited get stronger next time                              |
| **Cell membrane**         | QualityGate                          | Tight by default; auth / payment tasks open the membrane                      |
| **Mycelium**              | Hyphal prefetch                      | Warm the next hop before the second tool call                                 |
| **Neural mesh**           | Project graph in RAM                 | Files, functions, `Imports`, `Calls` — a nervous system, not a bag of strings |


**A graph in RAM, a short path, the rest dormant.** After seeds resolve, Physarum grows tubes between them (under 20ms). Fill respects the token cap. `record_feedback` is how synapses change the next packet.

Full metaphor map and crate wiring: **[docs/nature.md](docs/nature.md)**

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


Hybrid/deep embeddings, CBM proxy, monitor port: [docs/configuration.md](docs/configuration.md).

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

`neuromesh monitor` is the live mesh: subsystems as a constellation, then the file graph, then the symbols inside a module.

![Macro constellation of project subsystems in the 3D Neural Galaxy](docs/assets/galaxy-constellation.jpg)

Constellation — crates and subsystems

![3D Neural Galaxy file graph with Physarum slime tubes](docs/assets/galaxy-3d.jpg)

3D galaxy — files and synapses; Play slime grows Physarum tubes

![Module zoom showing Core files and function symbols](docs/assets/galaxy-module.jpg)

Module zoom — files and AST symbols in one crate

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
neuromesh eval               # token savings + recall on gold tasks
neuromesh doctor --engine    # show retrieval preset
```

Command reference: [docs/cli.md](docs/cli.md).

---

## Languages

Rust, TypeScript, Python, Go, Java, Kotlin, PHP, C#, Dart, Swift, Ruby, and more via **tree-sitter**. Framework overlays for Laravel, Django, Next, Vue, Axum, Rails, Flutter, and others. Details: [docs/architecture.md](docs/architecture.md).

---

## What we actually measured

Not a universal “99.6%” — that number was never a warranty. Savings are **per task**, after folding. Re-run on your repo: `neuromesh eval`.

On this repo (release **v0.9.0**, 650,859 workspace tokens):

| Task | Mode | WS tok | Selected | Packet | vs WS | vs selected | Recall | Prec | Grep | ms |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `handle_tool_call_intent` | balanced | 650859 | 72428 | 17389 | 97.3% | 76.0% | 1.00 | 0.75 | **0** | 22 |
| `physarum_usage` | balanced | 650859 | 19625 | 4080 | 99.4% | 79.2% | 1.00 | 0.50 | **0** | 12 |

`Selected` = raw token count before fold. `Packet` = after fold. `Grep` = 0 when every gold file is already in the packet.

Index snapshot from that run: **340 files · 3,161 nodes · 6,795 edges · 552 ms** (release). Full gates and multilingual holdout: [docs/quality.md](docs/quality.md).

---

## Documentation


| Doc                                    | For                                              |
| -------------------------------------- | ------------------------------------------------ |
| [Docs index](docs/README.md)           | Full map                                         |
| [Living systems](docs/nature.md)       | DNA, Physarum, STDP — mapped to crates           |
| [Agent guide](docs/agent-guide.md)     | Teach Cursor / VS Code / Claude to use NeuroMesh |
| [MCP tools](docs/mcp.md)               | Tool inputs and packet shape                     |
| [Configuration](docs/configuration.md) | Engines, proxy, advanced tuning                  |
| [Quality](docs/quality.md)             | Benchmarks and release gates                     |
| [Changelog](docs/CHANGELOG.md)         | v0.9.0                                           |


MIT · [LICENSE](LICENSE)