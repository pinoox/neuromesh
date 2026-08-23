<div align="center">

# NeuroMesh

### Don’t delete the extra code. Fold it.

How does nature pack two metres of DNA into a nucleus without deleting a single letter?  
Not by throwing genes away — by **folding**.

NeuroMesh does the same thing to your repository: a neural graph in RAM, reversible one-line folds, and an evidence packet instead of three thousand-line files dumped into Cursor or Claude.

[![Release](https://img.shields.io/github/v/release/pinoox/neuromesh?style=flat-square&color=22c55e)](https://github.com/pinoox/neuromesh/releases/latest)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![CI](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml/badge.svg)](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml)
[![MCP](https://img.shields.io/badge/MCP-stdio-5b21b6.svg?style=flat-square)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

Local-first [MCP](https://modelcontextprotocol.io/) · Cursor · Claude · VS Code · Windsurf · Cline · Zed

[The pain](#the-pain) · [The idea](#dont-delete-fold) · [Install](#install) · [Connect](#connect) · [Docs](docs/README.md)

</div>

---

## The pain

You ask a simple question in a large project. The editor copies two or three **thousand-line files** and ships them to the model.

What you pay for:

1. **Tokens you never needed** — dollar cost on every turn  
2. **Seconds of fake loading** while the window fills with helpers you will not touch  
3. **Lost in the middle** — the model drowns in unrelated bodies and invents bugs

Today’s workarounds all leak in a different place:

| Approach | What goes wrong |
| :--- | :--- |
| Vector RAG | Chunks smash functions. The shape of the code disappears. |
| “Just attach the files” | The model sees everything and understands nothing. |
| A static code graph | Better *map* — then it still pastes **full files** into the prompt. |

NeuroMesh is the missing step: **route first, then fold.** The graph is for finding the path. The packet is what the model actually reads.

---

## Don’t delete. Fold.

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

| Nature | In the engine | For the agent |
| :--- | :--- | :--- |
| **DNA supercoiling** | Genetic skeletonizer | Fold unused bodies; keep the map of the file |
| **Physarum (slime mold)** | Steiner / shortest connecting tissue | Don’t flood the prompt with the whole neighborhood — grow the cheapest path between seeds |
| **Synapses & STDP** | Pheromone edges + `record_feedback` | Paths you actually edited get stronger next time |
| **Cell membrane** | QualityGate | Tight by default; auth / payment tasks open the membrane |
| **Mycelium** | Hyphal prefetch | Warm the next hop before the second tool call |
| **Neural mesh** | Project graph in RAM | Files, functions, `Imports`, `Calls` — a nervous system, not a bag of strings |

The philosophy of the social pitch is right: **a graph in RAM, a short path, the rest dormant.** The shipped `get_context` loop is seed-then-fill (unique resolve, real token budget, then splice). The slime-mold solver lives on the graph for Steiner-style experiments. Both share one rule: *do not dump the organism into every thought.*

Play with the metaphors and the crates: [docs/nature.md](docs/nature.md).

---

## How a turn actually goes

```mermaid
flowchart LR
  P[Prompt] --> I[Identifiers]
  I --> G[Graph in RAM]
  G --> S[Seed files]
  S --> F[Fill the path]
  F --> X[Exon / intron splice]
  X --> E[Evidence packet]
  E --> W[expand_fold if needed]
```

1. **Read the task** as written. `handle_tool_call` survives; it is not lowercased into mush.  
2. **Resolve on the mesh.** Edges exist when the target is unique. Ambiguous names stay sleepy — never a million fake links.  
3. **Ship seeds, fill the path.** The files that own those symbols always go in. Callees and imports fill a **real** budget (`balanced` = 8k extra tokens, not “the whole crate”).  
4. **Splice.** Untargeted bodies become fold markers. Coverage tells you if a seed was missed — only then is Grep fair.

Tell the agent:

```
neuromesh_get_context(task)
  → neuromesh_expand_fold if a body is still folded
  → neuromesh_trace for callers
  → neuromesh_record_feedback after a good edit
```

### Modes (the membrane)

| Mode | Extra tokens on top of seeds | Feel |
| :--- | ---: | :--- |
| `max_savings` | 0 | Tiny, obvious edits |
| `balanced` | 8,000 | Default |
| `max_quality` | 16,000 | Refactors, auth, “don’t you dare miss it” |

Seeds are never truncated to fake a small packet.

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

```bash
# From source
git clone https://github.com/pinoox/neuromesh.git
cd neuromesh
cargo build --release --bin neuromesh

# Or Cargo
cargo install --git https://github.com/pinoox/neuromesh.git neuromesh-cli --bin neuromesh
```

```bash
neuromesh doctor
neuromesh connect    # MCP JSON for the binary on your PATH
```

---

## Connect

Native **MCP stdio** — what Cursor, Claude, VS Code, Windsurf, Cline, and Zed actually launch:

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

| Client | Config |
| :--- | :--- |
| Cursor | `.cursor/mcp.json` or Settings → MCP |
| VS Code / Copilot | `.vscode/mcp.json` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` |
| Claude Desktop | `claude_desktop_config.json` |
| Claude Code | `claude mcp add neuromesh -- neuromesh mcp` |
| Cline | `cline_mcp_settings.json` |
| Zed | `context_servers` in settings |

It finds the git / Cargo / `package.json` root. It **refuses** `$HOME` and drive roots (that is how you accidentally index 11k junk files).

**3D galaxy UI** of the live graph: `neuromesh monitor` → [http://127.0.0.1:8765](http://127.0.0.1:8765).

---

## Tools

| Tool | Role |
| :--- | :--- |
| **`neuromesh_get_context`** | The product. Evidence packet: skeletons, why, folds, coverage |
| **`neuromesh_expand_fold`** | Wake one intron by `fold_id` — no disk grep |
| **`neuromesh_get_file_skeleton`** | Fold one file with chosen exons open |
| **`neuromesh_search_symbols`** | Ranked search |
| **`neuromesh_get_dependencies`** | Typed neighbors |
| **`neuromesh_trace`** | Call / import chains |
| **`neuromesh_analyze_impact`** | Blast radius |
| **`neuromesh_get_architecture`** | Languages, packages, entry points |
| **`neuromesh_record_feedback`** | Synaptic learning on the path you used |
| **`neuromesh_get_project_memory`** | Facts from manifests and docs |
| **`neuromesh_get_stats`** | Mesh size |

Each file in the packet has `path`, `why`, `line_range`, `folded_symbols`, and `seed_call_coverage`. Details: [docs/mcp.md](docs/mcp.md).

Rust and TypeScript go through **tree-sitter**. Python, Vue, Go and friends use scoped extractors. Ambiguous names are not “resolved” by hope.

---

## What we actually measured

Not a universal “99.6%” — that number was never a warranty. Savings are **per task**, after folding, versus dumping the workspace.

On this repo (debug, 2026-08-23):

- Gold prompts *How does `handle_tool_call` extract intent?* and *Where is Physarum used?* — gold files already in the packet → **Grep still needed: 0**
- Recall ≥ 0.8 and precision ≥ 0.4 locked on this repo **and** five fixture projects
- Packet activation **&lt; 50 ms** in the debug gold test; symbol search **&lt; 1 ms**
- Index: **156 files · 956 nodes · 1,958 edges · ~1.2 s**

Re-run it yourself: `neuromesh eval` · [docs/quality.md](docs/quality.md).

---

## Docs

| | |
| :--- | :--- |
| [Living systems](docs/nature.md) | DNA, Physarum, STDP — mapped to crates |
| [Architecture](docs/architecture.md) | Pipeline and guarantees |
| [MCP](docs/mcp.md) · [CLI](docs/cli.md) | Tools and commands |
| [Quality](docs/quality.md) | Gold, eval, numbers |
| [Contributing](docs/contributing.md) | Come build a solver or a language |
| [Changelog](docs/CHANGELOG.md) | 0.5 |

MIT · [LICENSE](LICENSE)
