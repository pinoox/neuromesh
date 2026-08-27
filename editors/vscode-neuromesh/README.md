# NeuroMesh for VS Code and Cursor

Sidebar mesh stats, a packet inspector, fold CodeLens, and the galaxy UI — talking to a running `neuromesh monitor`.

The agent loop in the editor matches 0.7.10: **get_context → expand_fold** (or **expand_gap** for `packet_gaps`). Grep (`search_symbols`) when coverage is `partial` or `no_seed_resolved`. After a good edit, **record_feedback**; use **get_node_weights** to verify learning deltas.

## Install

1. Copy or symlink this folder into your extensions directory:
   - Cursor: `~/.cursor/extensions/vscode-neuromesh`
   - VS Code: `~/.vscode/extensions/vscode-neuromesh`

   Windows (from this repo):

   ```powershell
   cmd /c mklink /J "%USERPROFILE%\.cursor\extensions\vscode-neuromesh" "%CD%\editors\vscode-neuromesh"
   ```

2. Install the `neuromesh` binary ([root README](../../README.md#install)).
3. Reload the window. Start the monitor in **this workspace**:

```bash
neuromesh monitor
```

4. Point MCP at `neuromesh mcp` (Command Palette → **NeuroMesh: Copy MCP Config**).

## What you get

| Surface | Role |
| :--- | :--- |
| **Activity bar → NeuroMesh** | Live mesh (files / nodes / edges / mode / Physarum), last packet files, session folds |
| **Status bar** | Offline warning, or `NM 93.5% · 8 folds` from the last packet |
| **Packet Inspector** | Diagnostic evidence packet from HTTP simulate: vs workspace, vs selected, coverage, budget |
| **Galaxy Monitor** | The 3D/2D graph UI (default `http://127.0.0.1:8765`; `neuromesh port`) |
| **Fold markers** | `[neuromesh:fold:…]` lines get a ruler mark, hover, and CodeLens → expand from RAM |

## Commands

| Command | Shortcut |
| :--- | :--- |
| Get Context for Selection | `Ctrl+Alt+N` / `⌘⌥N` |
| Open Packet Inspector | `Ctrl+Alt+M` / `⌘⌥M` |
| Expand Fold at Cursor | — |
| Skeletonize Current File | — |
| Search Symbols | — |
| Record Feedback (STDP) | — |
| Set Membrane Mode | — |
| Re-index Workspace | — |

`max_savings` / `balanced` / `max_quality` are the same membrane as the MCP tools (`0` / `5k` / `16k` fill on top of seeds).

## Settings

`neuromesh.host`, `neuromesh.port` (default `8765` — match `neuromesh port` / the running monitor), `neuromesh.pollIntervalMs`, `neuromesh.defaultMode`, `neuromesh.showFoldDecorations`.

Product docs: [docs/](../../docs/README.md) · [MCP tools](../../docs/mcp.md)
