#!/usr/bin/env bash
# ==============================================================================
# NeuroMesh — Zero-prerequisite installer (Linux & macOS)
# Downloads the latest pre-built release binary (MiniLM embeddings included).
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pinoox/neuromesh/main/install.sh | bash
# ==============================================================================

set -e

REPO="pinoox/neuromesh"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="neuromesh"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

printf "${CYAN}${BOLD}"
cat << 'EOF'
  _   _                      __  __           _
 | \ | | ___ _   _ _ __ ___ |  \/  | ___  ___| |__
 |  \| |/ _ \ | | | '__/ _ \| |\/| |/ _ \/ __| '_ \
 | |\  |  __/ |_| | | | (_) | |  | |  __/\__ \ | | |
 |_| \_|\___|\__,_|_|  \___/|_|  |_|\___||___/_| |_|
EOF
printf "${NC}\n"
printf "${BOLD}NeuroMesh v0.8.6 — MCP context engine (MiniLM embeddings built in)${NC}\n\n"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux*)
        case "${ARCH}" in
            x86_64) TARGET="linux-x86_64" ;;
            aarch64|arm64) TARGET="linux-arm64" ;;
            *) printf "${RED}Unsupported architecture: ${ARCH}${NC}\n"; exit 1 ;;
        esac
        EXT="tar.gz"
        ;;
    Darwin*)
        case "${ARCH}" in
            arm64|aarch64) TARGET="darwin-arm64" ;;
            x86_64) TARGET="darwin-x86_64" ;;
            *) printf "${RED}Unsupported architecture: ${ARCH}${NC}\n"; exit 1 ;;
        esac
        EXT="tar.gz"
        ;;
    *)
        printf "${RED}Unsupported OS: ${OS}${NC}\n"
        printf "On Windows run: ${YELLOW}irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex${NC}\n"
        exit 1
        ;;
esac

printf " Platform: ${GREEN}${OS} (${ARCH}) → neuromesh-${TARGET}.${EXT}${NC}\n"

printf " Fetching latest release…\n"
LATEST_RELEASE=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

if [ -n "${LATEST_RELEASE}" ]; then
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_RELEASE}/neuromesh-${TARGET}.${EXT}"
    printf " Release: ${GREEN}${LATEST_RELEASE}${NC}\n"
else
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/neuromesh-${TARGET}.${EXT}"
fi

TEMP_DIR=$(mktemp -d)
trap 'rm -rf "${TEMP_DIR}"' EXIT

printf " Downloading…\n"
if ! curl -fSL --progress-bar "${DOWNLOAD_URL}" -o "${TEMP_DIR}/neuromesh.${EXT}"; then
    printf "${RED}Download failed.${NC}\n"
    printf "Check ${CYAN}https://github.com/${REPO}/releases${NC} or build from source:\n"
    printf "  ${YELLOW}cargo install --git https://github.com/${REPO}.git neuromesh-cli --bin neuromesh --features embeddings${NC}\n"
    exit 1
fi

mkdir -p "${INSTALL_DIR}"
tar -xzf "${TEMP_DIR}/neuromesh.${EXT}" -C "${TEMP_DIR}"
chmod +x "${TEMP_DIR}/${BINARY_NAME}"
mv "${TEMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
ln -sf "${BINARY_NAME}" "${INSTALL_DIR}/nmx"

if [ -d "${TEMP_DIR}/models/minilm-multilingual-q" ]; then
    mkdir -p "${INSTALL_DIR}/models/minilm-multilingual-q"
    cp -r "${TEMP_DIR}/models/minilm-multilingual-q/." "${INSTALL_DIR}/models/minilm-multilingual-q/"
    printf "${GREEN}✓ MiniLM weights bundled next to binary${NC}\n"
fi

printf "\n${GREEN}${BOLD}✓ Installed: ${INSTALL_DIR}/${BINARY_NAME} (alias: nmx)${NC}\n"

# PATH — append to shell rc when missing
PATH_LINE='export PATH="$HOME/.local/bin:$PATH"'
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    export PATH="${INSTALL_DIR}:${PATH}"
    SHELL_RC=""
    if [ -n "${ZSH_VERSION:-}" ] || [ "$(basename "${SHELL:-}")" = "zsh" ]; then
        SHELL_RC="${HOME}/.zshrc"
    elif [ -f "${HOME}/.bashrc" ]; then
        SHELL_RC="${HOME}/.bashrc"
    fi
    if [ -n "${SHELL_RC}" ] && ! grep -q '.local/bin' "${SHELL_RC}" 2>/dev/null; then
        printf "\n${CYAN}Adding ~/.local/bin to ${SHELL_RC}${NC}\n"
        printf '\n# NeuroMesh\n%s\n' "${PATH_LINE}" >> "${SHELL_RC}"
    else
        printf "\n${YELLOW}Add to PATH:${NC} ${CYAN}${PATH_LINE}${NC}\n"
    fi
fi

VERSION=$("${INSTALL_DIR}/${BINARY_NAME}" -V 2>/dev/null || "${INSTALL_DIR}/${BINARY_NAME}" --version 2>/dev/null || echo "unknown")
printf "\n${BOLD}Version:${NC} ${VERSION}\n"

printf "\n${GREEN}${BOLD}Quick start${NC}\n"
printf "  1. ${CYAN}neuromesh doctor${NC}       verify install\n"
printf "  2. ${CYAN}neuromesh connect${NC}     wire Cursor / VS Code / Claude MCP\n"
printf "  3. ${CYAN}neuromesh index${NC}        index your repo\n"
printf "  4. ${CYAN}neuromesh monitor${NC}      3D galaxy UI → http://127.0.0.1:8765\n\n"
