#!/usr/bin/env sh
# install.sh — install the b (Bun version manager) binary
set -e

RED='\033[1;31m'
GREEN='\033[1;32m'
CYAN='\033[1;36m'
NC='\033[0m'

REPO="THernandez03/b"
BIN="b"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS
case "$(uname -s)" in
  Linux)  OS="linux" ;;
  Darwin) OS="darwin" ;;
  *)
    printf "${RED}Error: Unsupported OS: $(uname -s)${NC}\n" >&2
    exit 1
    ;;
esac

# Detect architecture
case "$(uname -m)" in
  x86_64 | amd64)   ARCH="x64" ;;
  aarch64 | arm64)  ARCH="arm64" ;;
  *)
    printf "${RED}Error: Unsupported architecture: $(uname -m)${NC}\n" >&2
    exit 1
    ;;
esac

ARTIFACT="${BIN}-${OS}-${ARCH}"

# Fetch the latest release tag from GitHub
printf "${CYAN}Fetching latest release...${NC}\n"
TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$TAG" ]; then
  printf "${RED}Error: Failed to fetch latest release tag.${NC}\n" >&2
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/${ARTIFACT}"

printf "${CYAN}Downloading ${BIN} ${TAG} (${OS}/${ARCH})...${NC}\n"
mkdir -p "$INSTALL_DIR"
curl -fsSL "$URL" -o "${INSTALL_DIR}/${BIN}"
chmod +x "${INSTALL_DIR}/${BIN}"

printf "\n${GREEN}✓ Installed ${BIN} ${TAG} to ${INSTALL_DIR}/${BIN}${NC}\n\n"
printf "Make sure the following are in your PATH:\n"
printf "  ${CYAN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}   # for the b binary\n"
printf "  ${CYAN}export PATH=\"\$HOME/.b/bin:\$PATH\"${NC}        # for managed Bun binaries\n"
