# NeuroMesh V1 — Detailed Design & Mathematical Formulations

## 1. Mathematical Models & Algorithms

### 1.1 Activation Scoring Formulation
For every node $n \in \mathcal{V}$ in the Neural Project Graph, its context activation score $S(n)$ with respect to a Task Signature $\mathcal{T}$ is defined as:

$$S(n, \mathcal{T}) = R(n, \mathcal{T}) \cdot C(\mathcal{T}) \cdot I(n, \mathcal{T}) \cdot \rho(n) \cdot \Phi(n) \cdot H(n, \mathcal{T})$$

Where:
- $R(n, \mathcal{T}) \in [0, 1]$: **Relevance Factor**, computed via exact symbol overlap, semantic embedding cosine similarity, and AST structural alignment with task entities and concepts.
- $C(\mathcal{T}) \in [0, 1]$: **Confidence Metric**, derived from task clarity, entity specificity, and parser coverage.
- $I(n, \mathcal{T}) \in [0, 1]$: **Task Impact Factor**, determining how critical node $n$ is to the execution path (e.g. entrypoints, interfaces, or modified targets vs. distant dependencies).
- $\rho(n) \in (0, 1]$: **Recency Decay**, modeling temporal relevance:
  $$\rho(n) = \exp\left(-\lambda \cdot \frac{t_{\text{now}} - t_{\text{last\_access}}}{\Delta t_{\text{session}}}\right)$$
- $\Phi(n) \in [0, 1]$: **Relationship Strength**, maximum or aggregated edge weight connecting $n$ to the active seed set.
- $H(n, \mathcal{T}) \in [0, 1]$: **Historical Success Ratio**, empirical win rate of node $n$ in previously resolved tasks sharing similar task signatures.

---

### 1.2 Spreading Activation Algorithm
Activation originates from the seed nodes $\mathcal{S}_0 \subseteq \mathcal{V}$ directly identified in the Task Signature $\mathcal{T}$. In iteration $k+1$, activation spreads across outgoing and incoming weighted edges:

$$A^{(k+1)}(u) = (1 - \gamma) A^{(0)}(u) + \gamma \sum_{v \in \mathcal{N}(u)} A^{(k)}(v) \cdot W(v, u) \cdot \kappa(\tau(v, u))$$

Where:
- $\gamma \in (0, 1)$: Decay factor per propagation hop (default $\gamma = 0.65$).
- $W(v, u) \in [0, 1]$: Dynamic pheromone weight of edge $(v, u)$.
- $\kappa(\tau) \in [0, 1]$: Edge-type attenuation coefficient based on relationship type $\tau$ (e.g. $\kappa(\text{Imports}) = 1.0$, $\kappa(\text{Calls}) = 0.85$, $\kappa(\text{RelatedTo}) = 0.40$).
- Propagation terminates when $\Delta A < \epsilon$ (typically $\epsilon = 10^{-3}$) or maximum hops $K = 4$ is reached.

---

### 1.3 Pheromone Learning & Reinforcement
Edges dynamically learn from task outcomes through biological reinforcement:

$$W_{e}^{(t+1)} = \text{clamp}\left( (1 - \delta) W_e^{(t)} + \Delta W_{e}^{\text{feedback}}, \, W_{\text{min}}, \, W_{\text{max}} \right)$$

- $\delta \in [0.01, 0.05]$: Pheromone evaporation rate per task epoch.
- Upon Task Success ($y = +1$):
  $$\Delta W_{e}^{\text{feedback}} = \eta \cdot \frac{1}{\text{depth}(e) + 1} \cdot \mu_{\text{success}}$$
- Upon Task Failure / Expansion Penalty ($y = -1$):
  $$\Delta W_{e}^{\text{feedback}} = -\eta \cdot \mu_{\text{failure}}$$
- Boundary bounds: $W_{\text{min}} = 0.05$, $W_{\text{max}} = 1.0$.

---

## 2. Reversible Context Invariants

1. **Zero Information Loss**:
   No context node is permanently destroyed when deactivated. Inactive nodes are stored in the `ReversibleContextRegistry` as compact descriptors:
   ```rust
   pub struct InactiveContextDescriptor {
       pub id: NodeId,
       pub source_file: PathBuf,
       pub line_range: Range<usize>,
       pub content_hash: Blake3Hash,
       pub version: u64,
       pub token_cost: usize,
       pub relevance_score: f32,
       pub parent_node: Option<NodeId>,
   }
   ```
2. **Deterministic Expansion**:
   If an agent or downstream model emits an expansion probe (e.g. `EXPAND_CONTEXT(ProductGrid.vue)` or missing symbol diagnostics), the expansion engine reactivates the exact slice in $O(1)$ lookup time and splices it into the active context stream.
3. **Traceability**:
   Every expansion step emits an audit record with `[expansion_id, reason, activated_nodes, added_tokens, previous_state_hash]`.

---

## 3. Tree-sitter Code Intelligence Architecture

The parser engine (`neuromesh-parser`) constructs a multi-language AST symbol forest:

### Vue 3 Single File Component (SFC) Extraction
- **Template Block (`<template>`)**: Extracted component tags (`<ProductCard>`, `<ProductGrid>`), v-bind props, slot usages, event listeners.
- **Script Block (`<script setup lang="ts">`)**: Extracted TypeScript interfaces, imports, Pinia store hooks (`useCartStore()`), Vue composables (`useRoute()`, `ref()`, `computed()`), reactive state variables.
- **Style Block (`<style lang="scss" scoped>`)**: Extracted `@import`, `@use`, SCSS mixins, CSS variables (`--color-primary`, `$spacing-md`), breakpoint queries.

### Cross-Layer Linker
The parser resolves inter-layer dependencies:
$$\text{Vue SFC} \xrightarrow{\text{imports}} \text{TS Store} \xrightarrow{\text{references}} \text{API Client}$$
$$\text{Vue SFC} \xrightarrow{\text{styles}} \text{SCSS Tokens} \xrightarrow{\text{uses}} \text{Global Mixins}$$

---

## 4. Universal Provider Proxy & Stream Multiplexer

The HTTP engine (`neuromesh-api`) operates an asynchronous token-by-token streaming proxy:
1. Inbound Server-Sent Events (SSE) from the upstream provider are parsed into raw completion chunks.
2. In-flight token usage is tracked via byte-level BPE token counters without buffering or adding pipeline delay.
3. On stream completion, the actual prompt tokens, completion tokens, duration, and optimization ratios are written to the telemetry ring buffer.
