#!/bin/bash
set -e

echo "=== Installing Mint CLI ==="

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo is not installed."
    echo "Please install Rust first: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Create a temporary directory
TEMP_DIR=$(mktemp -d)
echo "Cloning Mint repository to temporary directory..."
git clone https://github.com/Pheem49/Mint.git "$TEMP_DIR"

cd "$TEMP_DIR"

echo "Building Mint CLI in release mode..."
cargo build --release -p mint-cli

echo "Installing binary to /usr/local/bin..."
# Try copying to /usr/local/bin (may require sudo)
if [ -w /usr/local/bin ]; then
    cp target/release/mint /usr/local/bin/mint
else
    echo "Permission denied for /usr/local/bin. Requesting sudo permission..."
    sudo cp target/release/mint /usr/local/bin/mint
fi

echo "Cleaning up..."
rm -rf "$TEMP_DIR"

echo "=== Mint CLI Installed Successfully! ==="
echo "Type 'mint' to get started."
