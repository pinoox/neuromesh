# Contributing

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

Helpful targeted runs:

```bash
cargo test -p neuromesh-parser --lib
cargo test -p neuromesh-graph --lib
cargo test -p neuromesh-context --lib
```

## The biomimetic layer

If you came for slime molds and synapses: read [nature.md](nature.md) first. Good first patches:

- Physarum flux / Steiner tissue — `crates/neuromesh-graph/src/physarum.rs`
- STDP on real agent paths — `synapse.rs` + `neuromesh_record_feedback`
- A new language extractor with unique-resolve tests
- Gold fixtures under `tests/fixtures/`

Nature is the metaphor. Unique edges and fill caps are the contract.

## Adding a language

1. Extract **symbols**, **imports**, and **calls scoped to the current function**.
2. Plug into `CodeIntelligenceEngine` with a regex or tree-sitter path. Keep a fallback.
3. Add a unique-resolve test (same name in two files must not explode edges).
4. Prefer a fixture under `tests/fixtures/` and a gold prompt over a marketing number.

Do not add a fuzzy “link every namesake” pass. That is how a home-directory index grew a million edges.

Rust and TypeScript already go through tree-sitter **queries** (`src/queries/*.scm`) via `LanguageSpec`. Python, Go, Java, Kotlin, PHP, C#, Dart, Swift, and Ruby use the same driver (regex/`GenericParser` fallback). Function `line_range` must cover the **body** (not only the name) so skeletonizer folds are honest. Framework overlays tag Android/Spring/Django/FastAPI/Next/Nuxt/Laravel/Pinoox/Symfony/WordPress/React/Vue/Svelte/Astro/Twig/Electron/Tauri/Vite/Prime/Rails/Flutter routes and components after extract. Vue/Svelte/Astro share a scoped regex extractor (Astro frontmatter is parsed as TypeScript). `.twig` uses the HTML fallback. C / C++ stay on `GenericParser`. A new language is a registry row plus queries or a fallback — not a new `match` arm in the engine. Do not add tree-sitter crates on ABI 15.

## Docs

User-facing markdown lives in [`docs/`](README.md). The root [README](../README.md) is the product page. Version history is [CHANGELOG.md](CHANGELOG.md) — do not dump release notes into the README.
