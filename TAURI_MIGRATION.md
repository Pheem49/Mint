# Mint Tauri Migration

Mint 2 is being rewritten alongside the existing Electron application. The
Electron code remains available as a behavioral reference until the Tauri
replacement covers the required desktop and CLI workflows.

## New Architecture

- `crates/mint-core`: Shared Rust configuration and domain logic.
- `crates/mint-cli`: Native `mint` command built with Rust.
- `src-tauri`: Tauri v2 desktop backend and IPC commands.
- `src/renderer`: React and TypeScript webview UI built with Vite.

## Commands

```bash
npm install
cargo test -p mint-core -p mint-cli
cargo run -p mint-cli -- status
cargo run -p mint-cli -- memory recent
cargo run -p mint-cli -- chat "Hello"
npm run build:web
npm run tauri:dev
```

## Linux Prerequisites

Tauri desktop builds need native Linux development packages. On Debian,
Ubuntu, or Mint:

```bash
sudo apt-get install -y \
  build-essential \
  curl \
  file \
  pkg-config \
  wget \
  libdbus-1-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

## Migration Order

1. Shared config and safety policy. Initial migration complete.
2. Native CLI command parity. Status, provider, memory, safety, and basic chat commands migrated.
3. Chat and settings UI. Rust-backed dashboard, event-stream delivery, and settings adapter migrated.
4. AI providers and local memory. Provider calls, SQLite conversation context, and orchestration migrated.
5. Desktop integrations. Native tray, widget, spotlight, screen selection capture, workflow monitor,
   allowlisted desktop actions, and transient MCP stdio tool calls migrated.
6. Remove Electron runtime and the legacy JavaScript compatibility UI.

## Remaining Compatibility Work

- Stream provider bytes directly instead of delivering chunks after a provider response completes.
- Port multimodal image translation and continuous screen-region translation.
- Port the remaining legacy desktop actions, OAuth-backed plugins, proactive context engine,
  global shortcuts, and channel bridges.
- Remove Electron dependencies only after the replacement workflows are exercised interactively.
