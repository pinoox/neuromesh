# Changelog

All notable user-facing changes live here. The README stays a product guide, not a version diary.

## 0.5.3 — 2026-08-24

Build on 1.80+, honest MCP args, Kotlin in the indexer, and a less hollow PHP/Go/Java graph.

- **MSRV.** Even/odd uses `seed & 1` instead of `u64::is_multiple_of` (Rust 1.87). Workspace `rust-version` is 1.80. `rust-toolchain.toml` pins stable + rustfmt/clippy. Ubuntu `apt` rustc 1.75 still cannot parse lockfile v4 — use rustup.
- **`task` alias.** `neuromesh_get_context` accepts `task_description`, `prompt`, or `task`. An empty prompt is a JSON-RPC error, not a silent empty packet.
- **Generic languages.** PHP/Go/Java/Kotlin/C#/C/C++ extract functions and calls. `.kt` / `.kts` are indexed (`fun`, `object`, `data class`, imports). `throw new X`, `catch (X`, Kotlin `catch (e: X)`, and PHP `X $param` become inbound `Calls` edges.
- **Doctor skipped files.** `neuromesh doctor` (and `index`) report unsupported extensions so a Kotlin-only repo is no longer a silent empty scan.
- **Query extractors.** Rust and TypeScript parsing is driven by tree-sitter queries (`src/queries/*.scm`) behind a language registry. Regex remains the fallback. Gold on this repo must stay green.
- **Wave 3 framework overlays.** Android Activity/Compose/BroadcastReceiver, Spring mappings, Django `urls.py`, Next `app/` routes, Laravel `Route::`, Pinoox `action()`, Symfony `#[Route]`, WordPress REST, React/Vue/Svelte/Twig/Electron/Tauri/Vite/Prime UI become `Component`/`Api`/`Config` from layout and annotations — no compiler. Stack facts come from manifests (`pinoox/pincore`, `react`, `vite`, Shopfa mentions). Gold: `mini-kotlin` “How is a received SMS stored?”, `mini-next`, `mini-pinoox`.
- **Index speed.** Workspace ingest parses files in parallel (rayon) and reuses a tree-sitter parser per thread. Hash skip is unchanged. `neuromesh index` uses the same ingest path as MCP.

## 0.5.2 — 2026-08-23

Monitor port is a first-class CLI setting, not a hardcoded 8765.

- **`neuromesh port`.** Print the effective port, or persist it with `neuromesh port 9000` (`<cwd>/.neuromesh/config.json`).
- **One-shot override.** `neuromesh monitor --port 9000` (`-p`, `--port=`) and the same flag on `start`. Env `NEUROMESH_PORT` wins over files.
- **Clients follow.** `doctor`, `connect`, and telemetry POST use the loaded host/port. VS Code / Cursor still uses Settings → `neuromesh.port` — keep it in sync.

## 0.5.1 — 2026-08-23

Accuracy first, then faster index, then a thinner default packet.

- **Seed ranking.** `search_symbols` and `pick_dominant_candidate` prefer exact case, Class/Function/Component, and a path that repeats the symbol name (`Searcher` → `searcher/mod.rs`). Test/bench/example paths are penalized so a lowercase field twin does not steal the seed.
- **Hybrid resolve.** After `resolve_ranked`, activate also checks a high-score search hit. If case or path agrees and the ids differ, the search hit becomes the seed — so a confident but wrong ranked pick no longer ships a thin, wrong packet.
- **Index skip.** Walker ignores `benches`, `examples`, `testdata`, and extra caches (`.tox`, `.mypy_cache`, `.pytest_cache`). `tests/` stays indexed; fill still treats test/bench/example as noise.
- **Balanced fill.** Extra connector cap is 5,000 tokens (was 8,000). Gold on this repo still passes.
- **Seed callees stay exons.** Functions the seed actually calls keep their bodies; their files are required so the answer is not folded away.
- **MCP handshake.** Stdio initialize no longer hangs, so the monitor and Cursor can start the session.

## 0.5.0 — 2026-08-23

The agent loop is real: **get_context → expand_fold**, Grep only when coverage is `partial`.

- **Folds.** Skeletonization registers each `[neuromesh:fold]` body. `neuromesh_expand_fold` restores it by `fold_id` from the registry (no disk re-read).
- **Smarter fill.** Soft crate caps, giant files skeletonized instead of dropped, unresolved-call closers scored. Each callee file is scored once so a large `match` does not drown the packet. Seed callees stay exons so the function that answers the question is not folded.
- **Packets.** Every file includes `path`, `why`, `line_range`, `folded_symbols`, and `seed_call_coverage`.
- **Parse.** tree-sitter for Rust and TypeScript behind the same `AstAnalysisResult`; regex remains the fallback. Impl- and field-aware resolve (`self.activator.activate` → `ContextActivator::activate`). Ambiguous calls stay `Likely` instead of vanishing.
- **Skeletonizer** prefers parser/graph function spans over brace counting.
- **Gold.** Path-qualified files, five fixture repos under `tests/fixtures/`, recall ≥ 0.8 **and** precision ≥ 0.4. `neuromesh eval` prints workspace / selected / packet tokens, reduction, and Grep-still-needed.
- **Hot path.** Neighborhood Physarum tubes after two+ seeds (skip huge subgraphs; stats `active` only when used). Selector reads pheromone. Folds persist for the MCP session. Mycelium prefetches packet neighbors.

## 0.4.0

Seed-then-fill packets. Seeds always ship; connectors fill under a real extra-token cap (`max_savings` 0 · `balanced` 8k · `max_quality` 16k). Coverage claims (`no_recorded_gap` / `partial`). QualityGate honors the requested mode unless the task is critical.

## 0.3.0

Two-pass structural graph: extract symbols, imports, and scoped calls, then unique-resolve edges after every file exists. Ranked search. Safe workspace discovery (git/Cargo root; refuse home and drive roots).
