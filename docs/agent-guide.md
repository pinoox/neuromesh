# Teach the agent to use NeuroMesh

`neuromesh connect` only registers the MCP **server**. Most IDEs still default to opening whole files unless you add **project instructions**. This guide covers every client NeuroMesh connects to.

| Step | What |
| :--- | :--- |
| 1 | Install / build `neuromesh`, then `neuromesh doctor` |
| 2 | From your **app** repo: `neuromesh connect` (or `--print` and paste) |
| 3 | Add the agent instructions below for your IDE |
| 4 | Restart the IDE (or reload MCP) and smoke-test |

Without step 3, tool lists may show NeuroMesh while the agent never calls it. That is expected: MCP ≠ rules.

---

## Universal instructions (any client)

Paste this body into whatever “project instructions / rules / AGENTS” file your tool supports. Cursor users can instead copy [agent-rule.mdc](agent-rule.mdc) (same content + `alwaysApply` frontmatter).

```markdown
# NeuroMesh context

This workspace has the NeuroMesh MCP server. Prefer it for **reading and exploring** code so the agent gets folded skeletons and targeted symbols instead of multi-thousand-line files.

## Default loop

1. Start with `neuromesh_get_context` using the task as written (`task_description` / `prompt` / `task`).
2. If `coverage.claim` is `partial` or `no_seed_resolved`, follow `packet_gaps` / `next` — `neuromesh_expand_gap` for near-miss paths, or `neuromesh_search_symbols` before broad Grep. `bounded` means seeds resolved with optional sidecar fill — proceed unless you need more files.
3. Expand only what you need: `neuromesh_expand_fold` with a `fold_id` from the packet (or `neuromesh_get_file_skeleton` / `neuromesh_expand_gap` for one path).
4. Use `neuromesh_trace` / `neuromesh_get_dependencies` / `neuromesh_analyze_impact` for callers, neighbors, and blast radius.
5. After a successful edit, call `neuromesh_record_feedback` with `task_success` and the nodes you touched. Use `neuromesh_get_node_weights` before/after to verify learning deltas when debugging routing.
6. If feedback should have changed the packet but `files[]` looks the same, call `neuromesh_explain_packet` and inspect `selection.candidates` for `emitted`, `drop_stage`, and `score_breakdown`.

Do not treat a utility fallback file as the answer when coverage says seeds missed or `packet_gaps` is non-empty.

## When not to force NeuroMesh

- Small, already-known paths (a few dozen lines) for a precise edit
- Applying patches / writing files (use normal editor tools; NeuroMesh does not replace them)
- Config, lockfiles, generated assets, or binary-ish content
- After NeuroMesh already reported incomplete coverage and you need a targeted Grep/Read on the gap

## Anti-patterns

- Opening large whole source files into context when a packet or skeleton is enough
- Expanding every fold “just in case”
- Skipping `neuromesh_record_feedback` after a good edit (no STDP learning for the next packet)
```

Keep one copy in the repo (for example `AGENTS.md`) and point each IDE at it, or duplicate into the client-specific paths below. Prefer **one** shared `AGENTS.md` when several tools share the same git root.

---

## Per-client setup

MCP config paths come from `neuromesh connect` (see [mcp.md](mcp.md#connect)). Instructions are separate.

### Cursor

1. MCP — either `neuromesh connect` → `.cursor/mcp.json` (or user `~/.cursor/mcp.json`), or paste manually into **Settings → MCP** when `neuromesh` is on PATH:

   ```json
   {
     "mcpServers": {
       "neuromesh": {
         "command": "neuromesh",
         "args": ["mcp"]
       }
     }
   }
   ```

   See [mcp.md#manual](mcp.md#manual) for when to prefer `connect`.

2. Copy the Cursor rule:

```bash
mkdir -p .cursor/rules
cp /path/to/neuromesh/docs/agent-rule.mdc .cursor/rules/neuromesh.mdc
```

3. Confirm Rules → the NeuroMesh rule is enabled / always apply.
4. Restart Cursor or **MCP: Restart Servers** if tools do not appear.

Optional: also put the universal body in root `AGENTS.md` for other tools in the same repo.

### VS Code + GitHub Copilot

1. `neuromesh connect` → `.vscode/mcp.json` (`servers`).
2. Copilot does not load `.cursor/rules`. Use one of:
   - **Repository custom instructions:** `.github/copilot-instructions.md` with the universal body
   - **VS Code Copilot instructions:** Settings → Copilot → custom instructions, or a workspace `.github/instructions/*.instructions.md` if your Copilot build supports it
3. Reload the window; open Chat / Agent and confirm NeuroMesh tools are listed.
4. If Agent mode ignores MCP, enable MCP for Copilot in settings and approve the `neuromesh` server.

### Claude Code / Claude Desktop

| Surface | MCP | Instructions |
| :--- | :--- | :--- |
| Claude Code (CLI) | `.mcp.json` or `claude mcp add neuromesh -- neuromesh mcp …` | Root **`CLAUDE.md`** and/or **`AGENTS.md`** (paste universal body) |
| Claude Desktop | `claude_desktop_config.json` (user) | Project **`CLAUDE.md`** / custom instructions in the project; Desktop often needs the instruction text in the chat project’s custom instructions UI |

Restart Claude after connect. Prefer `neuromesh_*` tool names if the client shows aliases.

### OpenAI Codex

1. `neuromesh connect` → `.codex/config.toml` or `~/.codex/config.toml`.
2. Put the universal body in root **`AGENTS.md`** (Codex reads project agent docs) and/or Codex’s project instructions if you use them in the app.
3. Restart Codex / reload the session so MCP tools refresh.

### OpenCode

1. Add NeuroMesh under `mcp` in project `opencode.json` / `.opencode/opencode.jsonc`, or globally in `~/.config/opencode/opencode.jsonc`. OpenCode expects a **local** server, for example:

   ```json
   {
     "mcp": {
       "neuromesh": {
         "type": "local",
         "command": ["neuromesh", "mcp"],
         "enabled": true
       }
     }
   }
   ```

   Prefer `neuromesh connect --print` and map the absolute binary + workspace args into that shape.
2. Put the universal body in root **`AGENTS.md`** (and OpenCode project instructions if your build exposes them).
3. Restart OpenCode / reload MCP.

### MiMo CLI

1. Merge into `.mimo-code.json` (project) or `~/.mimo-code/config.json` under **`mcpServers`**. Use `neuromesh connect --print` for the command and args, then add an entry with `"enabled": true`.
2. Mirror the universal body into **`AGENTS.md`** (MiMo CLI also reads project agent docs when configured).
3. Restart MiMo CLI or reload MCP from the TUI settings.

### Google Antigravity

1. `neuromesh connect` → `.agents/mcp_config.json` (project) or `~/.gemini/config/mcp_config.json` (user).
2. Add the universal body under the project’s agent instructions (Antigravity / Gemini agent docs for the workspace — often alongside `.agents/`). If the product offers a free-form “system instructions” field for the workspace, paste there.
3. Also keep root **`AGENTS.md`** so other agents in the same tree stay aligned.

### Gemini CLI

1. `neuromesh connect --global` → `~/.gemini/settings.json` when Gemini CLI is installed (same `mcpServers` merge as other Google tools).
2. Root **`AGENTS.md`** for agent behavior.
3. Restart the Gemini CLI session after connect.

### Kilo Code

1. `neuromesh connect` → `.kilo/kilo.jsonc` (MCP block).
2. Add instructions via Kilo’s project / custom instructions (UI or docs file Kilo indexes). Mirror into **`AGENTS.md`**.
3. Reload the Kilo window after connect.

### Trae / MiniMax Code

| Client | MCP | Instructions |
| :--- | :--- | :--- |
| Trae | `.trae/mcp.json` or Trae user MCP | Project rules / custom instructions in Trae settings; plus **`AGENTS.md`** |
| MiniMax Code | `.minimax/mcp.json` | MiniMax project instructions UI; plus **`AGENTS.md`** |

Restart the IDE after writing MCP + instructions.

### Windsurf

1. MCP is usually user-level (`~/.codeium/windsurf/mcp_config.json` via connect).
2. Windsurf Cascade: **Workspace Rules** / `.windsurfrules` (or the current Windsurf rules path in Settings). Paste the universal body.
3. Reload Cascade / restart Windsurf.

### Cline / Roo Code

1. Connect merges into each extension’s MCP settings file (user-level).
2. Cline: **Custom Instructions** in the Cline panel (or `.clinerules` if you use file-based rules).
3. Roo: project `.roo/` / custom instructions as documented by your Roo version — paste the same body.
4. Approve the NeuroMesh MCP server when prompted; restart the extension host if tools stay empty.

### Zed

1. `neuromesh connect --print` → paste into Zed `context_servers` (connect may not write Zed automatically).
2. Zed agent instructions: project **`AGENTS.md`** or Zed’s agent settings instructions field.
3. Restart Zed.

### One prompt that works everywhere

If you cannot edit rule files (locked CI image, guest machine), start the chat with:

```text
Use NeuroMesh MCP for context: neuromesh_get_context first, expand folds only as needed,
search_symbols if coverage is partial, record_feedback after a successful edit.
Do not dump large whole files when a packet is enough.
```

That is weaker than a persistent rule but unblocks a single session.

---

## Smoke test

In a repo that is already indexed (`neuromesh doctor` / prior MCP session):

1. Ask: *“How does X work in this codebase?”* (pick a real symbol).
2. Expect a **`neuromesh_get_context`** (or alias) tool call before large file reads.
3. Optionally: `neuromesh usage` — a row appears when the agent actually called a NeuroMesh tool (not when you only saved a file). See [cli.md](cli.md).

If the agent only Opens/Reads multi-thousand-line files and never calls MCP:

- Confirm MCP server is listed and enabled for that chat/agent mode
- Confirm the instructions file path for **that** client (table above)
- Restart the IDE after editing rules
- Try the one-shot prompt above

---

## What the handshake already does

On MCP `initialize`, NeuroMesh returns short **`instructions`** telling clients to start with `neuromesh_get_context`. Some hosts surface that string; many ignore it. Treat the project rule as the reliable channel; handshake text is a bonus, not a substitute.

Tool details and packet shape: [mcp.md](mcp.md). Living-systems loop: [nature.md](nature.md).
