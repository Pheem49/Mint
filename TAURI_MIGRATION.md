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
cargo run -p mint-cli -- task list
cargo run -p mint-cli -- plugin list
cargo run -p mint-cli -- files find Cargo.toml --root .
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
2. Native CLI command parity. Status, provider, config, memory, task, safety, file, plugin, and chat commands migrated.
3. Chat and settings UI. Rust-backed dashboard, event-stream delivery, and settings adapter migrated.
4. AI providers and local memory. Provider calls, SQLite conversation context, and orchestration migrated.
5. Desktop integrations. Native tray, global shortcuts, widget, spotlight, screen selection capture,
   live Gemini translation, workflow monitor, smart context, allowlisted desktop actions,
   system automation, transient MCP stdio tool calls, Telegram long-polling, Discord Gateway,
   Slack Socket Mode, LINE webhook, WhatsApp Cloud API webhook, selected HTTP plugins, local
   plugins, safe Chromium DevTools automation, and proactive screen suggestions migrated.
6. Remove Electron runtime and the legacy JavaScript compatibility UI.

## Remaining Compatibility Work

- Add UI onboarding fields for WhatsApp Cloud API credentials and webhook forwarding URLs.
- Extend smart context with a browser extension fallback when Chromium remote debugging is unavailable.
- Port advanced code-agent and headless-agent workflows that still live under `src/CLI` and
  `src/AI_Brain`.
- Replace the remaining Electron-only onboarding, updater, picture store, TTS, and system-event
  adapters before deleting their TypeScript sources.
- Remove Electron dependencies only after the replacement workflows are exercised interactively.
