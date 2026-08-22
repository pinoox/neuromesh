# NeuroMesh V2 — Nature-Inspired Neural Context Runtime

## 1. System Vision & Core Biomimetic Paradigm

NeuroMesh is a local-first, ultra-high-performance neural context runtime written in **Rust**. It serves as an intelligent intermediary layer between AI coding agents (such as Cursor, Claude Code, Codex, Gemini CLI, Hermes, OpenCode, Aider, and any OpenAI-compatible client) and LLM inference providers (OpenAI, Anthropic, Google, OpenRouter, and Local GGUF).

### 1.1 The Biomimetic Paradigm

Traditional AI agents pump unbounded history, full files, and sprawling tool outputs into an LLM, leading to massive token waste, latency degradation, context distraction, and steep API costs.

NeuroMesh introduces a unified biologically inspired architecture modeled after living biological systems:

- **Physarum Polycephalum (Slime Mold) Network Solver**: Finds the optimal minimal Steiner context subgraph connecting multiple seed symbols via Hagen-Poiseuille cytoplasmic fluid flux simulation ($Q_{ij} = \frac{D_{ij}}{L_{ij}}(p_i - p_j)$), dynamically pruning atrophied branches.
- **Synaptic STDP & Hebbian Plasticity**: Applies Spike-Timing-Dependent Plasticity (STDP) where causal co-activations undergo Long-Term Potentiation (LTP) and unreferenced/distracting paths undergo Long-Term Depression (LTD).
- **Bio-Genetic Code Slicing (Exon/Intron Splicing)**: Slices code into expressed exons (active symbols, signatures, types, imports) and suppressed introns (untargeted implementation bodies folded into reversible folds), achieving 85–98% token reduction on large codebases.
- **Mycelial Hyphal Network Predictive Cache**: Models symbol access as fungal nutrient gradients and predictive hyphal growth, pre-warming downstream dependencies in zero latency.
- **Cellular Membrane Osmotic Quality Gate**: Regulates context permeability (Hyper-Impermeable, Semi-Permeable, Fully Permeable) based on internal osmotic pressure (task risk, AST complexity) and external budget.

```
Agent Request / Tool Action
           │
           ▼
    Task Signature (Stimulus)
           │
           ▼
 Cellular Membrane Osmotic Gate (Permeability Regulation)
           │
           ▼
 Physarum Polycephalum Solver (Minimal Steiner Flux Optimization)
           │
           ▼
 Synaptic STDP Plasticity Engine (Causal LTP / LTD Pathways)
           │
           ▼
 Bio-Genetic Code Skeletonizer (Exon Express / Intron Fold)
           │
           ▼
 Mycelial Hyphal Predictive Cache (Zero-Latency Pre-Warming)
           │
           ▼
 Reversible Ultra-Lean Context View ◄─── (Instant Sensory Expansion)
           │
           ▼
 Universal Provider Gateway (OpenAI / Anthropic / Gemini / Local)
           │
           ▼
 Target LLM / High-Speed Streamed Completion
```

---

## 2. Workspace Organization & Crate Topology

| Crate | Purpose | Core Responsibilities & Biomimetic Modules |
|---|---|---|
| `neuromesh-core` | Core domain types & utilities | `TaskSignature`, `ContextNode`, `ContextEdge`, `ContextView`, `Config`, `Error`, UUIDs |
| `neuromesh-index` | File indexing & workspace tracking | Fast parallel directory walker, BLAKE3 content hashing, git diff detection, token counting |
| `neuromesh-parser` | Tree-sitter Code Intelligence | Multi-language AST parsing (Vue 3 SFC, TS, JS, SCSS, Rust, Python, Go, etc.), symbol & import extraction |
| `neuromesh-graph` | Neural Project Graph & Bio Solvers | **Physarum Solver**, **STDP Synaptic Plasticity Engine**, spreading activation, edge homeostasis |
| `neuromesh-task` | Task Engine | Task Signature extraction, hierarchical subtask decomposition, dependency DAGs |
| `neuromesh-context` | Context Runtime & Slicing | **Bio-Genetic Code Skeletonizer**, $O(N)$ linear Myers diffing, reversible context registry |
| `neuromesh-cache` | Semantic & Predictive Cache | **Mycelial Hyphal Network Prefetcher**, exact cache, semantic cache, tool result cache |
| `neuromesh-router` | Osmotic Quality Gate & Budgeting | **Cellular Membrane Osmotic Quality Gate**, adaptive budget allocation, safety overrides |
| `neuromesh-provider` | Provider Abstraction | Universal `Provider` trait, client implementations for OpenAI, Anthropic, Google, OpenRouter, Local, Mock |
| `neuromesh-api` | HTTP API & Proxy | Axum HTTP server hosting OpenAI-compatible endpoints (`/v1/chat/completions`, `/v1/responses`, `/v1/models`) |
| `neuromesh-mcp` | Model Context Protocol Server | MCP JSON-RPC 2.0 implementation over stdio/SSE exposing context and memory tools |
| `neuromesh-local-ai` | Local Intelligence Engine | Native GGUF inference (llama.cpp bindings / native fallback) for classification, intent detection, ranking |
| `neuromesh-observability` | Telemetry & Audit | Zero-leak metrics collector, latency monitor, token savings analytics, SQLite audit logger |
| `neuromesh-cli` | CLI Entrypoint & Benchmarks | Command-line interface (`neuromesh start/benchmark/status/graph/evaluate/optimize/doctor/etc.`) |

---

## 3. Runtime Guarantees

1. **Structural honesty**: Import and call edges are created only when the target resolves uniquely (same file, imported files, or a single global definition). Ambiguous names stay unlinked.
2. **Bounded activation**: `get_context` walks a neighborhood of seed identifiers. Physarum never runs on the full edge set of a large repo.
3. **Reversible folds**: Untargeted function bodies become `[neuromesh:fold]` markers and can be expanded by id.
4. **Safe workspace**: Indexing prefers a git/cargo root and refuses `$HOME` / drive roots.
5. **Native Rust MCP binary**: stdio JSON-RPC, no hosted service.
