# Changelog

All notable user-facing changes live here. The README stays a product guide, not a version diary.

## 0.5.0 — 2026-08-23

The agent loop is real: **get_context → expand_fold**, Grep only when coverage is `partial`.

- **Folds.** Skeletonization registers each `[neuromesh:fold]` body. `neuromesh_expand_fold` restores it by `fold_id` from the registry (no disk re-read).
- **Smarter fill.** Soft crate caps, giant files skeletonized instead of dropped, unresolved-call closers scored. Each callee file is scored once so a large `match` does not drown the packet.
- **Packets.** Every file includes `path`, `why`, `line_range`, `folded_symbols`, and `seed_call_coverage`.
- **Parse.** tree-sitter for Rust and TypeScript behind the same `AstAnalysisResult`; regex remains the fallback. Impl- and field-aware resolve (`self.activator.activate` → `ContextActivator::activate`). Ambiguous calls stay `Likely` instead of vanishing.
- **Skeletonizer** prefers parser/graph function spans over brace counting.
- **Gold.** Path-qualified files, five fixture repos under `tests/fixtures/`, recall ≥ 0.8 **and** precision ≥ 0.4. `neuromesh eval` runs the workspace and the fixtures.

## 0.4.0

Seed-then-fill packets. Seeds always ship; connectors fill under a real extra-token cap (`max_savings` 0 · `balanced` 8k · `max_quality` 16k). Coverage claims (`no_recorded_gap` / `partial`). QualityGate honors the requested mode unless the task is critical.

## 0.3.0

Two-pass structural graph: extract symbols, imports, and scoped calls, then unique-resolve edges after every file exists. Ranked search. Safe workspace discovery (git/Cargo root; refuse home and drive roots).
