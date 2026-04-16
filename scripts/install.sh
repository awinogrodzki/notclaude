#!/bin/bash
#
# Install notclaude — macOS desktop notifications for Claude Code
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/awinogrodzki/notclaude/main/scripts/install.sh | bash
#
# Or clone the repo and run:
#   ./scripts/install.sh
#
set -euo pipefail

INSTALL_DIR="${NOTCLAUDE_INSTALL_DIR:-$HOME/.notclaude/bin}"
REPO="awinogrodzki/notclaude"
BINARY_NAME="notclaude"

info()  { printf '\033[1;34m%s\033[0m\n' "$*"; }
ok()    { printf '\033[1;32m%s\033[0m\n' "$*"; }
err()   { printf '\033[1;31m%s\033[0m\n' "$*" >&2; }

main() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        err "Error: notclaude only supports macOS."
        exit 1
    fi

    local arch
    arch="$(uname -m)"
    case "$arch" in
        arm64|aarch64) arch="arm64" ;;
        x86_64)        arch="x86_64" ;;
        *)
            err "Error: Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    info "Installing notclaude for darwin-${arch}..."

    if try_download "$arch"; then
        ok "Installed notclaude to ${INSTALL_DIR}/${BINARY_NAME}"
        post_install
        return 0
    fi

    info "Pre-built binary not available, trying cargo..."

    if command -v cargo &>/dev/null; then
        cargo install --git "https://github.com/${REPO}"
        ok "Installed notclaude via cargo to $(which notclaude 2>/dev/null || echo '~/.cargo/bin/notclaude')"
        return 0
    fi

    err "Could not download a pre-built binary and cargo is not installed."
    echo ""
    echo "Options:"
    echo "  1. Install Rust: https://rustup.rs"
    echo "     Then run: cargo install --git https://github.com/${REPO}"
    echo ""
    echo "  2. Download manually from:"
    echo "     https://github.com/${REPO}/releases"
    exit 1
}

try_download() {
    local arch="$1"
    local url="https://github.com/${REPO}/releases/latest/download/${BINARY_NAME}-darwin-${arch}"

    mkdir -p "$INSTALL_DIR"

    if ! curl -fsSL "$url" -o "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null; then
        rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        return 1
    fi

    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    # Verify the binary actually runs
    if ! "${INSTALL_DIR}/${BINARY_NAME}" status &>/dev/null; then
        err "Downloaded binary failed verification."
        rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        return 1
    fi

    return 0
}

post_install() {
    if [[ ":$PATH:" == *":${INSTALL_DIR}:"* ]]; then
        return
    fi

    echo ""
    info "Add to your shell profile for global access:"
    echo ""
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
    echo "Or use the full path directly:"
    echo ""
    echo "  ${INSTALL_DIR}/${BINARY_NAME} install --project"
}

main "$@"
