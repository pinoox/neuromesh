#!/usr/bin/env bash
# Create nmx as a symlink to neuromesh. Run after release build.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RELEASE="${ROOT}/target/release"
SRC="${RELEASE}/neuromesh"
ALIAS="${RELEASE}/nmx"

if [[ ! -f "${SRC}" ]]; then
  echo "Missing ${SRC} — build first: cargo build --release -p neuromesh-cli --features embeddings" >&2
  exit 1
fi

ln -sf neuromesh "${ALIAS}"
echo "Linked: ${ALIAS} -> neuromesh"
