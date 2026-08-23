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

Rust and TypeScript already go through tree-sitter. Other languages stay regex until eval on those two stays green.

## Docs

User-facing markdown lives in [`docs/`](README.md). The root [README](../README.md) is the product page. Version history is [CHANGELOG.md](CHANGELOG.md) — do not dump release notes into the README.
