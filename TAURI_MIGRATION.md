# Mint Tauri Migration

Mint 2 now runs on Tauri v2 with a Rust backend and a React TypeScript webview.
The historical desktop and Node CLI runtime has been removed after the native
replacement covered the required desktop and CLI workflows.

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
cargo run -p mint-cli -- config doctor
cargo run -p mint-cli -- knowledge add README.md
cargo run -p mint-cli -- knowledge search Mint
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
  poppler-utils \
  unzip \
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
   plugins, safe Chromium DevTools automation, proactive screen suggestions, picture library,
   Google TTS URL generation, multimodal Gemini chat, system event monitoring, opt-in headless
   task processing, PDF/DOCX/XLSX local knowledge extraction, and read-only native code workspace
   inspection migrated. Signed Tauri update check and explicitly approved installation are
   available after a release endpoint and signing key are configured.
6. Historical desktop runtime, legacy npm scripts, compatibility sources, and Node-only
   dependencies removed.

## Remaining Compatibility Work

- Exercise signed updater installation against a published release endpoint and public key.
- Package a browser extension that serves the optional smart-context fallback endpoint.

## Webhook Forwarding

Mint binds webhook listeners to localhost so the desktop backend is not directly exposed to the
network. Use a TLS forwarding service such as Cloudflare Tunnel or ngrok and keep signature
verification enabled. See [`docs/WEBHOOK_FORWARDING.md`](docs/WEBHOOK_FORWARDING.md) for the
standalone setup guide.

LINE listens on `http://127.0.0.1:3000/callback`:

```bash
cloudflared tunnel --url http://127.0.0.1:3000
```

Register the resulting HTTPS URL with `/callback` appended in the LINE Developers Console.
Configure `lineChannelAccessToken`, `lineChannelSecret`, and `enableLineBridge` in Mint settings.

WhatsApp Cloud listens on `http://127.0.0.1:3001/`:

```bash
cloudflared tunnel --url http://127.0.0.1:3001
```

Register the resulting HTTPS URL in Meta Webhooks. Configure `whatsappCloudAccessToken`,
`whatsappPhoneNumberId`, `whatsappVerifyToken`, `whatsappAppSecret`, and
`enableWhatsappBridge`. The verify token must match Meta's subscription request.

## Browser Extension Context Fallback

When Chromium remote debugging is unavailable, Mint attempts to read the active tab from
`http://127.0.0.1:3212/context`. A browser extension helper may serve JSON in this format:

```json
{ "title": "Active page", "url": "https://example.com/" }
```
