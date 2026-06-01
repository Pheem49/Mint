# Mint

Mint is a native desktop AI assistant built with Tauri v2, a Rust backend, and a React
TypeScript webview. The same Rust domain layer powers the desktop application and native CLI.

## Features

- Multi-provider chat: Gemini, OpenAI, Anthropic, Ollama, local endpoints, and Hugging Face.
- Native streaming chat, local SQLite memory, tasks, and searchable local knowledge.
- Screen capture, live translation, spotlight, tray, widget, proactive suggestions, and weather.
- MCP tools, local plugins, Discord Gateway and RPC, Slack Socket Mode, LINE, WhatsApp Cloud API,
  Telegram, and safe Chromium automation.
- Native code-agent workspace inspection and editing with an explicit approval preview.
- Signed Tauri update checks and explicitly approved installation.

## Prerequisites

- Node.js and npm
- Rust toolchain
- Tauri v2 native build dependencies for your operating system

On Debian, Ubuntu, or Linux Mint:

```bash
sudo apt-get install -y \
  build-essential curl file pkg-config wget \
  libdbus-1-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  poppler-utils unzip patchelf
```

## Development

```bash
npm install
npm run tauri:dev
```

Useful validation commands:

```bash
npm run build:web
cargo test -p mint-core -p mint-cli -p mint-desktop
cargo check -p mint-desktop
npm run tauri:build -- --debug --no-bundle
```

## Native CLI

Run the CLI from the workspace:

```bash
npm run cli -- status
npm run cli -- config doctor
npm run cli -- chat "Hello"
npm run cli -- knowledge add README.md
npm run cli -- knowledge search Mint
npm run cli -- code agent "inspect this repo and fix the failing tests"
npm run cli -- run --approve -- "cargo test -p mint-core"
npm run cli -- agent "inspect this workspace"
npm run cli -- learn ./skill.md
npm run cli -- symbols .
npm run cli -- semantic-code index .
npm run cli -- semantic-code search "shell approval flow"
npm run cli -- mcp list
npm run cli -- gmail auth --no-open
npm run cli -- update --check
```

Inside the interactive CLI, use `/code <task>` to enter the autonomous inspect, act, and verify
loop directly. Code-related fix, inspection, and test requests are also routed to Code Agent
automatically. Shell commands and file edits always require explicit terminal approval.

## Project Layout

```text
crates/mint-core   Shared Rust domain logic
crates/mint-cli    Native Rust CLI
src-tauri          Tauri desktop backend and IPC commands
src/renderer       React and TypeScript webview UI
docs               Project documentation
```

## Configuration Notes

Mint stores local configuration under the platform config directory. High-risk system actions
remain blocked by policy. Code edits and updater installation require explicit approval.

LINE and WhatsApp webhook listeners bind to localhost. See
[`docs/WEBHOOK_FORWARDING.md`](docs/WEBHOOK_FORWARDING.md) before exposing them through a TLS
tunnel.

The optional smart-context browser helper can serve active-tab context from
`http://127.0.0.1:3212/context`.

## Migration Status

The historical desktop and Node CLI runtime has been removed. See
[`TAURI_MIGRATION.md`](TAURI_MIGRATION.md) for the current compatibility notes.

## License

AGPL-3.0-only
