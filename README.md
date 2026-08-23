<div align="center">

# NeuroMesh

**A living index for your repo.** Give the agent the tissue it needs. Keep the rest folded.

Inspired by slime molds, synapses, and gene splicing — implemented as a local MCP server in Rust.

[![Release](https://img.shields.io/github/v/release/pinoox/neuromesh?style=flat-square&color=22c55e)](https://github.com/pinoox/neuromesh/releases/latest)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![CI](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml/badge.svg)](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml)
[![MCP](https://img.shields.io/badge/MCP-stdio-5b21b6.svg?style=flat-square)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

Local-first [Model Context Protocol](https://modelcontextprotocol.io/) server for Cursor, Claude, Cline, and friends.

[Install](#install) · [Connect](#connect) · [Living systems](#inspired-by-living-systems) · [How it works](#how-it-works) · [Docs](docs/README.md)

</div>

---

## Why this exists

A coding agent that greps and reads file-by-file is slow, expensive, and often wrong. It fills the context window with helpers it will never touch, then loses the function it actually needed.

NeuroMesh sits between the agent and your repository and answers one question:

> For **this** task, which files and symbols are required — and what can stay folded?

You get an **evidence packet**: ranked files, skeletonized source, why each node is there, and reversible folds for the bodies that can wait. One call replaces a grep loop.

Everything runs on your machine. No API key. No cloud index.

---

## Inspired by living systems

Biology already solved “do not dump the whole organism into every thought.” NeuroMesh borrows those tricks — with names you can find in the crates — so the project feels like something you *want* to extend, not another JSON graph.

| In nature | In NeuroMesh | What it does for the agent |
| :--- | :--- | :--- |
| **Physarum polycephalum** (slime mold) | `PhysarumSolver` | Grows the cheapest tissue that still connects the seeds — a Steiner-like path, not a dump of every neighbor |
| **Synapses & STDP** | pheromone edges + `record_feedback` | Paths the agent actually used get stronger (LTP). Dead ends fade (LTD) |
| **Exon / intron splicing** | `CodeSkeletonizer` | Keep the expressed symbols (exons). Fold untargeted bodies into reversible `[neuromesh:fold]` introns |
| **Cell membrane / osmosis** | QualityGate | `max_savings` is tight. Auth and payment tasks open the membrane (`max_quality`) |
| **Mycelium** | hyphal prefetch cache | Warm the next hop before the agent asks |
| **Neural mesh** | project graph | Files, functions, `Imports`, `Calls` — a nervous system for the repo, not a bag of strings |

The everyday loop is still boring in the best way: **seed files always ship, fill under a real budget, fold the rest.** The biomimetic layer is how we *name and grow* that loop — and where contributors can play.

Want to work on a solver, a plasticity rule, or a new language extractor? Start at [docs/nature.md](docs/nature.md) and [docs/contributing.md](docs/contributing.md).

---

## Install

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/pinoox/neuromesh/main/install.sh | bash
```

**Windows**

```powershell
irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex
```

**From source**

```bash
git clone https://github.com/pinoox/neuromesh.git
cd neuromesh
cargo build --release --bin neuromesh
```

**Cargo**

```bash
cargo install --git https://github.com/pinoox/neuromesh.git neuromesh-cli --bin neuromesh
```

Check the install:

```bash
neuromesh doctor
neuromesh connect    # prints MCP JSON for the binary on your PATH
```

---

## Connect

Stdio is the path IDEs expect. Paste this and point `command` at your `neuromesh` binary if it is not on `PATH`:

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

| Client | Where to put it |
| :--- | :--- |
| Cursor | `.cursor/mcp.json` or Settings → MCP |
| VS Code / Copilot | `.vscode/mcp.json` |
| Claude Desktop | `claude_desktop_config.json` |
| Claude Code | `claude mcp add neuromesh -- neuromesh mcp` |
| Cline | `cline_mcp_settings.json` |
| Zed | `~/.config/zed/settings.json` → `context_servers` |

NeuroMesh discovers the git or Cargo root from the working directory. It **will not** index `$HOME` or a drive root.

Live graph UI: `neuromesh monitor` → [http://127.0.0.1:8765](http://127.0.0.1:8765).

---

## How it works

```mermaid
flowchart LR
  A[Your prompt] --> B[Extract identifiers]
  B --> C[Resolve on the graph]
  C --> D[Seed files always ship]
  D --> E[Fill callees under budget]
  E --> F[Skeleton + folds]
  F --> G[Evidence packet]
  G --> H[expand_fold if needed]
```

1. **Read the task.** Identifiers, file hints, and intent come out of the prompt as written — PascalCase survives.
2. **Resolve uniquely.** `Contains`, `Imports`, and `Calls` edges exist only when the target is unique (same file, imported files, impl/field, or a single global hit). Ambiguous names stay unlinked or `Likely`, never a million fake edges.
3. **Ship seeds, then fill.** The files that own those symbols always go in. Extra callees and imports fill a real token budget by mode.
4. **Splice.** Untargeted function bodies become intron markers (`[neuromesh:fold:…]`). The original exon/intron split is reversible — `neuromesh_expand_fold` restores a body from the registry, no second disk read.

Tell the agent this loop:

```
neuromesh_get_context(task)
  → neuromesh_expand_fold if a folded body is required
  → neuromesh_trace if you need callers
  → neuromesh_record_feedback after a successful edit
```

Grep is the exception, not the opening move.

### Modes

| Mode | Extra tokens on top of seeds | When to use |
| :--- | ---: | :--- |
| `max_savings` | 0 | Small, obvious edits |
| `balanced` | 8,000 | Default |
| `max_quality` | 16,000 | Refactors, auth, anything you cannot miss |

Seeds are never truncated to fake a small packet. A large target function can exceed the fill cap; that is honest.

---

## Tools

| Tool | Role |
| :--- | :--- |
| **`neuromesh_get_context`** | Primary. Evidence packet: files, skeletons, symbols, folds, coverage |
| **`neuromesh_expand_fold`** | Restore a folded body by `fold_id` |
| **`neuromesh_get_file_skeleton`** | Skeletonize one file with chosen exons open |
| **`neuromesh_search_symbols`** | Ranked search (exact / prefix / camel-snake / path) |
| **`neuromesh_get_dependencies`** | Typed neighbors of a name or path |
| **`neuromesh_trace`** | Inbound / outbound call and import chains |
| **`neuromesh_analyze_impact`** | Blast radius for a symbol or file |
| **`neuromesh_get_architecture`** | Languages, packages, entry points |
| **`neuromesh_get_project_memory`** | Facts seeded from manifests and docs |
| **`neuromesh_record_feedback`** | Reinforce the path the agent actually used |
| **`neuromesh_get_stats`** | Node and edge counts |

Full schemas: [docs/mcp.md](docs/mcp.md). CLI: [docs/cli.md](docs/cli.md).

---

## What you get back

Each file in the packet carries:

- **`path`** — full path, not a basename guess
- **`why`** — seed, callee, import, or fill score
- **`line_range`** and **`folded_symbols`**
- **`seed_call_coverage`** — how many of the seed’s calls landed in the packet

Coverage is `no_recorded_gap` or `partial` (missed seeds). If it is partial, Grep is fair. If it is not, start from the packet.

---

## Languages

Depth where agents actually edit: **Rust** and **TypeScript / JavaScript** go through tree-sitter (function spans, impl parents, field-aware `self.foo.bar()`). Python, Vue, Go, and others use scoped regex extractors. Ambiguous names are not “resolved” by hope.

---

## Quality

Locked by a gold harness on this repo and five fixture projects (`tests/fixtures/`). Recall ≥ 0.8 and precision ≥ 0.4. Folds round-trip without disk.

On two prompts in this workspace — *How does `handle_tool_call` extract intent?* and *Where is Physarum used?* — the gold files were already in the packet (**Grep still needed: 0**).

Measured index of this repo (debug, 2026-08-23): **156 files · 956 nodes · 1,958 edges · 558 calls · 571 imports · ~1.2 s**.

How to re-measure: [docs/quality.md](docs/quality.md).

```bash
neuromesh eval
cargo test --workspace
```

---

## Docs

| | |
| :--- | :--- |
| [Living systems](docs/nature.md) | Physarum, STDP, exons, osmosis — mapped to code |
| [Architecture](docs/architecture.md) | Pipeline, crates, guarantees |
| [MCP tools](docs/mcp.md) | Tool list and agent loop |
| [CLI](docs/cli.md) | `mcp`, `index`, `eval`, `monitor`, … |
| [Quality](docs/quality.md) | Gold tasks, eval, numbers |
| [HTTP monitor](docs/api.md) | Local UI and SSE |
| [Contributing](docs/contributing.md) | Tests, clippy, language extractors |
| [Changelog](docs/CHANGELOG.md) | What shipped in 0.5 |

---

## License

MIT. See [LICENSE](LICENSE).
