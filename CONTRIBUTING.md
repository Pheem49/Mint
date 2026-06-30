# Contributing to Mint

Thank you for your interest in contributing to Mint! Mint is a privacy-first, local-first AI assistant built with Tauri v2, Rust, and React/TypeScript. 

By contributing, you help make private and autonomous AI agents more accessible to everyone. Since Mint is licensed under the [AGPL-3.0-only license](LICENSE), all contributions will also be licensed under the same terms.

---

## Table of Contents
1. [Code of Conduct](#code-of-conduct)
2. [Areas of Contribution](#areas-of-contribution)
3. [Security & Secrets Policy](#security--secrets-policy)
4. [Development Environment Setup](#development-environment-setup)
5. [Project Architecture](#project-architecture)
6. [Testing & Verification](#testing--verification)
7. [Submitting a Pull Request](#submitting-a-pull-request)

---

## Code of Conduct

We aim to foster an open, welcoming, and inclusive community. Please be respectful, constructive, and supportive of all contributors.

---

## Areas of Contribution

We welcome contributions across all areas of the project! Here are some ways you can get involved:

- **Frontend & UI (`src/renderer`)**: Improve user interfaces, polish styles, build interactive widgets, or optimize Live2D rendering and animations.
- **Core Rust Engine (`crates/mint-core`)**: Enhance agent execution loops, implement new LLM provider integrations, refine local SQLite memory stores, or optimize security checking logic.
- **Tauri Integration (`src-tauri`)**: Improve OS-level features, tray menu interaction, shortcut triggers, or local device capture integrations.
- **Documentation & Workflows**: Write guides, document custom workflows, update READMEs, or help translate Mint into multiple languages.
- **Bug Fixes & Refactoring**: Pick up active issues, optimize memory footprint, or write unit and integration tests.

---

## Security & Secrets Policy

> [!IMPORTANT]
> **Never commit private API keys, credentials, tokens, or personal configurations.**

Before pushing code or making a Pull Request:
1. Ensure your local `.env` and `mint-config.json` files are not tracked by Git (these should be matched by [.gitignore](.gitignore)).
2. Double-check your git staging area (`git status`) to verify that no temporary logs, keys, or credentials are being committed.
3. If you find a security vulnerability, please do **not** open a public issue. Email the project maintainers directly at `killerpheem13@gmail.com` so we can coordinate a fix.

---

## Development Environment Setup

### Prerequisites
To build and run Mint locally, you need the following tools:
- **Rust Toolchain:** Installed via [rustup](https://rustup.rs/) (latest stable version).
- **Node.js & npm:** (Node 18+ recommended).
- **OS Dependencies:**
  - **Linux (Debian/Ubuntu):**
    ```bash
    sudo apt-get install -y build-essential curl file pkg-config wget \
      libdbus-1-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
      librsvg2-dev poppler-utils unzip patchelf
    ```
  - **macOS:** Xcode Command Line Tools.
  - **Windows:** C++ Build Tools via Visual Studio Installer.

### Steps to Run Locally
1. **Clone the repository:**
   ```bash
   git clone https://github.com/Pheem49/Mint.git
   cd Mint
   ```
2. **Install frontend dependencies:**
   ```bash
   npm install
   ```
3. **Configure Environment Variables:**
   ```bash
   cp .env.example .env
   # Add your API keys if you want to use cloud providers (e.g. Gemini, OpenAI)
   ```
4. **Run the Tauri Desktop app (Dev mode):**
   ```bash
   npm run tauri:dev
   ```
5. **Run the CLI helper:**
   ```bash
   npm run cli -- chat "Hello"
   ```

---

## Project Architecture

Mint shares a single Rust codebase for both its GUI and CLI wrappers:
- [crates/mint-core](crates/mint-core): Core domain logic (configurations, memory engine, Ollama/Gemini API bridges, MCP integration, Spotify and Notion plugins).
- [crates/mint-cli](crates/mint-cli): Command Line Interface terminal client.
- [src-tauri](src-tauri): Tauri desktop wrapper, OS-native window configs, tray, and IPC command routes.
- [src/renderer](src/renderer): React & TypeScript frontend application.

---

## Testing & Verification

We enforce linting and testing for all incoming contributions. 

### Running Tests
Make sure all Rust tests pass before proposing changes:
```bash
npm run test
```
Or run cargo tests directly:
```bash
cargo test --all-targets --workspace
```

### Running Checks
Ensure your Tauri desktop and CLI builds successfully:
```bash
cargo check --workspace
npm run build:web
```

---

## Submitting a Pull Request

1. **Fork the repo** and create your branch from `main` (or `Rust` depending on the active branch strategy).
2. **Implement your changes** with clear, descriptive commit messages.
3. **Write tests** for any new core features or bug fixes.
4. **Ensure linting passes** and run the test suite.
5. **Submit a Pull Request (PR)**, describing:
   - What changes were made.
   - Why they were made.
   - Any testing steps you followed.
