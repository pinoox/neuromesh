<div align="center">

# NeuroMesh v0.4.0
### Task-conditioned context engine for AI coding agents

[![Latest Release](https://img.shields.io/github/v/release/pinoox/neuromesh?style=flat-square&color=brightgreen&label=Release)](https://github.com/pinoox/neuromesh/releases/latest)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![CI](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml/badge.svg)](https://github.com/pinoox/neuromesh/actions/workflows/ci.yml)
[![Model Context Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg?style=flat-square&logo=anthropic)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

<p align="center">
  <b>Do not dump the repo. Do not ask the agent to rediscover it. Deliver the evidence packet.</b>
</p>

[Quick Start](#quick-start) • [Why this is not another code graph](#why-neuromesh) • [MCP Tools](#mcp-tools) • [Measured quality](#measured-quality)

</div>

---

## What NeuroMesh is

NeuroMesh is a **local-first MCP server** written in Rust. It sits between a coding agent and the repository and answers one question well:

> Given this task, which files and symbols are actually required — and can the rest stay folded?

That is a different product from a general knowledge-graph MCP.

[codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) is a strong **query engine**: index once, then call 15 tools (`search_graph`, `trace_path`, `query_graph`, `get_architecture`, …). The agent has to know which tool to pick.

NeuroMesh is a **context engine**:

1. Extract real identifiers, file hints, and intent from the prompt.
2. Resolve them uniquely against a structural graph (`Contains`, `Imports`, `Calls`).
3. Walk only the neighborhood (Physarum on that subgraph, not the whole repo).
4. Return a compact **evidence packet**: skeletonized files, ranked symbols, and why each node was included.

One `neuromesh_get_context` call is meant to replace a file-by-file grep/read loop. Precision tools (`search`, `trace`, `impact`, `architecture`) exist when the agent needs a second look.

All processing is local. No API key. Transport is MCP stdio (recommended) or the monitor SSE endpoint.

---

## Why NeuroMesh

| Problem in naive agent workflows | What NeuroMesh actually does |
| :--- | :--- |
| Dumping whole files into the prompt | Returns an evidence packet with ranked files + symbols |
| Lost-in-the-middle from 25k–120k tokens | Folds untargeted function bodies into reversible `[neuromesh:fold]` markers |
| Graph tools that time out on large indexes | Ranked search and neighborhood activation — no full-graph scan |
| Fake “learning” and empty project memory | STDP only on touched paths; memory is seeded from `Cargo.toml`, docs, and crate layout |
| Ambiguous name matching that creates millions of edges | Unique / import-aware resolution. Ambiguous names stay unlinked |

### How this differs from codebase-memory-mcp

| | codebase-memory-mcp | NeuroMesh |
| :--- | :--- | :--- |
| Product | Persistent knowledge graph + 15 query tools | Task-conditioned evidence packet + 11 MCP tools |
| Primary call | Agent chooses `search_graph` / `trace_path` / Cypher | `neuromesh_get_context` |
| Code returned | Full snippets by qualified name | Skeleton with reversible folds |
| Call edges | Tree-sitter + Hybrid LSP (many languages) | Scoped call extraction + import-aware unique resolve |
| Language bet | 158 grammars in one C binary | Depth on the languages agents actually edit here (Rust, TS/JS, Python, Vue, Go, …) |
| Index safety | Project-scoped store | Refuses home/drive roots; prefers git/cargo workspace; caps file count |

NeuroMesh does **not** claim 158 languages, Linux-kernel index times, or Cypher. Those are CBM’s strengths. NeuroMesh’s bet is: **fewer tokens, higher precision, one tool for the common path**.

---

## What v0.4 ships

v0.3 made the graph structural. v0.4 decides **which of those nodes actually fit in the packet**.

`neuromesh_get_context` no longer dumps a neighborhood. It:

1. Resolves prompt identifiers to seeds (and records misses instead of hiding them).
2. Takes a Steiner union of proven `Calls` / `Imports` connectors around those seeds.
3. Greedy-fills remaining neighborhood nodes under a **token budget** by mode:

| Mode | Token cap |
| :--- | ---: |
| `MaxSavings` | 900 |
| `Balanced` | 2,500 |
| `MaxQuality` | 6,000 |

Physarum is off this hot path. Every packet reports `budget.used` / `budget.cap`, seed resolutions, and a coverage claim (`no_recorded_gap` or `partial` when seeds were missed).

Quality is locked by a gold harness on this repository (`tests/gold_tasks.toml`):

- Known prompts must recall ≥ 80% of gold files (`handle_tool_call` → `tools.rs` / `signature.rs` / `activator.rs`).
- A missing symbol must surface as `seeds_missed`, not a silent empty packet.
- Context activation stays under **50 ms** in the debug gold test.

```bash
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
```

---

## What v0.3 changed in the core

The previous core looked biomimetic on paper and failed on a live MCP session:

- `neuromesh_get_context` and `neuromesh_search_symbols` timed out.
- `neuromesh_get_dependencies("neuromesh_get_context")` returned **0 neighbors**.
- Search used bidirectional `contains`, so short tokens matched almost every node.
- Import/call ingest linked every fuzzy name match → **1.2M edges** on a home-directory index, **0** high-conductance synapses.
- Task intent was hardcoded to ecommerce entities (`Cart`, `ProductCard`) and lowercased the prompt before looking for PascalCase — so real identifiers never survived.
- File bodies were not stored, so skeletonization never ran on graph nodes.
- Project memory was empty unless the repo happened to be a Vue shop.

v0.3 replaces that with a two-pass structural index:

1. **Extract** symbols, grouped `use`/`import` trees, and calls scoped to the current function.
2. **Link once** after every file exists: unique name, else unique-in-imported-files, else no edge.

Activation no longer scores the entire graph. It seeds from prompt anchors, walks a bounded neighborhood, and optionally runs Physarum on that subgraph only.

---

## Quick Start

### Installers

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/pinoox/neuromesh/main/install.sh | bash
```

```powershell
# Windows
irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex
```

```bash
# From source
git clone https://github.com/pinoox/neuromesh.git
cd neuromesh
cargo build --release --bin neuromesh
```

```bash
# Cargo
cargo install --git https://github.com/pinoox/neuromesh.git neuromesh-cli --bin neuromesh
```

### Run

```bash
neuromesh mcp          # stdio MCP server (what Cursor / Claude / Cline launch)
neuromesh monitor      # Web UI + SSE on http://127.0.0.1:8765
neuromesh index        # build the graph + seed project memory
neuromesh doctor       # local diagnostics
neuromesh connect      # print ready-to-paste MCP JSON
```

The MCP process discovers the git/cargo root from its working directory and **will not index** `$HOME` or a drive root. That was the live failure mode that produced a 11k-file “yoose” graph.

---

## Connect any MCP client

Stdio (recommended):

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

| Client | Config path |
| :--- | :--- |
| Cursor | `.cursor/mcp.json` or Settings → MCP |
| VS Code / Copilot | `.vscode/mcp.json` |
| Claude Desktop | `claude_desktop_config.json` |
| Claude Code | `claude mcp add neuromesh -- neuromesh mcp` |
| Cline | `cline_mcp_settings.json` |
| Roo Code | `~/.roo/mcp.json` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` |
| Continue | `~/.continue/config.json` (`modelContextProtocolServers`) |
| Zed | `~/.config/zed/settings.json` (`context_servers`) |

Remote / multi-agent: `neuromesh monitor` then `GET http://127.0.0.1:8765/sse` and `POST http://127.0.0.1:8765/mcp`.

---

## MCP tools

| Tool | What it returns |
| :--- | :--- |
| **`neuromesh_get_context`** | Evidence packet: intent, identifiers, skeletonized files, ranked symbols, fold hints |
| **`neuromesh_get_file_skeleton`** | One file with untargeted functions folded |
| **`neuromesh_expand_fold`** | Restore a folded body from the reversible registry |
| **`neuromesh_search_symbols`** | Ranked exact / prefix / camel-snake token / path search |
| **`neuromesh_get_dependencies`** | Resolves a name or path, then returns typed neighbors |
| **`neuromesh_trace`** | Inbound / outbound / both call and import chains |
| **`neuromesh_analyze_impact`** | Blast radius and risk for a symbol or file |
| **`neuromesh_get_architecture`** | Languages, packages, entry points, degree hotspots |
| **`neuromesh_get_project_memory`** | Facts seeded from manifests and docs |
| **`neuromesh_record_feedback`** | STDP on the nodes the agent actually touched |
| **`neuromesh_get_stats`** | Node/edge counts, resolved calls/imports |

Typical agent loop:

```
neuromesh_get_context(task_description)
  → if a folded body is required: neuromesh_expand_fold
  → if you need callers: neuromesh_trace
  → after a successful edit: neuromesh_record_feedback
```

---

## Measured quality

These numbers come from `cargo test -p neuromesh-graph indexes_real_neuromesh_repo_with_usable_graph` on this repository (debug build, Windows). They are not marketing estimates.

| Metric | Before (live MCP on a home-scoped index) | After v0.3–v0.4 (this repo) |
| :--- | ---: | ---: |
| Indexed files | 11,564 (user profile) | **139** (workspace, `target/` ignored) |
| Graph nodes | 34,450 | **872** |
| Graph edges | 1,230,610 | **1,555** |
| Resolved `Calls` | not trustworthy | **344** |
| Resolved `Imports` | exploded fuzzy matches | **444** |
| `search_symbols("handle_tool_call")` | timed out | **<1 ms**, exact hit |
| `get_dependencies("neuromesh_get_context")` | 0 neighbors | name resolves; structural neighbors exist |
| `get_context` | timed out (full-graph + Physarum on 1.2M edges) | token-budget packet (`steiner_greedy`), coverage claim |
| Full workspace index | unbounded | **519 ms** in the measured debug run |

Unit tests that lock this in:

- Identifier extraction from a real prompt (`neuromesh_get_context` + `tools.rs`)
- Rust parser: functions, grouped `use`, scoped calls
- Unique resolution does not explode edges
- Ranked search does not treat `"get_context"` as a match for every name contained in the query
- Context activator keeps the seed symbol and stays under a small node budget
- Steiner-greedy selector beats “first five files” under a token cap
- Gold harness: recall ≥ 0.8, missing seeds reported, packet under 50 ms
- Real-repo index + search + trace + architecture

```bash
cargo test --workspace
cargo test -p neuromesh-graph indexes_real_neuromesh_repo_with_usable_graph -- --nocapture
```

Skeletonization still folds untargeted helpers (see `CodeSkeletonizer` tests). Token reduction is **per file and per task**, not a universal 99.6% claim.

---

## Architecture

```
Prompt
  │
  ▼
Task anchors (identifiers, paths, intent)
  │
  ▼
Unique / import-aware graph resolve
  │
  ▼
Bounded neighborhood walk
  │
  ├─ Physarum Steiner on the subgraph only
  └─ Hebbian STDP on feedback, not on every edge
  │
  ▼
Genetic skeleton (exons kept, introns folded)
  │
  ▼
Osmotic budget gate
  │
  ▼
Evidence packet → MCP client
```

| Crate | Role |
| :--- | :--- |
| `neuromesh-parser` | Structural extractors + prompt anchors |
| `neuromesh-graph` | Two-pass ingest, ranked search, trace, impact, architecture |
| `neuromesh-task` | Intent + identifier extraction |
| `neuromesh-context` | Neighborhood activation, token-budget selector, gold harness, skeletonizer |
| `neuromesh-memory` | Project facts seeded from the repo |
| `neuromesh-mcp` | MCP JSON-RPC 2.0 over stdio |
| `neuromesh-cli` | `mcp`, `monitor`, `index`, `doctor`, `connect` |

Details: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Web UI

`neuromesh monitor` serves `http://127.0.0.1:8765`:

- 2D/3D graph of the indexed workspace
- Telemetry for token reduction and graph density
- English / Persian UI toggle

---

## Contributing

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

New language support should add a scoped extractor (symbols + imports + calls) and a unique-resolve test — not a fuzzy edge dump.

---

## License

MIT. See [LICENSE](LICENSE).
