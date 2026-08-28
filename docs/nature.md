# Living systems

NeuroMesh is a context engine first. The biology is the **design language**: names, modules, and research hooks that make the repo fun to grow — not a claim that a slime mold is compiling your Rust.

If you want to contribute, this page is the map from metaphor → crate.

## The five tissues

```
Stimulus (the prompt)
        │
        ▼
  Osmotic membrane     QualityGate — how much context may pass
        │
        ▼
  Neural mesh          Graph of files, symbols, Imports, Calls
        │
        ▼
  Physarum flux        Optional Steiner tissue between seeds
        │
        ▼
  Genetic splice       Exons stay open; introns fold
        │
        ▼
  Synaptic STDP        Feedback on the path the agent really used
        │
        ▼
  Mycelial cache       Prefetch the next hypha
```

### Physarum — cheapest connecting tissue

Slime molds find short paths that still feed every food source. `PhysarumSolver` in `neuromesh-graph` is a Hagen–Poiseuille style flux on the project graph: reinforce tubes that carry flow, atrophy the rest.

On `neuromesh_get_context`, after seeds resolve, **neighborhood Physarum** grows tubes between two or more seeds on a capped subgraph (hot-path config, target &lt; 20ms). If the neighborhood is too large or there is only one seed, fill falls back to seed-then-fill. `get_stats` reports `physarum_solver: active` only when that tube path actually ran.

### Synapses — STDP / Hebbian edges

Edges carry pheromone weight. `neuromesh_record_feedback` is a **required** step after a successful edit: spike the nodes the agent touched so the next `get_context` can prefer co-edited files and **emit** them into the packet (not only raise candidate scores). Without feedback there is no synaptic learning. Mycelium records the same transitions and pre-warms predicted neighbors.

### Exons / introns — genetic slicing

`CodeSkeletonizer` treats seed functions as **exons** (expressed) and sibling helpers as **introns** (folded). Markers are reversible; `neuromesh_expand_fold` is the spliceosome. This *is* on the hot path of `get_context`.

### Osmosis — QualityGate

A cell does not dump its cytosol because someone knocked. `max_savings` / `balanced` / `max_quality` are permeability. Tasks that smell like auth, payment, or secrets force a more permeable membrane.

### Mycelium — prefetch

`neuromesh-cache` is on the `get_context` loop: after a packet ships, hyphal tips of those files are pre-warmed so `expand_fold` / the next activate can hit RAM.

## Where to hack

| You like… | Open this |
| :--- | :--- |
| Graph algorithms, Steiner, flux | `crates/neuromesh-graph/src/physarum.rs` |
| Plasticity, edge weights, emission | `crates/neuromesh-graph/src/synapse.rs`, `crates/neuromesh-context/src/emission.rs`, `neuromesh_record_feedback` |
| Parsers, tree-sitter, new languages | `crates/neuromesh-parser` |
| Packet composition, folds | `crates/neuromesh-context` |
| Budgets, critical-task policy | `crates/neuromesh-router` |
| Prefetch / cache | `crates/neuromesh-cache` |

Ground rules: unique resolve (no million fake edges), real fill caps, gold tests in `tests/` and `tests/fixtures/`. Nature is the metaphor; honesty is the runtime. See [contributing.md](contributing.md).
