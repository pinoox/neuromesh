#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

NEW_VERSION="${1:-}"
if [ -z "${NEW_VERSION}" ]; then
  echo "Usage: $0 <new_version> [old_version]"
  exit 1
fi

NEW_VERSION="${NEW_VERSION#v}"
OLD_VERSION="${2:-}"

if [ -z "${OLD_VERSION}" ]; then
  OLD_VERSION="$(grep -A 5 '\[workspace.package\]' Cargo.toml | grep '^version' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
fi
OLD_VERSION="${OLD_VERSION#v}"

if [ "${OLD_VERSION}" = "${NEW_VERSION}" ]; then
  echo "Current version is already ${NEW_VERSION}. Nothing to bump."
  exit 0
fi

echo "Bumping NeuroMesh version: ${OLD_VERSION} -> ${NEW_VERSION}"

replace_in_file() {
  local file="$1"
  local pattern="$2"
  local replacement="$3"
  if [ -f "${file}" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
      sed -i '' -E "s|${pattern}|${replacement}|g" "${file}"
    else
      sed -i -E "s|${pattern}|${replacement}|g" "${file}"
    fi
    echo "  Updated: ${file}"
  fi
}

# 1. Cargo.toml
replace_in_file "Cargo.toml" "version = \"${OLD_VERSION}\"" "version = \"${NEW_VERSION}\""

# 2. package.json (VS Code extension)
replace_in_file "editors/vscode-neuromesh/package.json" "\"version\": \"${OLD_VERSION}\"" "\"version\": \"${NEW_VERSION}\""

# 3. install.sh & install.ps1
replace_in_file "install.sh" "v${OLD_VERSION}" "v${NEW_VERSION}"
replace_in_file "install.ps1" "v${OLD_VERSION}" "v${NEW_VERSION}"

# 4. MCP protocol & descriptors
replace_in_file "crates/neuromesh-mcp/src/protocol.rs" "NeuroMesh MCP v${OLD_VERSION}" "NeuroMesh MCP v${NEW_VERSION}"
replace_in_file "crates/neuromesh-mcp/src/descriptors.rs" "Default \(v${OLD_VERSION}\):" "Default (v${NEW_VERSION}):"

# 5. README & Docs
DOC_FILES=(
  "README.md"
  "docs/index.html"
  "docs/assets/i18n.js"
  "docs/README.md"
  "docs/agent-guide.md"
  "docs/agent-rule.mdc"
  "docs/api.md"
  "docs/architecture.md"
  "docs/cli.md"
  "docs/configuration.md"
  "docs/engines.md"
  "docs/graph-proxy.md"
  "docs/mcp.md"
  "editors/vscode-neuromesh/README.md"
)

for doc in "${DOC_FILES[@]}"; do
  replace_in_file "${doc}" "v${OLD_VERSION}" "v${NEW_VERSION}"
  replace_in_file "${doc}" "نسخه ${OLD_VERSION}" "نسخه ${NEW_VERSION}"
done

echo
echo "Version bump to ${NEW_VERSION} completed successfully."
echo "Next recommended steps:"
echo "  1. Update docs/CHANGELOG.md with release notes"
echo "  2. cargo check --workspace (updates Cargo.lock)"
echo "  3. cargo test --workspace"
echo "  4. git commit -am \"chore(release): bump version to ${NEW_VERSION}\""
echo "  5. git tag -a v${NEW_VERSION} -m \"Release v${NEW_VERSION}\""
echo "  6. git push origin main --tags"
