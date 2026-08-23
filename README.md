<div align="center">

# NeuroMesh v0.5.0
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
3. Always ship the seed files, then fill outbound callees / inbound usages / imports under a **real fill budget**.
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
| Call edges | Tree-sitter + Hybrid LSP (many languages) | tree-sitter for Rust and TypeScript; regex fallback elsewhere. Impl- and field-aware unique resolve |
| Language bet | 158 grammars in one C binary | Depth on the languages agents actually edit here (Rust, TS/JS, Python, Vue, Go, …) |
| Index safety | Project-scoped store | Refuses home/drive roots; prefers git/cargo workspace; caps file count |

NeuroMesh does **not** claim 158 languages, Linux-kernel index times, or Cypher. Those are CBM’s strengths. NeuroMesh’s bet is: **fewer tokens, higher precision, one tool for the common path**.

---

## What v0.5 ships

v0.4 chose which files fit. v0.5 makes the packet a loop the agent can finish without Grep:

1. **Folds are real.** Skeletonization registers each `[neuromesh:fold]` body. `neuromesh_expand_fold` restores it from the registry (no disk re-read).
2. **Fill is tighter.** Soft crate caps (a third file from the same crate can enter if it still scores), giant files are skeletonized instead of dropped, and each callee file is scored once so a match-heavy function does not drown the packet in `graph.rs`.
3. **Parse is IDE-shaped for Rust and TypeScript.** tree-sitter sits in front of `AstAnalysisResult`; regex parsers remain the fallback. `self.activator.activate` resolves to `ContextActivator::activate`, not a same-named method in another crate. Ambiguous names stay `Likely` instead of being dropped.
4. **Gold is path-qualified and not only this repo.** `tests/gold_tasks.toml` plus five fixture repos under `tests/fixtures/`. Threshold: recall ≥ 0.8 **and** precision ≥ 0.4. `neuromesh eval` runs both.

`neuromesh_get_context` still seed-then-fills:

| Mode | Extra tokens on top of seeds | Extra files (soft per crate) |
| :--- | ---: | ---: |
| `MaxSavings` | 0 | 0 |
| `Balanced` | 8,000 | 2, overflow to 4 if the extra file still scores |
| `MaxQuality` | 16,000 | 3, overflow to 6 |

Packets include `path`, `why`, `line_range`, `folded_symbols`, and `seed_call_coverage`. QualityGate honors the requested mode unless the task is critical.

```bash
cargo test -p neuromesh-context gold_harness_on_neuromesh_repo -- --nocapture
cargo test -p neuromesh-context gold_harness_on_fixture_repos -- --nocapture
neuromesh eval
```

---

## What v0.4 shipped

v0.3 made the graph structural. v0.4 decided **which of those nodes actually fit in the packet**.

`neuromesh_get_context` no longer dumps a neighborhood. It:

1. Resolves prompt identifiers to seeds (and records misses instead of hiding them).
2. Always includes those seed files (after skeletonization). Seeds are not truncated to a fake packet cap.
3. Fills connectors on top of seeds — outbound `Calls`, inbound usages, and outbound `Imports` — under a **fill budget** by mode.

Docs and test fixtures stay out of the fill list. Physarum is off this hot path (it remains available as `solve_physarum_context` / spreading activation, not inside `get_context`). Every packet reports `budget.seed_tokens`, `fill_used` / `fill_cap`, seed resolutions, and a coverage claim (`no_recorded_gap` or `partial` when seeds were missed).

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

Activation no longer scores the entire graph. It seeds from prompt anchors, walks a bounded neighborhood, and fills connectors under the mode's fill budget.

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
neuromesh eval         # gold-task recall / packet size / fill budget on this repo
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

These numbers come from `cargo test -p neuromesh-graph indexes_real_neuromesh_repo_with_usable_graph` and the gold harness on this repository (debug build, Windows, 2026-08-23). They are not marketing estimates.

| Metric | Before (live MCP on a home-scoped index) | After v0.5 (this repo) |
| :--- | ---: | ---: |
| Indexed files | 11,564 (user profile) | **156** (workspace, `target/` ignored) |
| Graph nodes | 34,450 | **956** |
| Graph edges | 1,230,610 | **1,958** |
| Resolved `Calls` | not trustworthy | **558** |
| Resolved `Imports` | exploded fuzzy matches | **571** |
| `search_symbols("handle_tool_call")` | timed out | **<1 ms**, exact hit |
| `get_dependencies("handle_tool_call")` | 0 neighbors | **28** structural neighbors |
| `get_context` | timed out | seed-then-fill packet, coverage claim, modes differ |
| Full workspace index | unbounded | **1,202 ms** in the measured debug run |

Gold (`tests/gold_tasks.toml`, path-qualified) plus five fixture repos (`tests/fixtures/`):

- Known prompts must recall ≥ 0.8 **and** precision ≥ 0.4.
- `handle_tool_call extract intent` → `crates/neuromesh-mcp/src/tools.rs`, `signature.rs`, `activator.rs`.
- A missing symbol must surface as `seeds_missed`.
- Context activation stays under **50 ms** in the debug gold test.
- `neuromesh_expand_fold` restores a registered body without reading the disk.

### Grep after `get_context`

Measured on two real prompts in this repo by whether the gold files are already in the packet (if they are, Grep is not required to find them):

| Prompt | Gold files in packet | Grep still needed |
| :--- | ---: | ---: |
| How does `handle_tool_call` extract intent? | yes (recall 1.0) | **0** |
| Where is Physarum used? | yes (recall 1.0) | **0** |

That is the intended agent loop: `neuromesh_get_context` first, `neuromesh_expand_fold` if a folded body is required, Grep only when the coverage claim is `partial` or a seed was missed.

Unit tests that lock this in:

- Identifier extraction from a real prompt (`neuromesh_get_context` + `tools.rs`)
- tree-sitter Rust: calls stay inside the function; `self.activator.activate` carries a field hint
- Unique resolution does not explode edges; field receivers do not pick a same-named method in another crate
- Ranked search does not treat `"get_context"` as a match for every name contained in the query
- Context activator keeps the seed symbol; expand_fold roundtrips without disk
- Seed-then-fill selector: seeds always ship; callee files are scored once
- Gold harness on this repo and on five fixture repos
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
Seed files always ship (skeletonized)
  │
  ▼
Fill callees / usages / imports under fill_cap
  │
  ├─ MaxSavings: seeds only
  ├─ Balanced: +8k extra, soft crate cap
  └─ MaxQuality: +16k extra
  │
  ▼
Evidence packet → MCP client
  │
  └─ neuromesh_expand_fold restores a folded body from the registry
```

| Crate | Role |
| :--- | :--- |
| `neuromesh-parser` | tree-sitter Rust/TS + regex fallback extractors + prompt anchors |
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
