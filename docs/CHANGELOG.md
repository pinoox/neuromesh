# Changelog

All notable user-facing changes live here. The README stays a product guide, not a version diary.

## Unreleased

- **Cluster seeds pick the router guard, not a UI helper.** A "router permission guard" clause still splits, but the noun `permission` now prefers `src/permission.js` / store permission modules over `directive/permission`. Clipboard and profile decoys are forbidden in gold.
- **Pinoox View→Twig is a walkable `Calls` edge.** `View::render('hello')` attaches to the rendering method, binds `theme/{theme}/hello.twig` by file path before the stem `hello` can steal another symbol, and `get_context` / `trace` on `MainController::index` ship the template without the prompt saying `twig`.
- **MCP usage is complete.** Handshake writes one `mcp_session` row; trace, deps, stats, explain, architecture, impact, and feedback append too. Mean reduction ignores 0-token rows so search/session do not drag the %. Monitor `GET /api/usage` reports the token-weighted overall %, and telemetry POST works without a Tokio handle.
- **Explicit MCP workspace stays put.** `neuromesh mcp <dir>` and initialize `rootUri` no longer walk up to a parent git/`Cargo.toml` root, so a fixture like `mini-auth` is not mixed with the rest of the repo.
- **`Type::method` seeds resolve, templates beat namesake helpers.** `MainController::index` binds the method (not a missed seed), and `hello.twig` outranks `Greeter.hello()` so the decoy stays out of the packet.

## 0.7.3 — 2026-08-26

Workspace confinement, first-query readiness, and compact MCP packets.

- **Workspace confinement.** `get_file_skeleton` and `read_source` refuse absolute paths, `../` traversal, and symlink escape. `neuromesh index` / start / monitor / eval / connect refuse home and filesystem roots in under 100ms. CLI `Processed Tokens` is the current run; `Workspace Tokens` is the graph total.
- **Index readiness.** Cold MCP `get_context` waits for the first index (or returns `indexing_in_progress`) instead of an empty `no_seed_resolved` packet. `neuromesh_get_stats` includes `index_state`, `generation`, and `ready`.
- **Coverage honesty.** Imperative verbs (`Modify`, `Refactor`, …) are not seeds. Equivalent file-path forms count as a hit. Unknown `mode` is a tool error. `.env` is skipped; `.env.example` and siblings are indexed.
- **Pinoox View→Twig.** `View::render('hello')` links `theme/{theme}/hello.twig` with a `Calls` edge so the template ships with the controller.
- **No fold bodies on the wire.** `get_file_skeleton` and `get_context` return `FoldDescriptor` (id, symbol, signature, lines, saved tokens). `original_body` stays in the session registry and returns only from `neuromesh_expand_fold`.
- **Minimal `get_context` by default.** Response is `packet_id`, `coverage` (`no_recorded_gap` | `partial` | `no_seed_resolved`), `tokens`, skeletonized `files`, `missing`/`next` only when coverage is incomplete. `mode` still picks files; `response_detail` (`minimal` | `standard` | `diagnostic`) picks metadata.
- **`neuromesh_explain_packet`.** Fetch seeds, selection, budget, physarum, and membrane for a `packet_id` from a 32-slot / 10-minute LRU (no source bodies). Unknown or expired ids are a tool error.
- **Compact MCP wire.** Tool `content[].text` is minified JSON of the same object as `structuredContent` (not pretty-printed). HTTP `/api/simulate` still requests `diagnostic` so the VS Code inspector is unchanged.

## 0.7.1 — 2026-08-26

Compound-task coverage is honest: each topical cluster seeds independently, and a named half that misses is `partial` — never a silent `no_recorded_gap`.

- **Cluster seeds.** `including` / `and how` / `as well as` split the prompt. A clause with no camelCase identifier (e.g. "router permission guard") still tries those nouns against the graph, so `src/permission.js` ships with the login module instead of being omitted while `coverage.claim` says complete.
- **False-complete coverage.** If that second cluster resolves nothing, `seeds_missed` is non-empty and `claim` is `partial` (Grep in `next_actions`). `unresolved` stays graph call/import gaps — it is not a list of missing task nouns.
- **Usage from IDE chat.** `neuromesh_expand_fold` now appends a telemetry row (it previously only recorded the inactive-node path). Handshake / chatting without a tool call still does not. Rows use unique request ids so two calls in the same millisecond are not dropped.

## 0.7.0 — 2026-08-26

Accurate cheaper packets: task-matched methods stay open, windows replace whole-file skeletons, and a hard packet cap cuts cost.

- **Task exons.** Skeletonization scores each function against the prompt (`nullSafe` + `TypeAdapter` → `NullSafeTypeAdapter.write`, `serialized` → `write`). The closest match stays open instead of folding the exact body an agent needs to diagnose.
- **Stable fold ids.** Markers are unique across files (`fold_write_4_<tag>`), so a later `JsonWriter.write` cannot overwrite `TypeAdapter.write` in the session registry.
- **`expand_fold` accepts `query`.** `next_actions` already pass `query`; the tool now reads `fold_id`, `node_id`, or `query`, including the full `[neuromesh:fold:…]` marker. Prefix lookup still finds the printed id.
- **Ranked `next_actions`.** Expand suggestions prefer high-scoring folds, not the first three in packet order.
- **Windowed packets.** Each file keeps at most K open bodies (seed `K=4`, optional `K=1`), ranked by task score. The skeleton emits imports, the enclosing type, those exons, and fold markers for sibling methods in the same type — not the rest of the file.
- **Packet cap.** After skeletonization, balanced packets are capped at 12k tokens (6k / 24k for max_savings / max_quality). Optional files drop first; then seed K shrinks 4→2. The top-scored method stays open. Fill caps stay 0 / 5k / 16k extra tokens.
- **Qualified symbol ids.** `NodeId` is `sym:{path}:{parent}.{name}` when the symbol has an enclosing type, so `TypeAdapter.write` and `NullSafeTypeAdapter.write` are distinct spans.

## 0.6.9 — 2026-08-25

Compact incremental mesh, managed store, usage telemetry, and multi-client MCP connect.

- **No file bodies in the mesh.** File nodes keep path, hash, mtime, and token cost. Source is read on demand for skeletonization, `expand_fold`, and `neuromesh_get_file_skeleton`, so the in-RAM graph no longer holds N copies of the workspace.
- **Binary snapshot.** The persisted graph is `graph.bin` (bincode, bodies stripped). An existing `graph.json` is still read once for migration. Cold load on this repo is **28 ms** against a **346 ms** full index; a one-file reindex is **27 ms** (`docs/quality.md`).
- **Compact graph store.** Nodes and edges live in slot vectors with `u32` adjacency; `NodeId`/`EdgeId` are `Arc<str>`, so lookups, neighborhood walks, and Physarum tubes stop cloning whole maps. Spreading activation walks the adjacency arrays under one read lock, and ingesting a file takes a single write lock instead of one per symbol.
- **Prefix symbol index.** `search_symbols` resolves prefixes through a sorted name index instead of scanning every symbol name.
- **Real incremental index.** The walker compares size + mtime first and reads only changed files; `neuromesh index` reports `Unchanged skip`. Live sync (CLI `start`/`monitor`/`mcp` and the MCP handshake) uses an OS watcher (`notify`, 200 ms debounce) instead of a 150 ms full-tree poll. Hashing is now really Blake3.
- **Inbound relink.** Re-ingesting a file re-queues the inbound `Calls`/`Imports` edges that pointed at its old symbols, so callers no longer lose edges until the next full reindex.
- **`neuromesh usage`.** Print MCP token telemetry from `~/.neuromesh/telemetry_history.json` (`--all`, `--limit N`). The file is the source of truth so stats show even when the monitor is down. Duplicate `request_id`s are ignored; the monitor reloads the file on each usage fetch.
- **Managed store.** Graph, memory, and per-project config default to `~/.neuromesh/projects/<name>-<hash>/`. A workspace `.neuromesh` folder is not trusted. Opt in with `neuromesh store local` or `trust_local` in `~/.neuromesh/config.json`. Existing in-repo files are copied into the managed slot once, then ignored.
- **MCP clients.** `neuromesh connect` writes stdio configs (absolute binary + `NEUROMESH_WORKSPACE`) for Cursor, VS Code, Codex, Antigravity, Kilo Code, Trae, MiniMax, Claude, Windsurf, Cline/Roo. Handshake accepts Windows `file://` URIs, `prompt`/`task`/`input` tool args, and returns tool errors as `isError` so picky agents keep going.

## 0.6.3 — 2026-08-25

Inbound throw edges for PHP rethrow and ternary `new Type`.

- **Inbound throws.** `throw $e` after `catch (Type $e)`, catch unions, and ternary `throw … new Type` become inbound `Calls` edges. Symfony matchers throw `ResourceNotFoundException`, not `RouteNotFoundException` — trace the type that is actually constructed or caught.

## 0.6.2 — 2026-08-24

Scale search on large repos, auto index cap, and `--max-files`.

- **Scale search.** Exact class/interface names outrank fuzzy `Http`/`Kernel` tokens. `neuromesh_get_context` uses `coverage.claim = no_seed_resolved` when every identifier misses, and does not ship a utility fallback file.
- **Index file cap.** Default is **auto**: grow to every production source (then tests), ceiling 50,000. `neuromesh index --max-files 20000` (or `auto`) persists like `neuromesh port`. Env `NEUROMESH_MAX_FILES`. `index` / `doctor` print the applied cap and warn on truncation.

## 0.6.1 — 2026-08-24

Language registry, tree-sitter queries, framework overlays, parallel index, and thinner packets.

- **MSRV.** Even/odd uses `seed & 1` instead of `u64::is_multiple_of` (Rust 1.87). Workspace `rust-version` is 1.80. `rust-toolchain.toml` pins stable + rustfmt/clippy. Ubuntu `apt` rustc 1.75 still cannot parse lockfile v4 — use rustup.
- **`task` alias.** `neuromesh_get_context` accepts `task_description`, `prompt`, or `task`. An empty prompt is a JSON-RPC error, not a silent empty packet.
- **Generic languages.** PHP/Go/Java/Kotlin/C#/C/C++ extract functions and calls. `.kt` / `.kts` are indexed (`fun`, `object`, `data class`, imports). `throw new X`, `catch (X`, Kotlin `catch (e: X)`, and PHP `X $param` become inbound `Calls` edges.
- **Doctor skipped files.** `neuromesh doctor` (and `index`) report unsupported extensions so a Kotlin-only repo is no longer a silent empty scan.
- **Query extractors.** Rust and TypeScript parsing is driven by tree-sitter queries (`src/queries/*.scm`) behind a language registry. Regex remains the fallback. Gold on this repo must stay green.
- **Wave 3 framework overlays.** Android Activity/Compose/BroadcastReceiver, Spring mappings, Django `urls.py`, Next `app/` routes, Laravel `Route::`, Pinoox `action()`, Symfony `#[Route]`, WordPress REST, React/Vue/Svelte/Twig/Electron/Tauri/Vite/Prime UI become `Component`/`Api`/`Config` from layout and annotations — no compiler. Stack facts come from manifests (`pinoox/pincore`, `react`, `vite`, Shopfa mentions). Gold: `mini-kotlin` “How is a received SMS stored?”, `mini-next`, `mini-pinoox`.
- **Index speed.** Workspace ingest parses files in parallel (rayon) and reuses a tree-sitter parser per thread. Hash skip is unchanged. `neuromesh index` uses the same ingest path as MCP.
- **Thinner packets.** Function spans follow the real tree-sitter body (Dart signature+body siblings, Kotlin `fun`, TS `const fn = () =>`). Folds replace the **body**, not the signature, so the file map stays; a parent that contains a seed exon is not folded. Fill caps are unchanged.
- **Wave 5 overlays.** Express `app.post`, Nest `@Controller`/`@Post`, Angular `@Component` + `path:`, Gin/Echo `.POST`, Axum `.route(..., post(`. Gold: `mini-express`, `mini-nest`, `mini-angular`, `mini-gin`, `mini-axum`. Prompt “how does store use …” keeps the lowercase method name so Astro/Express pages seed.
- **Wave 6 overlays.** ASP.NET `MapPost`/`[HttpPost]` + Razor `@page`/`@code` (`.cshtml`/`.razor` indexed as HTML), SwiftUI `struct: View`, Remix `app/routes/` + React Router `createBrowserRouter`, Ktor `post("/sms")`. Gold: `mini-aspnet`, `mini-swiftui`, `mini-remix`, `mini-ktor`.
- **Stylesheets and SVG.** `.less` is indexed. CSS/SCSS extract class/id selectors and `--custom-properties` (SCSS still gets `$var` / `@mixin`; LESS gets `@var` and `.mixin()`). `.svg` uses the HTML extractor so `<symbol id>` / `id=` and `<use href="#…">` become components. Gold: `mini-styles`.
- **Eval honesty.** `neuromesh eval` prints fixture dirs with an empty scan instead of skipping them. README numbers from release eval (2026-08-24): 219 files, 1,323 nodes, 2,891 edges, ~209 ms index.
- **Monitor galaxy.** 2D clicks open nodes (they used to pan instead). 3D picking chooses the front-most blob, pauses spin on hover, and ignores giant label hit-boxes. Nodes render as Physarum slime; **Play slime** grows, streams, and prunes tubes.
- **Monitor header.** Drop the extra Projects & Switch button — click the active-project chip to open the switcher. Compact one-line labels.

## 0.5.2 — 2026-08-23

Monitor port is a first-class CLI setting, not a hardcoded 8765.

- **`neuromesh port`.** Print the effective port, or persist it with `neuromesh port 9000` (managed project slot, or `<cwd>/.neuromesh` if trusted).
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
