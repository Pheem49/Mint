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

# Prefix for privileged commands. Empty when we are already root (common in
# containers, where `sudo` frequently isn't installed); `sudo` otherwise.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  if have sudo; then
    SUDO="sudo"
  else
    echo "Warning: not root and 'sudo' is not installed — package installs may fail."
    echo "Re-run as root or install sudo if you hit permission errors below."
  fi
fi

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
    apt)    $SUDO apt-get update -qq && $SUDO apt-get install -y "$@" ;;
    dnf)    $SUDO dnf install -y "$@" ;;
    pacman) $SUDO pacman -S --needed --noconfirm "$@" ;;
    zypper) $SUDO zypper --non-interactive install "$@" ;;
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
  deps_ok=1
  case "$PM" in
    apt)    pm_install build-essential pkg-config libasound2-dev libasound2 curl file ;;
    dnf)    $SUDO dnf groupinstall -y "Development Tools" || true
            pm_install pkgconf-pkg-config alsa-lib-devel alsa-lib curl file ;;
    pacman) pm_install base-devel pkgconf alsa-lib curl file ;;
    zypper) $SUDO zypper --non-interactive install -t pattern devel_basis || true
            pm_install pkg-config alsa-devel libasound2 curl file ;;
    *)      deps_ok=0
            echo "Could not auto-install system build dependencies (unknown package manager)."
            echo "Install these manually first, then re-run:"
            echo "  - a C toolchain (gcc/clang + make)"
            echo "  - pkg-config"
            echo "  - ALSA dev + runtime libs (libasound2-dev / alsa-lib-devel / alsa-lib)"
            ask "Continue anyway?" || exit 1 ;;
  esac
  if [ "$deps_ok" = 1 ]; then
    echo "OK: C toolchain, pkg-config and ALSA libraries installed."
  fi
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
        if have brew; then
          brew install node
        else
          echo "Homebrew not found — installing the official Node.js package."
          node_pkg="$(mktemp -d)/node.pkg"
          if curl -fsSL "https://nodejs.org/dist/v22.11.0/node-v22.11.0.pkg" -o "$node_pkg"; then
            $SUDO installer -pkg "$node_pkg" -target /
            rm -f "$node_pkg"
            hash -r 2>/dev/null || true
          else
            echo "Error: could not download Node.js. Install it from https://nodejs.org and re-run."
            exit 1
          fi
        fi ;;
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
      echo "Homebrew not found — skipping poppler / ffmpeg (git ships with the Xcode tools)."
      echo "For those: install Homebrew (https://brew.sh), then 'brew install poppler ffmpeg'."
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
# 7. Feature-tool status report
# ---------------------------------------------------------------------------
echo
echo "Optional feature tools (Mint runs without them; each unlocks a feature):"

report_tool() {
  # report_tool <command> <description> <install hint>
  if command -v "$1" >/dev/null 2>&1; then
    printf '  [ok]      %-10s %s\n' "$1" "$2"
  else
    printf '  [missing] %-10s %s  -> %s\n' "$1" "$2" "$3"
  fi
}

report_tool git       "repo-aware tools"           "https://git-scm.com/downloads"
report_tool ffmpeg    "video / subtitle / speech"  "https://ffmpeg.org/download.html"
report_tool pdftotext "PDF ingestion (poppler)"    "install poppler-utils / poppler"
report_tool ollama    "local LLM provider"         "https://ollama.com/download"
report_tool docker    "sandboxed shell execution"  "https://docs.docker.com/engine/install/"
report_tool whisper   "offline speech-to-text"     "pip install -U openai-whisper"

if command -v chromium >/dev/null 2>&1 || command -v chromium-browser >/dev/null 2>&1 \
  || command -v google-chrome >/dev/null 2>&1 || command -v google-chrome-stable >/dev/null 2>&1 \
  || command -v chrome >/dev/null 2>&1; then
  printf '  [ok]      %-10s %s\n' "browser" "browser-automation tools"
else
  printf '  [missing] %-10s %s  -> %s\n' "browser" "browser-automation tools" "install Chromium or Google Chrome"
fi
