#!/bin/bash
# Mint CLI Installation Script
# Usage: curl -fsSL <URL>/install.sh | bash

set -e

echo "Starting Mint CLI installation..."

# 1. Check for Node.js
if ! command -v node &> /dev/null
then
    echo "[Error] Node.js is not found on your system!"
    echo "Please install Node.js (version 18 or higher) before using Mint CLI."
    echo "Download at: https://nodejs.org/"
    exit 1
fi

# 2. Check for NPM
if ! command -v npm &> /dev/null
then
    echo "[Error] NPM is not found on your system!"
    exit 1
fi

# Check Node.js version (recommend 18+)
NODE_VERSION=$(node -v | cut -d 'v' -f 2 | cut -d '.' -f 1)
if [ "$NODE_VERSION" -lt 18 ]; then
    echo "[Warning] Your Node.js version is too old (current: v${NODE_VERSION})"
    echo "It is recommended to update to version 18 or higher for full functionality."
fi

# 3. Install Mint CLI via NPM
echo "Downloading and installing @pheem49/mint (may require Admin/Root privileges)..."

# Use sudo automatically if not root and not on Windows (Msys/Cygwin)
if [ "$EUID" -ne 0 ] && command -v sudo &> /dev/null; then
  sudo npm install -g @pheem49/mint@latest
else
  npm install -g @pheem49/mint@latest
fi

echo ""
echo "Mint CLI installed successfully!"
echo "- Run 'mint' to get started."
echo "- Run 'mint onboard' to set up your API Keys."
echo "----------------------------------------"
echo "Happy coding!"
