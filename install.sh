#!/bin/bash
# Installs the Mint CLI via npm (the npm package compiles `mint-cli` from source
# on postinstall). This script makes sure every build- and run-time dependency
# that compile needs is present first:
#
#   Required (build fails without them):
#     - a C toolchain            rusqlite (bundled SQLite) + tree-sitter grammars
#     - pkg-config               used to locate ALSA
#     - ALSA dev + runtime libs  cpal / native microphone capture (Linux only)
#     - Node.js & npm            install vehicle + frontend build
#     - Rust toolchain (cargo)   compiles mint-core / mint-cli
#
#   Optional feature tools (installed too, unless MINT_SKIP_OPTIONAL=1):
#     - git                      repo-aware tools
#     - poppler-utils (pdftotext) PDF ingestion for knowledge/search
#     - ffmpeg / ffprobe         video, subtitle and speech tooling
#
#   Not installed here (opt in when you need the feature):
#     - ollama    local LLM provider          https://ollama.com/download
#     - docker    sandboxed shell execution   https://docs.docker.com/engine/install/
#     - a browser (chromium/chrome)           browser-automation tools
#     - whisper CLI                           offline speech-to-text
set -e

NPM_PKG="@pheem49/mint@latest"
SKIP_OPTIONAL="${MINT_SKIP_OPTIONAL:-0}"

OS="$(uname -s)"

have() { command -v "$1" >/dev/null 2>&1; }

ask() {
  # ask "question" -> returns 0 for yes (default yes)
  local reply
  read -r -p "$1 [Y/n]: " reply
  reply="${reply:-Y}"
  [[ "$reply" =~ ^([yY][eE][sS]|[yY])$ ]]
}

echo "=== Installing Mint CLI ==="
echo

# ---------------------------------------------------------------------------
# 1. Detect the Linux package manager (no-op on macOS)
# ---------------------------------------------------------------------------
PM=""
if [ "$OS" = "Linux" ]; then
  if   have apt-get; then PM="apt"
  elif have dnf;     then PM="dnf"
  elif have pacman;  then PM="pacman"
  elif have zypper;  then PM="zypper"
  else
    echo "Warning: could not detect your package manager (apt/dnf/pacman/zypper)."
    echo "Install these manually, then re-run:  a C toolchain, pkg-config, ALSA dev headers."
  fi
fi

pm_install() {
  # pm_install <apt pkgs> ||| <dnf pkgs> ||| <pacman pkgs> ||| <zypper pkgs>
  case "$PM" in
    apt)    sudo apt-get update -qq && sudo apt-get install -y "$@" ;;
    dnf)    sudo dnf install -y "$@" ;;
    pacman) sudo pacman -S --needed --noconfirm "$@" ;;
    zypper) sudo zypper --non-interactive install "$@" ;;
    *)      return 1 ;;
  esac
}

# ---------------------------------------------------------------------------
# 2. System build dependencies
# ---------------------------------------------------------------------------
echo "--- Checking system build dependencies ---"

if [ "$OS" = "Darwin" ]; then
  if ! have cc || ! xcode-select -p >/dev/null 2>&1; then
    echo "Xcode Command Line Tools are required to compile Mint (C toolchain + linker)."
    if ask "Install them now? (opens Apple's installer)"; then
      xcode-select --install || true
      echo
      echo "Finish the Xcode Command Line Tools installer, then re-run this script."
      exit 1
    else
      echo "Installation aborted. Xcode Command Line Tools are required."
      exit 1
    fi
  fi
  echo "OK: Xcode Command Line Tools present."
elif [ "$OS" = "Linux" ]; then
  case "$PM" in
    apt)    pm_install build-essential pkg-config libasound2-dev libasound2 curl file ;;
    dnf)    sudo dnf groupinstall -y "Development Tools" || true
            pm_install pkgconf-pkg-config alsa-lib-devel alsa-lib curl file ;;
    pacman) pm_install base-devel pkgconf alsa-lib curl file ;;
    zypper) sudo zypper --non-interactive install -t pattern devel_basis || true
            pm_install pkg-config alsa-devel libasound2 curl file ;;
  esac
  echo "OK: C toolchain, pkg-config and ALSA libraries installed."
fi
echo

# ---------------------------------------------------------------------------
# 3. Node.js / npm
# ---------------------------------------------------------------------------
echo "--- Checking Node.js / npm ---"
if ! have npm; then
  echo "Node.js / npm is not installed (required to install and manage Mint CLI)."
  if ask "Install Node.js and npm automatically?"; then
    case "$OS" in
      Darwin)
        if have brew; then brew install node
        else echo "Error: Homebrew not found. Install Node.js from https://nodejs.org"; exit 1; fi ;;
      Linux)
        case "$PM" in
          apt)    pm_install nodejs npm ;;
          dnf)    pm_install nodejs npm ;;
          pacman) pm_install nodejs npm ;;
          zypper) pm_install nodejs npm ;;
          *)      echo "Error: install Node.js manually from https://nodejs.org"; exit 1 ;;
        esac ;;
      *) echo "Error: install Node.js manually from https://nodejs.org"; exit 1 ;;
    esac
  else
    echo "Installation aborted. Node.js/npm is required."
    exit 1
  fi
fi
echo "OK: $(node --version 2>/dev/null || echo node) / npm $(npm --version 2>/dev/null)"
echo

# ---------------------------------------------------------------------------
# 4. Rust toolchain
# ---------------------------------------------------------------------------
echo "--- Checking Rust / Cargo ---"
if ! have cargo; then
  echo "Rust / Cargo is not installed (the npm package compiles Mint CLI from source)."
  if ask "Install Rust via rustup?"; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  else
    echo "Installation aborted. Rust/Cargo is required."
    exit 1
  fi
fi
echo "OK: $(cargo --version)"
echo

# ---------------------------------------------------------------------------
# 5. Optional feature tools
# ---------------------------------------------------------------------------
if [ "$SKIP_OPTIONAL" != "1" ]; then
  echo "--- Installing optional feature tools (set MINT_SKIP_OPTIONAL=1 to skip) ---"
  if [ "$OS" = "Darwin" ]; then
    if have brew; then
      brew install git poppler ffmpeg || true
    else
      echo "Homebrew not found — skipping optional tools (git, poppler, ffmpeg)."
    fi
  elif [ "$OS" = "Linux" ]; then
    case "$PM" in
      apt)    pm_install git poppler-utils ffmpeg || true ;;
      dnf)    pm_install git poppler-utils ffmpeg || true ;;
      pacman) pm_install git poppler ffmpeg || true ;;
      zypper) pm_install git poppler-tools ffmpeg || true ;;
    esac
  fi
  echo "OK: optional tools processed."
  echo
fi

# ---------------------------------------------------------------------------
# 6. Install the CLI
# ---------------------------------------------------------------------------
echo "--- Installing $NPM_PKG ---"
if npm install -g "$NPM_PKG"; then
  echo
  echo "=== Mint CLI installed successfully! ==="
  have mint && mint --version 2>/dev/null || true
  echo "Run 'mint' to get started."
else
  echo
  echo "Error: npm installation failed."
  echo "If you hit permission errors (EACCES), try:"
  echo "  sudo npm install -g $NPM_PKG --unsafe-perm"
  exit 1
fi

# ---------------------------------------------------------------------------
# 7. Reminder: heavier, feature-specific tools
# ---------------------------------------------------------------------------
cat <<'EOF'

Optional, install only if you use the feature:
  - ollama   local LLM provider ......... https://ollama.com/download
  - docker   sandboxed shell execution .. https://docs.docker.com/engine/install/
  - chromium / google-chrome ............ browser-automation tools
  - whisper  offline speech-to-text ..... pip install -U openai-whisper
EOF
