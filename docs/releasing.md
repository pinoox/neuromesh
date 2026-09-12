# Release & Version Bumping Guide

This guide describes how to bump versions and perform a release across the entire NeuroMesh repository.

---

## ⚡ Quick Automated Bump

NeuroMesh provides helper scripts in `scripts/` that update all configuration files, installers, MCP protocol strings, and documentation files in one shot:

### On Windows (PowerShell)

```powershell
pwsh ./scripts/bump-version.ps1 0.9.3
```

### On Linux / macOS (Bash)

```bash
./scripts/bump-version.sh 0.9.3
```

*(You can omit the old version — the script reads the current version directly from `[workspace.package]` in `Cargo.toml`.)*

---

## 📋 Files Updated by the Script

When a new version is set, the following files are synchronized:

| Category | File | Description |
| :--- | :--- | :--- |
| **Cargo Workspace** | `Cargo.toml` | `[workspace.package].version` (all crates inherit this) |
| **VS Code Extension** | `editors/vscode-neuromesh/package.json` | Extension `"version"` |
| **Installer Scripts** | `install.sh` & `install.ps1` | Header banners and version announcements |
| **MCP Engine** | `crates/neuromesh-mcp/src/protocol.rs` | Initialization handshake instructions string |
| **MCP Engine** | `crates/neuromesh-mcp/src/descriptors.rs` | Tool list description for `get_context_packet` |
| **Product Landing** | `README.md` | Pre-built binary, version check, benchmark notes |
| **Documentation Site** | `docs/index.html` & `docs/assets/i18n.js` | Release badge and i18n copy (EN & FA) |
| **Agent Guidance** | `docs/agent-guide.md` & `docs/agent-rule.mdc` | Default agent loop references |
| **API & Architecture** | `docs/architecture.md`, `docs/cli.md`, `docs/mcp.md`, `docs/api.md`, `docs/configuration.md`, `docs/engines.md`, `docs/graph-proxy.md`, `docs/README.md` | Architecture and engine references |
| **Extension Docs** | `editors/vscode-neuromesh/README.md` | Extension agent loop copy |

---

## 🚀 Step-by-Step Release Checklist

### 1. Run the Bump Script
```bash
pwsh ./scripts/bump-version.ps1 0.9.3
# or on Linux/macOS:
./scripts/bump-version.sh 0.9.3
```

### 2. Update Changelog
Open [`docs/CHANGELOG.md`](CHANGELOG.md) and add release notes under the new version header:
```markdown
## 0.9.3 — YYYY-MM-DD

### Fixes
- **Component name** — description of fix.
```

### 3. Verify & Test Workspace
Update `Cargo.lock` and ensure all tests pass:
```bash
cargo check --workspace
cargo test --workspace
```

### 4. Commit Changes
```bash
git add -A
git commit -m "chore(release): bump version to 0.9.3 and update CHANGELOG"
```

### 5. Tag and Push
Push both the commit and annotated tag to GitHub:
```bash
git tag -a v0.9.3 -m "Release v0.9.3"
git push origin main --tags
```

### 6. GitHub Actions Automated Release
Pushing the `v*` tag triggers `.github/workflows/release.yml`, which:
- Compiles multi-platform binaries (`linux-x86_64`, `darwin-arm64`, `windows-x86_64`)
- Packages archives with SHA256 checksums
- Publishes/updates the GitHub Release with attached assets
