#!/usr/bin/env bash
# scripts/CheckDeps.sh
# CheckDeps.sh — verify (and optionally install) local dev dependencies for SketchUploader.
#
# Usage:
#   ./scripts/CheckDeps.sh            # check only (default)
#   ./scripts/CheckDeps.sh --install  # install missing deps (Ubuntu/Debian/WSL, macOS)
#
# What it checks:
#   - Node.js (and npm)
#   - Rust (rustup/cargo)
#   - Tauri system libs (Linux only): GTK/WebKit, appindicator, rsvg, patchelf

set -euo pipefail

usage() {
  cat <<'EOF'
CheckDeps.sh — verify (and optionally install) local dev dependencies for SketchUploader.

Usage:
  ./scripts/CheckDeps.sh            # check only (default)
  ./scripts/CheckDeps.sh --install  # install missing deps (Ubuntu/Debian/WSL, macOS)

What it checks:
  - Node.js (and npm)
  - Rust (rustup/cargo)
  - Tauri system libs (Linux only): GTK/WebKit, appindicator, rsvg, patchelf
EOF
}

INSTALL=0

# Parse args (if any). With no args, do nothing (check-only).
if [ "$#" -gt 0 ]; then
  for arg in "$@"; do
    case "$arg" in
      -h|--help) usage; exit 0 ;;
      --install) INSTALL=1 ;;
      *) echo "Unknown arg: $arg"; usage; exit 1 ;;
    esac
  done
fi

have() { command -v "$1" >/dev/null 2>&1; }

echo "Checking development prerequisites..."

have_node=0; have_npm=0; have_rustup=0; have_cargo=0
if have node; then have_node=1; fi
if have npm; then have_npm=1; fi
if have rustup; then have_rustup=1; fi
if have cargo; then have_cargo=1; fi

status() {
  if [ "$2" -eq 1 ]; then
    printf "  ✔ %s\n" "$1"
  else
    printf "  ✗ %s\n" "$1"
  fi
}

status "Node.js" "$have_node"
status "npm" "$have_npm"
status "Rust (rustup)" "$have_rustup"
status "Cargo" "$have_cargo"

OS="$(uname -s)"
if [ "$OS" = "Linux" ]; then
  echo "Linux detected. Tauri requires GTK/WebKit runtime dev packages."
  echo "On Debian/Ubuntu, install (with sudo):"
  echo "  apt-get update && apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf"
fi

if [ "$INSTALL" -eq 1 ]; then
  if [ "$OS" = "Darwin" ]; then
    if ! have brew; then
      echo "Homebrew not found. Please install Homebrew first: https://brew.sh"
      exit 1
    fi
    [ "$have_node" -eq 1 ]    || brew install node@lts
    [ "$have_rustup" -eq 1 ]  || brew install rustup-init && rustup-init -y
    echo "On macOS, Tauri system deps are usually satisfied by Xcode Command Line Tools."
  elif [ "$OS" = "Linux" ]; then
    if have apt-get; then
      if [ "$have_node" -eq 0 ]; then
        echo "Installing Node.js LTS via apt (NodeSource recommended if older distro)..."
        apt-get update && apt-get install -y nodejs npm || true
      fi
      if [ "$have_rustup" -eq 0 ]; then
        echo "Installing Rust (rustup)..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        export PATH="$HOME/.cargo/bin:$PATH"
      fi
      echo "Installing Tauri Linux dependencies..."
      apt-get update && apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
    else
      echo "Unknown Linux distro. Please install Node.js, Rust, and the GTK/WebKit deps manually."
    fi
  else
    echo "Install mode on this OS is not implemented in this script."
  fi
fi

echo
echo "If you just installed tools, restart your shell so PATH updates apply."
echo "Then run:"
echo "  npm install"
echo "  npm run tauri dev"
