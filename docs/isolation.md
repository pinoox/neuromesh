# Project isolation

One runtime, many projects, no bleed. This page describes how a project is
identified, what guarantees the graph makes, and what it refuses to index.

## Project identity

A `ProjectId` is a pure function of the canonical path of the **project root** —
the nearest enclosing git repository, or the workspace itself when it is not in
git:

```
ProjectId = sha256(normalize_workspace(project_root))[..16 hex]
```

It is derived the same way everywhere: the MCP server, every CLI command, and
the monitor's HTTP server. Nothing is written into your repository to compute
it, and no `git` binary is required (a `.git` file from a worktree or submodule
counts as well as a directory).

Consequences worth knowing:

| Situation | Result |
| :--- | :--- |
| You open the repo root | The repo's id |
| You open a subdirectory of the repo | **The same** id — a subdirectory is the same project |
| Two checkouts of the same repo in different paths | **Different** ids, different graphs |
| Two unrelated projects whose folders are both named `app` | **Different** ids |
| A monorepo with several packages | One id, one graph (see [Monorepos](#monorepos)) |

Storage follows identity: each project gets its own slot under
`~/.neuromesh/projects/<name>-<hash>/` holding `graph.bin`, `embeddings.bin`,
`neuromesh.json`, and `config.json`. See [configuration.md](configuration.md)
for the `managed` / `local` store modes.

## The single-project invariant

> Every node in a live graph belongs to the graph's current project.

Every `ContextNode` records the project it was ingested for, so the check is
exact and costs no disk access. It runs after each re-index:

- `assert_single_project()` — reports offenders as `(project id, path)` pairs
- `evict_foreign_nodes()` — drops them file by file, which also clears their
  `file_hashes` entry so the next scan re-ingests the path for the *current*
  project instead of skipping it
- `enforce_single_project()` — logs at error level with a sample, then evicts

Why this is needed rather than assumed: re-indexing prunes paths that are absent
from the new scan, which removes most of a previous project's nodes on its own.
But `ingest_file_keep` returns early when a path's stored hash equals the
incoming hash, and two projects very often share a relative path *and* its exact
bytes — `LICENSE`, `.gitignore`, an empty `__init__.py`, boilerplate config. For
those, the previous project's nodes would survive. The invariant closes that gap
instead of relying on the coincidence that paths differ.

### Loading a stored graph

A `graph.bin` belongs to exactly one project, but the id it recorded may be
stale — written before ids were derived from the path, say.
`reconcile_loaded_project_id` runs inside `load_persisted` and distinguishes:

| Snapshot | Outcome |
| :--- | :--- |
| All nodes carry the current id | `AlreadyCurrent` — nothing to do |
| All nodes carry **one** older id | `Restamped` — re-stamped, index preserved |
| Nodes carry **several** ids | `Mixed` — a real leak; deliberately *not* re-stamped, left for eviction |

Re-stamping a mixed snapshot would launder a contaminated graph into looking
clean, so it is refused.

## Switching projects

When an MCP client hands over a workspace in `initialize`, the server skips work
only when it is a genuine no-op — **same project id *and* same root**. Comparing
paths alone was not enough: workspace discovery can collapse two projects onto
one directory, and the old directory-name id made unrelated checkouts compare
equal.

A real switch clears the graph first, then points the memory store at the new
project's slot, loads any stored graph, and re-indexes. The memory store follows
the project because episodes and project facts are keyed by project id; leaving
it pinned wrote the new project's memory into the previous project's file.

## What will not be indexed

`is_safe_workspace` refuses home, drive, and system directories. On top of that,
a workspace root that was **guessed** rather than given must also look like a
project — it needs one of `.git`, `Cargo.toml`, `package.json`,
`pyproject.toml`, `go.mod`, `composer.json`, and friends.

The distinction matters:

- **You named the path** (`neuromesh mcp <path>`, `NEUROMESH_WORKSPACE`, an MCP
  client's `rootUri`) → safety checks only. Pointing at a nested folder with no
  manifest is supported and keeps working.
- **We inferred the path** (current directory, `neuromesh monitor`) → safety
  checks *and* a project marker. Without this, a server started with no
  workspace would index whatever happened to be in the current directory.

A refusal prints what was wrong and how to fix it, rather than silently
indexing.

Ignore rules (`node_modules`, `target`, `dist`, `vendor`, …) apply to paths
**relative to the workspace root**. A project that lives under a folder named
`build` or `dist`, or on Windows under `AppData`, indexes normally — only its
own subdirectories are matched.

## Monorepos

Today a monorepo is one project: one id, one graph. That is predictable and
right for most repositories. Splitting a monorepo into independently-scoped
sub-projects is a separate, explicit feature and is not implemented yet.

The monitor's `__all__` endpoint deliberately indexes sibling projects into a
single `collective_mesh` graph. That is an intentional cross-project view, not a
leak: nodes take the graph's current id at ingest, so the invariant still holds.

## Verifying it

`cargo test -p neuromesh-context --test cross_project_isolation` is a CI-gated
end-to-end check. It stages two fixtures — a Rust web service and a PyTorch
detection project — that share `scripts/util.py` byte-for-byte, swaps between
them, and asserts that the shared file changes owner, that no Rust file survives
into the Python project, and that no Rust file reaches its packet.

Unit coverage lives in `crates/neuromesh-graph/src/isolation_tests.rs` and
`crates/neuromesh-core/src/project_id.rs`.
