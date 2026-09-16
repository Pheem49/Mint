# Contributing to Mint

Thank you for your interest in contributing to Mint! Mint is a privacy-first, local-first AI assistant built with Tauri v2, Rust, and React/TypeScript. 

By contributing, you help make private and autonomous AI agents more accessible to everyone. Since Mint is licensed under the [AGPL-3.0-only license](LICENSE), all contributions will also be licensed under the same terms.

---

## Table of Contents
1. [Code of Conduct](#code-of-conduct)
2. [Areas of Contribution](#areas-of-contribution)
3. [Security & Secrets Policy](#security--secrets-policy)
4. [Development Environment Setup](#development-environment-setup)
5. [The Platform Parity Rule](#the-platform-parity-rule)
6. [Project Architecture](#project-architecture)
7. [Testing & Verification](#testing--verification)
8. [Submitting a Pull Request](#submitting-a-pull-request)

---

## Code of Conduct

We aim to foster an open, welcoming, and inclusive community. Please be respectful, constructive, and supportive of all contributors.

---

## Areas of Contribution

We welcome contributions across all areas of the project! Here are some ways you can get involved:

- **Frontend & UI (`src/renderer`)**: Improve user interfaces, polish styles, build interactive widgets in `src/renderer/shared`, or optimize Desktop (`src`) and Web (`src-web`) dashboards.
- **Core Rust Engine (`crates/mint-core`)**: Enhance the Agent Execution Harness, code intelligence, Git safety checkpoints, documentation search, LLM provider integrations, or memory compaction.
- **CLI Terminal (`crates/mint-cli`)**: Refine terminal interactive TUI, ANSI observability cards, one-shot prompt ergonomics, or subcommand workflows.
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

---

## The Platform Parity Rule

> [!IMPORTANT]
> **Every feature, tool capability, slash command, or UI control MUST maintain feature parity across all three primary interfaces.**

Whenever implementing a new feature, modifying behavior, or adding slash commands and UI options in this codebase, you must audit and ensure complete feature parity across:
1. **CLI (`crates/mint-cli`)**: Terminal TUI, interactive prompt selector, ANSI status cards, and global flags.
2. **Desktop UI (`src/renderer/src`)**: Tauri v2 desktop application.
3. **Web UI (`src/renderer/src-web`)**: Browser web application.

Shared backend logic must reside in **`crates/mint-core`**, and shared UI components and CSS theme tokens must reside in **`src/renderer/shared`**, guaranteeing that no platform is left out.

---

## Project Architecture

Mint shares a single Rust codebase for its GUI and CLI wrappers, paired with a React/TypeScript frontend:
- [crates/mint-core](crates/mint-core): Core domain logic, Agent Execution Harness (ReAct loop, Git checkpoints, AST symbols, knowledge engine, safe tools, two-tier memory, plugins).
- [crates/mint-cli](crates/mint-cli): Command Line Interface terminal client with interactive TUI and ANSI telemetry dashboards.
- [src-tauri](src-tauri): Tauri desktop wrapper, OS-native window configs, tray, and IPC command routes.
- [src/renderer/src](src/renderer/src): React & TypeScript desktop frontend application (Tauri).
- [src/renderer/src-web](src/renderer/src-web): React & TypeScript web frontend application (Browser).
- [src/renderer/shared](src/renderer/shared): Reusable components, CSS theme tokens, hooks, and types shared across Desktop and Web.
- [benchmarks](benchmarks): Automated evaluation benchmark suites for `mint eval`.

---

## Testing & Verification

We enforce strict linting, type-checking, and testing for all incoming contributions across all layers.

### Frontend Type-checking & Builds
Verify that TypeScript compiles and both Web and Desktop UI bundles build cleanly without errors:
```bash
# Type-check TypeScript across shared, desktop, and web
npm run typecheck

# Verify Web UI bundle build
npm run build:web

# Verify Desktop UI bundle build
npm run build:desktop:ui
```

### Backend Rust Checks & Tests
Make sure all Rust tests and checks pass:
```bash
# Compile and check all workspace crates
cargo check --workspace

# Run all unit and integration tests
cargo test --all-targets --workspace
```

### Benchmark Evaluation (Optional)
If modifying agent orchestration, tool actuators, or prompting:
```bash
mint eval --suite benchmarks/mint_eval.json
```

---

## Submitting a Pull Request

1. **Fork the repo** and create your branch from `main` (or active working branch).
2. **Implement your changes** with clear, descriptive commit messages.
3. **Verify Platform Parity**: Confirm your changes are accessible and functional across CLI, Desktop, and Web.
4. **Write tests** for any new core features or bug fixes.
5. **Ensure all checks pass**: Run `npm run typecheck`, `npm run build:web`, `npm run build:desktop:ui`, `cargo check --workspace`, and `cargo test --all-targets --workspace`.
6. **Submit a Pull Request (PR)**, describing:
   - What changes were made.
   - Why they were made.
   - Confirmation of platform parity across CLI, Desktop, and Web.
   - Testing steps followed.

