#!/bin/bash
set -e

echo "=== Installing Mint CLI via npm ==="

# Detect OS
OS="$(uname -s)"

# Check for Node.js / npm
if ! command -v npm &> /dev/null; then
    echo "Node.js / npm is not installed. It is required to run and manage Mint CLI."
    read -p "Would you like to install Node.js and npm automatically? (Requires sudo on Linux) [Y/n]: " install_node
    install_node=${install_node:-Y}
    
    if [[ "$install_node" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        if [ "$OS" = "Darwin" ]; then
            if command -v brew &> /dev/null; then
                echo "Installing Node.js via Homebrew..."
                brew install node
            else
                echo "Error: Homebrew is not installed. Please install Node.js manually from https://nodejs.org"
                exit 1
            fi
        elif [ "$OS" = "Linux" ]; then
            if command -v apt-get &> /dev/null; then
                echo "Installing Node.js and npm via apt-get..."
                sudo apt-get update
                sudo apt-get install -y nodejs npm
            elif command -v pacman &> /dev/null; then
                echo "Installing Node.js and npm via pacman..."
                sudo pacman -S --noconfirm nodejs npm
            elif command -v dnf &> /dev/null; then
                echo "Installing Node.js and npm via dnf..."
                sudo dnf install -y nodejs npm
            else
                echo "Error: Could not detect your package manager. Please install Node.js manually from https://nodejs.org"
                exit 1
            fi
        else
            echo "Error: OS not supported for automatic Node.js installation. Please install Node.js manually from https://nodejs.org"
            exit 1
        fi
    else
        echo "Installation aborted. Node.js/npm is required."
        exit 1
    fi
fi

# Check for Rust/Cargo (since the npm package compiles from source on postinstall)
if ! command -v cargo &> /dev/null; then
    echo "Rust/Cargo is not installed. It is required to compile Mint CLI."
    read -p "Would you like to install Rust/Cargo automatically via rustup? [Y/n]: " install_rust
    install_rust=${install_rust:-Y}
    
    if [[ "$install_rust" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        echo "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # Load cargo environment in the current shell session
        source "$HOME/.cargo/env"
    else
        echo "Installation aborted. Rust/Cargo is required."
        exit 1
    fi
fi

echo "Running npm install -g @pheem49/mint@latest..."

# Try to install globally
if npm install -g @pheem49/mint@latest; then
    echo ""
    echo "=== Mint CLI Installed Successfully! ==="
    echo "Type 'mint' to get started."
else
    echo ""
    echo "Error: npm installation failed."
    echo "If you encountered permission errors (EACCES), please try running:"
    echo "  sudo npm install -g @pheem49/mint@latest --unsafe-perm"
    exit 1
fi
