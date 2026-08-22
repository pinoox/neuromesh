#!/usr/bin/env bash
# ==============================================================================
# 🌿 NeuroMesh V2 — Zero-Prerequisite Universal Installer (Linux & macOS)
# ==============================================================================
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pinoox/neuromesh/main/install.sh | bash
# ==============================================================================

set -e

REPO="pinoox/neuromesh"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="neuromesh"

# Colors for terminal output
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
printf "${BOLD}🌿 Biomimetic MCP Context Engine & Visual Runtime${NC}\n\n"

# 1. Detect Operating System & Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux*)
        case "${ARCH}" in
            x86_64) TARGET="linux-x86_64" ;;
            aarch64|arm64) TARGET="linux-arm64" ;;
            *) printf "${RED}Error: Unsupported architecture: ${ARCH}${NC}\n"; exit 1 ;;
        esac
        EXT="tar.gz"
        ;;
    Darwin*)
        case "${ARCH}" in
            arm64|aarch64) TARGET="darwin-arm64" ;;
            x86_64) TARGET="darwin-x86_64" ;;
            *) printf "${RED}Error: Unsupported architecture: ${ARCH}${NC}\n"; exit 1 ;;
        esac
        EXT="tar.gz"
        ;;
    *)
        printf "${RED}Error: Unsupported operating system: ${OS}${NC}\n"
        printf "For Windows, run: ${YELLOW}irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex${NC}\n"
        exit 1
        ;;
esac

printf " Detected platform: ${GREEN}${OS} (${ARCH})${NC}\n"

# 2. Get latest release tag from GitHub
printf " Fetching latest release information from GitHub...\n"
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || echo "latest")

if [ -z "${LATEST_RELEASE}" ] || [ "${LATEST_RELEASE}" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/neuromesh-${TARGET}.${EXT}"
else
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_RELEASE}/neuromesh-${TARGET}.${EXT}"
fi

# Fallback directly to repository precompiled assets if release not published yet
TEMP_DIR=$(mktemp -d)
CLEANUP() { rm -rf "${TEMP_DIR}"; }
trap CLEANUP EXIT

printf " Downloading precompiled binary (${TARGET})...\n"
if ! curl -f -L --progress-bar "${DOWNLOAD_URL}" -o "${TEMP_DIR}/neuromesh.${EXT}"; then
    printf "${YELLOW}Release asset not found, checking fallback mirror...${NC}\n"
    # Fallback to main branch archive if release tag is building
    FALLBACK_URL="https://github.com/${REPO}/releases/latest/download/neuromesh-${TARGET}.${EXT}"
    curl -f -L --progress-bar "${FALLBACK_URL}" -o "${TEMP_DIR}/neuromesh.${EXT}" || {
        printf "${RED}Failed to download release binary.${NC}\n"
        printf "You can build directly via Cargo: ${YELLOW}cargo install --git https://github.com/${REPO}.git neuromesh-cli --bin neuromesh${NC}\n"
        exit 1
    }
fi

# 3. Extract and Install
mkdir -p "${INSTALL_DIR}"
tar -xzf "${TEMP_DIR}/neuromesh.${EXT}" -C "${TEMP_DIR}"
chmod +x "${TEMP_DIR}/${BINARY_NAME}"
mv "${TEMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

printf "\n${GREEN}${BOLD}✓ NeuroMesh binary installed successfully to: ${INSTALL_DIR}/${BINARY_NAME}${NC}\n"

# 4. PATH Configuration check
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    printf "\n${YELLOW}⚠️  ${INSTALL_DIR} is not in your current PATH.${NC}\n"
    printf "Add the following line to your ${BOLD}~/.bashrc${NC} or ${BOLD}~/.zshrc${NC}:\n\n"
    printf "  ${CYAN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}\n\n"
    export PATH="${INSTALL_DIR}:${PATH}"
fi

# 5. Verify Installation
printf "\n${BOLD}Verifying installation:${NC}\n"
"${INSTALL_DIR}/${BINARY_NAME}" --help | head -n 8

printf "\n${GREEN}${BOLD}🚀 Quick Start:${NC}\n"
printf "  1. Launch 3D Monitor:  ${CYAN}neuromesh monitor${NC} (Open http://127.0.0.1:8765)\n"
printf "  2. Connect to IDE:     ${CYAN}neuromesh connect${NC}\n"
printf "  3. Index Workspace:    ${CYAN}neuromesh index${NC}\n\n"