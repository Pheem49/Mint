<div align="center">
  <img src="assets/icon.png" alt="Mint icon" width="112" />

  # Mint

  **An AI coding agent that runs on your machine — available from your terminal, desktop, browser, or messaging app.**

  [![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)
  [![Rust](https://img.shields.io/badge/Rust-backend-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
  [![React](https://img.shields.io/badge/React-TypeScript-149ECA?logo=react&logoColor=white)](https://react.dev/)
  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
</div>

Mint inspects your workspace, edits code, runs tests, and reports results.
Use it interactively or hand off a task from Telegram, Discord, Slack, LINE,
WhatsApp, Signal, or Email. Code edits and risky actions require approval.

Mint is built for developers who want to work in their own local workspace
with a choice of cloud providers or local models through Ollama.

<table width="100%">
  <tr>
    <th width="33%">Workspace</th>
    <th width="33%">Web</th>
    <th width="33%">CLI / TUI</th>
  </tr>
  <tr>
    <td><img src="assets/Mint_WorkSpace.png" alt="Mint workspace and coding agent" /></td>
    <td><img src="assets/Mint_Web.png" alt="Mint Web interface" /></td>
    <td><img src="assets/Mint_CLI.png" alt="Mint terminal interface" /></td>
  </tr>
</table>

[Release notes](Release_Note.md) · [Roadmap](ROADMAP.md) · [Discussions](https://github.com/Pheem49/Mint/discussions)

## Install

### CLI

**macOS and Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
powershell -Command "iwr -useb https://raw.githubusercontent.com/Pheem49/Mint/main/install.ps1 | iex"
```

The installer uses a verified prebuilt CLI when one is available for your
platform, otherwise it builds from source. Source builds need Node.js/npm,
Rust, and native build dependencies; the installer helps set these up.

### Desktop

Download the installer for your platform from [GitHub Releases](https://github.com/Pheem49/Mint/releases).
Prebuilt downloads are currently unsigned; see the
[first-run instructions](docs/INSTALLATION.md#first-run--unidentified-developer--unknown-publisher)
for macOS Gatekeeper and Windows SmartScreen.

For manual installation, Linux dependencies, and building from source, see
[the installation guide](docs/INSTALLATION.md).

## Quick start

After installation, choose your AI provider with `mint onboard` and configure
agent tools with `mint setup`. Then open Mint in your project directory:

```bash
mint onboard
mint setup

mint
```

In the full-screen TUI, type what you want Mint to do:

```text
Inspect this repo and fix the failing tests.
```

Use `/help` to explore commands. See the [usage guide](docs/USAGE.md) for more.

## What Mint can do

- **Code and verify:** inspect files, navigate symbols, edit code, run tests,
  and delegate focused work to subagents, with Git checkpoints and approval gates.
- **Work across interfaces:** use the full-screen terminal, native Desktop,
  Web UI, or messaging bridges, backed by the same Rust core.
- **Choose your models:** connect OpenAI, Anthropic, Gemini, Ollama,
  Hugging Face, and OpenAI-compatible endpoints.
- **Remember and reuse:** keep conversation memory, search local knowledge,
  and install or create reusable agent skills.
- **Connect and customize:** add MCP servers, ecosystem plugins, and Mint Mods
  for custom panels, commands, and tool guards.
- **Automate other tasks:** schedule work, control a browser, analyze screenshots,
  generate images, or connect optional video-editing and avatar integrations.

See [features and integrations](docs/FEATURES.md) for details.

## Choose an interface

**Terminal:** run `mint` for the full-screen TUI. Use `mint --classic` for
Classic CLI, or press F6 to switch within a conversation.

**Desktop:** launch the installed app. From a source checkout, run
`npm install` once, then `npm start`. Use `npm run dev` for development
with UI hot reload.

**Web:** run `mint web` to launch the browser UI and local API.
For private access from your phone, see
[mobile/PWA setup with Tailscale](docs/MOBILE_PWA_TAILSCALE.md).

**Mint Companion:** an optional separate Live2D app that follows a selected
Mint conversation. See [Companion setup and operation](docs/MINT_COMPANION.md).

More screenshots, shortcuts, and command examples are in the
[usage guide](docs/USAGE.md).

## Privacy and approvals

Mint runs locally and stores its configuration and conversation memory on
your machine. When you choose a cloud model, prompts and relevant task context
are sent to that provider; Ollama supports local inference.

Code edits and high-risk actions require approval. Shell execution uses
policy checks and an OS sandbox by default; sensitive paths and credentials
are protected from routine workspace access. Review previews before approving.

Messaging bridges claim their first sender as owner. The API does not require
a token by default: keep it private, and configure `apiAuthToken` before
exposing it beyond your machine. See
[configuration and safety](docs/CONFIGURATION.md) and
[remote gateway access](docs/GATEWAY.md#checking-on-it-remotely).

## Documentation

- [Installation](docs/INSTALLATION.md) — dependencies, source builds, and first-run help.
- [Usage and commands](docs/USAGE.md) — CLI, screenshots, skills, plugins, and MCP.
- [Configuration](docs/CONFIGURATION.md) — providers, local settings, safety, and privacy.
- [Gateway and VPS](docs/GATEWAY.md) — messaging bridges, cron, and systemd operation.
- [Mint Mods](docs/mods/README.md) — custom panels, commands, authoring, and testing.
- [Browser automation](docs/MINT_AUTO_BROWSER.md) — the dedicated automation browser.
- [Agent architecture](docs/AGENT_HARNESS.md) and [memory](docs/MEMORY_ARCHITECTURE.md) — technical details.
- [Development](docs/DEVELOPMENT.md) and [build/release](BUILD_AND_RELEASE.md) — source layout and validation.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and contribution
guidelines. Bring questions and ideas to
[GitHub Discussions](https://github.com/Pheem49/Mint/discussions).

## License

Mint is licensed under the [MIT license](LICENSE).
Third-party dependencies and bundled assets retain their respective licenses.
