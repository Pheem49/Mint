<div align="center">
  <img src="assets/icon.png" alt="Mint icon" width="112" />

  # Mint

  **Your AI agent, reachable from Telegram, Discord, Slack, LINE, or WhatsApp — not just a terminal window.**

  [![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)
  [![Rust](https://img.shields.io/badge/Rust-backend-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
  [![React](https://img.shields.io/badge/React-TypeScript-149ECA?logo=react&logoColor=white)](https://react.dev/)
  [![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
</div>

Mint is a local-first AI assistant that runs on your own machine and follows you
wherever you already are: message it from Telegram, Discord, Slack, LINE, or
WhatsApp like you'd message a person, no desktop window required. It's also a
native desktop app with a Live2D companion, and a full terminal agent for
coding tasks — all backed by the same Tauri v2 + Rust + React/TypeScript core,
so chat, memory, knowledge, tools, and safety policies behave identically no
matter which door you walk in through.

See [Release Notes](Release_Note.md) for what's new.

## <img src="assets/features.svg" width="24" height="24" valign="middle" /> What Mint Can Do

Mint is a local-first AI assistant running on your machine, capable of handling tasks from a messaging app, the desktop application, or the terminal interface (CLI):

---

### 1. <img src="assets/bridges.svg" width="18" height="18" valign="middle" /> Reach Mint From Anywhere — Messaging Bridges
- Message it like a person from **Telegram, Discord (Gateway + RPC), Slack, LINE, WhatsApp, Signal, and Email (Gmail)** — no desktop window required. Each bridge locks to whoever messages it first, and all of them share one continuous memory/conversation with the terminal CLI.
- Runs unattended 24/7 on a VPS via `mint gateway start`/`install` — a systemd service with a `GET /api/gateway/health` endpoint. See [Running Mint 24/7 on a VPS](#running-mint-247-on-a-vps-headless-gateway).

---

### 2. <img src="assets/live2d.svg" width="18" height="18" valign="middle" /> Interactive Live2D Desktop Assistant
- An interactive anime avatar (**Shiroko**) on your desktop with gaze tracking, expression/accessory toggles, and interaction zones (Head, Cheek, Hands, Body) that trigger animations and message toasts.

---

### 3. <img src="assets/chat.svg" width="18" height="18" valign="middle" /> AI Chat & Multi-Providers
- Connect to **Gemini, OpenAI, Anthropic (Claude), Ollama (Local), Hugging Face**, and OpenAI-compatible custom endpoints — system instructions, temperature control, voice replies, and multimodal image analysis.

---

### 4. <img src="assets/code.svg" width="18" height="18" valign="middle" /> Autonomous Code Agent & Subagents
- Run code-agent loops via `/code <task>` or `mint code agent "<task>"`: scan the workspace, plan multi-file changes, edit, run tests/shell commands, and verify before finishing.
- Delegate focused sub-tasks to specialized subagents (`dispatch_subagent`), optionally isolated in a per-session Docker container (`sandboxBackend: "docker"`).
> [!IMPORTANT]
> **Safety First:** Risky actions and file writes require your explicit approval first.

---

### 5. <img src="assets/memory.svg" width="18" height="18" valign="middle" /> Memory, Knowledge & Self-Written Skills
- Persistent conversation memory (SQLite), a searchable local knowledge base, and semantic code search.
- After solving a hard, reusable problem, the agent can write its own skill (`.agents/skills/`) — and genuinely refine an existing one instead of duplicating it, the next time a similar task recurs.

---

### 6. <img src="assets/tools.svg" width="18" height="18" valign="middle" /> Scheduled Tasks & Linked Folders
- `mint cron` runs agent tasks on a schedule with no OS-level daemon — rides along on whatever's already open, or `mint gateway start` for always-on.
- Link a folder (e.g. "Food") and chat that touches its topic gets a short, cross-referenced note written into it automatically.

---

### 7. <img src="assets/tools.svg" width="18" height="18" valign="middle" /> Tool & MCP Integrations
- **Model Context Protocol (MCP)** servers for Search, Filesystem, GitHub, and more, plus local plugins for Spotify, Google Calendar, Gmail, and Notion — manage all of it interactively with `mint plugins`.
- Dedicated **Image Search** tool and an **Auto GitHub Link Resolver** that injects a linked repo's metadata/README as context automatically.

---

### 8. <img src="assets/screencapture.svg" width="18" height="18" valign="middle" /> Screen Capture & Translation
- Capture screen snapshots for instant visual analysis, or run real-time continuous overlay translation of a screen region.

---

### 9. <img src="assets/imagegen.svg" width="18" height="18" valign="middle" /> AI Image Generation
- Generate images from chat or terminal using **DALL-E 3, Stability AI, Ideogram, Replicate (Flux)**, and Google NanoBanana — aspect ratio, negative prompts, and automatic local storage.

---

### 10. <img src="assets/tools.svg" width="18" height="18" valign="middle" /> Browser Automation (`mint auto`)
- Drives a dedicated, isolated Chromium instance (port `9222`): open URLs, click, type, and extract page content — the agent registers these tools automatically once it detects the automation browser is running.

---

### 11. <img src="assets/tools.svg" width="18" height="18" valign="middle" /> AI Video Editing via FableMint
- Connect **[FableMint](https://github.com/Pheem49/FableMint)** — a free, open-source browser video editor — as an MCP server, and Mint can cut, grade, caption, chroma-key, and export edits from plain chat, with the open editor tab live-reloading as it works:
  ```bash
  mint mcp add fablemint node --args "<path-to>/FableMint/mcp-server.js"
  mint mcp allow fablemint "*"
  ```
  See [FableMint's README](https://github.com/Pheem49/FableMint#driving-it-with-an-ai-agent) for the full tool list.

## <img src="assets/setup.svg" width="24" height="24" valign="middle" /> Prerequisites

Before you can build or run Mint locally, make sure you have the following system tools installed:

| Tool | Description | Required For |
| :--- | :--- | :--- |
| **Node.js & npm** | JavaScript runtime and package manager | Frontend UI (React, Vite, TypeScript) |
| **Rust Toolchain** | Rust compiler (`rustc`) and package manager (`cargo`) | Shared domain logic, CLI, and Tauri backend |
| **System Dependencies** | Native OS libraries (compiler tools, dbus, webkit) | Compiling window GUI, Webview rendering, and OS utilities |

### Linux Dependencies

Install the required C compilers, WebKitGTK, and system libraries for your specific Linux distribution:

**Debian / Ubuntu / Linux Mint:**
```bash
sudo apt-get install -y \
  build-essential curl file pkg-config wget \
  libdbus-1-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev \
  poppler-utils unzip patchelf
```

**Fedora / RHEL / CentOS:**
```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y \
  webkit2gtk4.1-devel openssl-devel curl wget glibc-devel \
  dbus-devel libayatana-appindicator-devel librsvg2-devel \
  alsa-lib-devel \
  poppler-utils unzip patchelf
```

**Arch Linux:**
```bash
sudo pacman -Syu --needed \
  base-devel webkit2gtk-4.1 openssl curl wget \
  dbus libayatana-appindicator librsvg \
  alsa-lib \
  poppler unzip patchelf
```

> [!NOTE]
> ALSA development headers (`libasound2-dev`/`alsa-lib-devel`/`alsa-lib`) are required to build `cpal`, used for native microphone capture in the desktop app's voice input feature.

> [!TIP]
> **Other Platforms:** If you are developing on macOS or Windows, follow the official [Tauri Prerequisites Guide](https://v2.tauri.app/start/prerequisites/) to set up your build environment.

## Installation

### Quick Install (Recommended)
The easiest way to install Mint CLI is using our installation script:

**For macOS & Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash
```

**For Windows (PowerShell):**
```powershell
powershell -Command "iwr -useb https://raw.githubusercontent.com/Pheem49/Mint/main/install.ps1 | iex"
```

---
### Quick Start
```bash
mint onboard
mint setup
mint 
mint web
mint chat "Hello"
mint imagine "A futuristic mint-colored robot" --aspect 16:9
```

Most integrations can be configured from:
```bash
mint onboard
mint setup
mint
```

### Manual Installation

### 1. Configure API Keys
Copy the template and configure your LLM credentials (Gemini, OpenAI, Anthropic, etc.):
```bash
cp .env.example .env
```
Open the `.env` file and insert your API keys (e.g. `GEMINI_API_KEY=your_key_here`).

### 2. Desktop Application
Install the dependencies and start the application in development mode:
```bash
npm install
npm run tauri:dev
```
To compile and build a production standalone desktop package:
```bash
npm run tauri:build
```
*(The Vite renderer output is generated in `out/renderer` and can be manually built via `npm run build:web`)*

### 3. Native CLI
Pick one way to get the global `mint` command:

* **Release build (recommended — fastest to run):**
  ```bash
  cargo build --release -p mint-cli
  sudo cp target/release/mint /usr/local/bin/
  ```
* **Cargo install:**
  ```bash
  cargo install --path crates/mint-cli
  ```
  *(make sure `~/.cargo/bin` is on your shell's `$PATH`)*
* **Dev alias** — recompiles on every run, so code changes apply instantly; best while actively editing Mint itself:
  ```bash
  echo 'alias mint="cargo run --manifest-path $(pwd)/Cargo.toml -p mint-cli --"' >> ~/.bashrc  # or ~/.zshrc
  source ~/.bashrc  # or ~/.zshrc
  ```

No alias set up? Everything below still works via `npm run cli -- <command>` in place of `mint <command>`.

## User Interface

### Desktop App
<table width="100%">
  <tr>
    <td align="center" width="50%"><b>Desktop Assistant</b></td>
    <td align="center" width="50%"><b>Settings</b></td>
  </tr>
  <tr>
    <td><img src="assets/Mint_Desktop.png" alt="Desktop Assistant" /></td>
    <td><img src="assets/Setting.png" alt="Settings" /></td>
  </tr>
</table>

### Web UI
<table width="100%">
  <tr>
    <td align="center"><img src="assets/Mint_Web.png" alt="Web UI" /></td>
  </tr>
</table>

### CLI
<table width="100%">
  <tr>
    <td align="center"><img src="assets/Mint_CLI.png" alt="CLI" /></td>
  </tr>
</table>

### Workspace & Agent
<table width="100%">
  <tr>
    <td align="center"><img src="assets/Mint_WorkSpace.png" alt="Workspace & Agent" /></td>
  </tr>
</table>

## Desktop Assistant

The desktop app adds Spotlight, a system tray widget, and a background
task-queue window on top of everything in "What Mint Can Do" above. The
sidebar, Live2D interaction state, and area-guide visibility persist locally,
so the dashboard restores its previous state after a restart.

## Native CLI

You can interact with Mint's Rust backend directly using the command line —
install the `mint` shortcut in [Installation](#3-native-cli) above, or fall
back to `npm run cli -- <command>` in its place.

### Start Interactive Chat Assistant

To start the interactive terminal AI chatbot assistant, simply run:

```bash
mint
# Or fallback: npm run cli
```
This opens the Mint interactive shell, where you can type prompts naturally or use `/commands` (like `/help`, `/cd`, `/clear`, `/exit`).

---

### CLI Subcommands

You can run individual subcommands by appending them after `mint`:

```bash
mint onboard
mint setup
mint plugins
mint status
mint web
mint api
mint auto
mint chat "<message>"
```

### Common Commands

| Command | Purpose |
| --- | --- |
| `mint` | Start the interactive terminal chat assistant |
| `mint onboard` | Configure Mint for first use |
| `mint setup` | Interactively manage enabled agent tools |
| `mint plugins` | Centralized interactive management for built-in ecosystem plugins & skills |
| `mint web` | Launch the web UI and local API server |
| `mint api` | Start only the local API server |
| `mint gateway start` | Run headless: bridges + cron, no TUI — for VPS/systemd use |
| `mint gateway start --api-port <N>` | Same, plus the local API/WebUI on port `<N>` |
| `mint gateway install [--system] [--now] [--memory-max <size>]` | Register `mint gateway start` as a systemd unit |
| `mint auto` | Launch the GUI browser automation isolated port |
| `mint status` | Show runtime status |
| `mint config init` | Create the local configuration file |
| `mint config path` | Print the configuration file path |
| `mint config show` | Print the current configuration |
| `mint config set <key> <value>` | Update a configuration value |
| `mint config doctor` | Validate the local setup |
| `mint providers` | List configured AI providers |
| `mint chat "<message>"` | Send one chat message |
| `mint imagine "<prompt>"` | Generate an image from a text prompt |
| `mint memory recent` | Show recent conversation memory |
| `mint task list` | List all tasks (pending and completed) |
| `mint task pending` | List pending tasks |
| `mint knowledge add <path>` | Index a local document |
| `mint knowledge search "<query>"` | Search indexed knowledge |
| `mint plugin list` | List local plugins |
| `mint mcp list` | List configured MCP servers |
| `mint learn <path>` | Import a persistent learned skill file |
| `mint update --check` | Check for an available update |

### Code Agent

Mint includes native workspace tools for code inspection, planning, editing, and execution:

```bash
mint code agent "inspect this repo and fix the failing tests"
mint code github-overview "Pheem49/Mint"
mint code summary .
mint code search "shell approval flow" .
mint symbols .
mint semantic-code index .
mint semantic-code search "provider fallback"
```

Inside interactive mode, use:

```text
/code <task>
```

Code-related fixes, workspace inspection, and test requests are routed into the code-agent loop automatically. Shell commands and file edits require explicit terminal approval before Mint applies them.

### Tools And Automation

```bash
mint files find README
mint safety path README.md
mint safety shell cargo test -p mint-core
mint run --approve -- cargo test -p mint-core
mint open README.md
mint open-app code
mint learn ./skill.md
```

### Ecosystem Plugins (`mint plugins`)

Centralized interactive management for built-in plugins (Spotify, Discord RPC, Gmail, Google Calendar, Notion, YouTube Music, Vercel, GitHub):

```bash
mint plugins
```
* **Interactive Terminal Checklist:** Toggle plugins on or off directly using terminal spacebar navigation.
* **Credential Prompts:** Automatically prompts for missing OAuth Client IDs, Client Secrets, or API Tokens.
* **PKCE OAuth & REST Polling:** Starts OAuth authorization flows and polls local REST endpoints (`http://localhost:3000/api/oauth/*`) for seamless Single Sign-On across CLI, Desktop UI, and Web UI.

### MCP Servers

Add a local MCP server and call one of its tools:

```bash
mint mcp add filesystem npx \
  --args -y \
  --args @modelcontextprotocol/server-filesystem \
  --args .

mint mcp list
mint mcp call filesystem list_directory \
  --arguments '{"path":"."}'
```

### Interactive Commands

| Command | Purpose |
| --- | --- |
| `/help` | Show interactive help |
| `/fast [on\|off]` | Toggle fast response mode |
| `/models [name]` | List or select a model |
| `/image-provider [name]` | List or select default image generation provider |
| `/clear` or `/reset` | Clear the active conversation |
| `/cd <path>` | Change workspace directory |
| `/image <path> [prompt]` | Send an image with an optional prompt |
| `/paste [prompt]` | Use an image from the clipboard |
| `/learn <path>` | Import a local skill |
| `/plugins [name]` | List or interact with available plugins/skills |
| `/memory list` | List stored memories |
| `/memory clear` | Clear stored memories |
| `/memory get <key>` | Read one memory value |
| `/memory set <key> <value>` | Store one memory value |
| `/mcp [subcmd]` | Manage configured MCP servers (list, allow, disallow) |
| `/stats` | Show session statistics |
| `/code <task>` | Start a code-agent task |
| `/exit` or `/quit` | Leave interactive mode |

## Running Mint 24/7 on a VPS (Headless Gateway)

By default, messaging bridges and cron only run while something's actually
attached — the interactive terminal, the desktop app, or `mint web`/`mint
api`. **Gateway mode** is a real headless mode built for unattended
deployment: no TUI, no desktop window, just the bridges and the cron
scheduler running in the background, installable as a systemd service that
survives reboots.

### How it works

- `mint gateway start` calls the exact same `start_channels()`/
  `start_cron_scheduler()` the interactive app uses — it just never launches
  the terminal UI, so it needs no TTY and can run under systemd with no
  login session attached.
- Every bridge loop auto-restarts on error *or* panic (5s backoff), so a bad
  payload from one platform can't silently and permanently kill that bridge.
- All bridges (Telegram, Discord, Slack, LINE, WhatsApp, Signal, Email) share
  one continuous memory thread with the terminal CLI, not a siloed one per
  platform.
- `GET /api/gateway/health` reports each bridge's enabled state, last
  success, last error, and consecutive-failure count as JSON — check it
  remotely instead of SSHing in to read `journalctl`.

### Quick start on a fresh VPS

```bash
# 1. Install Mint (Linux, needs Node/npm + Rust — the installer offers to set both up)
curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash

# 2. Configure a provider + the bridge(s) you want (Telegram, Signal, Email, ...)
mint onboard

# 3. Test in the foreground first — fix any config errors here before installing as a service
mint gateway start
# Ctrl+C once you see your bridge(s) come up "Active" and a test message gets a reply

# 4. Install as a systemd service and start it now
mint gateway install --now --api-port 3000 --memory-max 512M

# 5. Per-user units (the default) only run while you're logged in —
#    this keeps it running after you log out / reboot with no session at all
sudo loginctl enable-linger "$(whoami)"
```

### Gateway commands

| Command | Purpose |
| --- | --- |
| `mint gateway start` | Run bridges + cron in the foreground, headless (no TUI) |
| `mint gateway start --api-port <N>` | Same, plus the local API/WebUI on port `<N>` |
| `mint gateway install` | Write + enable a per-user systemd unit (`~/.config/systemd/user/`, no root) |
| `mint gateway install --system` | Same, but system-wide (`/etc/systemd/system/`, needs `sudo`) |
| `mint gateway install --now` | Also start the service immediately after installing it |
| `mint gateway install --memory-max <size>` | Cap the service's memory (systemd size syntax, e.g. `512M`, `1G`) — unset by default |

Once installed:

```bash
systemctl --user status mint.service      # or `systemctl status mint` with --system
journalctl --user -u mint.service -f      # follow logs
```

### Checking on it remotely

Don't expose the API/WebUI port to the public internet — reach it over an
SSH tunnel or [Tailscale](https://tailscale.com/) instead:

```bash
ssh -L 3000:localhost:3000 you@your-vps
curl http://localhost:3000/api/gateway/health
```

For an extra layer beyond the tunnel itself, set a shared secret so every API
request needs it:

```bash
mint config set apiAuthToken "$(openssl rand -hex 32)"
```

Once set, every request (except the browser's CORS preflight) needs
`Authorization: Bearer <token>` or gets `401 Unauthorized`. Leave it unset to
keep the previous open-on-localhost behavior (desktop app / `mint web` don't
need to change anything).

### New bridges built for this: Signal and Email

- **Signal** has no official bot API, so Mint talks to a self-hosted
  [`signal-cli-rest-api`](https://github.com/bbernhard/signal-cli-rest-api)
  instance instead (you link the number yourself first). Config:
  `enableSignalBridge`, `signalApiUrl`, `signalNumber`.
- **Email** reuses the same Gmail OAuth connection as the `gmail` plugin —
  set `gmailClientId`/`gmailClientSecret`, run `mint gmail auth` once to get
  a refresh token, then enable it. Both are offered directly in `mint
  onboard` under "Messaging Bridges".

> [!NOTE]
> LINE and WhatsApp are webhook-based (the provider pushes to you), which
> means they need a real public HTTPS URL — a reverse proxy (Caddy/nginx) +
> TLS cert in front of the VPS. Telegram, Discord, Slack, Signal, and Email
> all connect *outbound* instead, so they need nothing public at all. See
> [`docs/WEBHOOK_FORWARDING.md`](docs/WEBHOOK_FORWARDING.md) before exposing
> a webhook listener.

## Configuration

Mint stores its local configuration in the platform config directory:

| Platform | Typical path |
| --- | --- |
| Linux | `~/.config/mint/mint-config.json` |
| macOS | `~/Library/Application Support/mint/mint-config.json` |
| Windows | `%APPDATA%\mint\mint-config.json` |

Create and inspect the configuration:

```bash
npm run cli -- config init
npm run cli -- config path
npm run cli -- config show
npm run cli -- config doctor
```

Configuration covers provider credentials, model preferences, browser context,
voice and TTS, proactive suggestions, headless tasks, updates, workflows, MCP
servers, and optional integrations such as Calendar, Gmail, Notion, Telegram,
Discord, Slack, LINE, WhatsApp, Google Search, and Brave Search.

The optional browser smart-context helper can provide active-tab context from:

```text
http://127.0.0.1:3212/context
```

Chromium automation uses the local debugging endpoint:

```text
http://127.0.0.1:9222/json/list
```

## Webhook Integrations

LINE and WhatsApp webhook listeners bind to localhost by default. Read
[`docs/WEBHOOK_FORWARDING.md`](docs/WEBHOOK_FORWARDING.md) before exposing them
through a TLS tunnel.

## Safety And Privacy

Mint keeps high-risk behavior behind explicit policy checks:

- Shell commands are evaluated before execution, then run inside an OS-level
  sandbox by default (bubblewrap on Linux, Seatbelt on macOS —
  `sandboxMode`). Subagents can additionally be isolated in a per-session
  Docker container (`sandboxBackend: "docker"`).
- Code edits and update installation require approval.
- Sensitive directories such as `.ssh`, `.gnupg`, and Mint's own config
  directory are protected by default.
- Sensitive filenames such as `.env` and private key files are blocked from
  routine workspace access.
- LINE and WhatsApp webhook services listen locally unless you intentionally
  forward them.
- Every messaging bridge (Telegram, Discord, Slack, LINE, WhatsApp, Signal,
  Email) locks itself to a single owner: the first sender it ever hears from
  is claimed as that owner, and every other sender is silently ignored from
  then on. To hand a bridge to a different sender, clear its stored owner id
  (e.g. `mint config set telegramOwnerChatId ""`) before they message it.
- The local API server (`mint api`, `mint web`, `mint gateway start
  --api-port`) is open by default, matching the assumption that it's only
  reached from localhost or your own desktop/web frontend. If you expose the
  port on a VPS, set `apiAuthToken` (`mint config set apiAuthToken
  "<secret>"`) to require every request to carry `Authorization: Bearer
  <token>` — and still prefer an SSH tunnel or Tailscale over opening the
  port publicly regardless. See [Running Mint 24/7 on a VPS](#running-mint-247-on-a-vps-headless-gateway).

Review the generated command or edit preview before approving an action.

## Development

Useful validation commands:

```bash
npm run build:web
cargo test -p mint-core -p mint-cli -p mint-desktop
cargo check -p mint-desktop
npm run tauri:build -- --debug --no-bundle
```

### Project Layout

```text
crates/mint-core   Shared Rust domain logic
crates/mint-cli    Native Rust CLI
src-tauri          Tauri desktop backend and IPC commands
src/renderer       React and TypeScript webview UI
docs               Project documentation
out/renderer       Generated Vite renderer output
```

## Migration Status

Mint's historical Electron desktop runtime and Node CLI have been removed. The
active application is the native Tauri v2 and Rust implementation documented
above. See [`TAURI_MIGRATION.md`](TAURI_MIGRATION.md) for compatibility notes.

## Contributing

We welcome contributions from the community! Whether you want to fix a bug, add a new provider, or build a new integration, please check out our [CONTRIBUTING.md](CONTRIBUTING.md) guide for setup instructions, project architecture details, and our roadmap.

## License

Mint is licensed under the [AGPL-3.0-only license](LICENSE).

