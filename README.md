# Mint

<p align="center">
  <img src="assets/icon.png" alt="Mint Icon" width="160">
</p>

<p align="center">
  <strong>Desktop assistant + CLI coding agent built with Electron, Node.js, and modern LLM providers.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/badge/Node.js-LTS-green?style=for-the-badge&logo=node.js" alt="Node.js">
  <img src="https://img.shields.io/badge/Electron-40.x-47848F?style=for-the-badge&logo=electron" alt="Electron">
  <img src="https://img.shields.io/badge/CLI-Agentic-orange?style=for-the-badge" alt="CLI Agentic">
</p>

Mint is an AI assistant that runs in two main surfaces:

- A desktop Electron app for chat, screen-aware help, and system actions
- A terminal-first CLI with a workspace-aware coding agent

The project is built around practical local workflows: inspect code, edit files, run safe commands with approval, use project context, and fall back across multiple AI providers when needed.

## What Mint Can Do

- Chat in Desktop or CLI mode
- Route coding requests into Code Mode automatically from the CLI chat UI
- Inspect a workspace before editing
- Search code, read file ranges, and patch files
- Run non-destructive shell commands with user approval
- Keep lightweight per-workspace session memory
- Perform a second-pass reviewer step in Code Mode
- Execute structured actions such as opening apps, URLs, folders, and system tasks
- Support multiple providers: Gemini, Anthropic, OpenAI, local OpenAI-compatible endpoints, Ollama, and Hugging Face

## Current Agent Capabilities

Mint CLI is an agentic coding workflow, not just a chat wrapper.

In Code Mode it can:

- Decide on the next step from the current observation
- Use tools in a loop: `list_files`, `read_file`, `search_code`, `run_shell`, `apply_patch`, `write_file`
- Observe tool output and continue iterating
- Stop with a summary and verification result
- Ask for approval before shell commands and file changes

That makes Mint a practical CLI coding agent, while still keeping the user in control of risky actions.

## Screenshots

<p align="center">
  <img src="assets/Agent_Mint.png" alt="Mint Desktop UI" width="48%">
  <img src="assets/Settings.png" alt="Mint Settings" width="48%">
</p>

<p align="center">
  <img src="assets/CLI_Screen.png" alt="Mint CLI" width="100%">
</p>

## Installation

### Global install

```bash
npm install -g @pheem49/mint@latest
```

### Local development

```bash
git clone https://github.com/Pheem49/Mint.git
cd Mint
npm install
```

## Quick Start

### Run the desktop app

```bash
npm start
```

### Run the CLI

```bash
mint
```

### First-time setup

```bash
mint onboard
```

## CLI Commands

- `mint` or `mint chat`
Start the interactive chat UI.

- `mint code "<task>"`
Run the workspace-aware coding agent in the current directory.

- `mint task "<task>"`
Queue a background task for the headless agent.

- `mint agent`
Run the background headless agent.

- `mint list`
Show major features and commands.

- `mint onboard`
Configure API keys and local settings.

## CLI Examples

### Interactive chat

```bash
mint
```

Then ask naturally:

```text
สำรวจโปรเจคนี้ให้หน่อย
แก้บัคใน CLI ตัวนี้และรันเทสต์
open github
```

### One-shot coding task

```bash
mint code "fix the failing tests and verify the result"
```

### Background task

```bash
mint task "inspect the repo and summarize the top 3 risks"
```

## Code Mode Workflow

Mint Code Mode is designed around an inspect -> act -> verify loop.

What it does well right now:

- Understand the current workspace path
- Read git status and diff summary
- Suggest verification commands from `package.json`
- Apply focused patches instead of blind rewrites
- Keep a workspace session summary for future tasks

What it deliberately does not do:

- Run destructive commands like `rm -rf` or `git reset --hard`
- Edit files outside the current workspace
- Execute shell edits without approval

## Desktop Features

- Chat window with custom UI
- Settings window for provider and behavior configuration
- System tray support
- Proactive suggestion loop
- Silent screen capture for analysis
- Screen translation support
- Floating widget / overlay UI elements

## AI Providers

Mint supports multiple providers and local backends.

- `gemini`
- `anthropic`
- `openai`
- `local_openai`
- `ollama`
- `huggingface`

For CLI Code Mode, Mint currently behaves best with:

- `gemini`
- `anthropic`
- `openai`
- `local_openai`

## Project Structure

```text
Mint/
├── src/
│   ├── AI_Brain/          # Provider integration, prompts, memory, orchestration
│   ├── Automation_Layer/  # App/file/browser actions
│   ├── CLI/               # Chat router, code agent, TUI support, onboarding
│   ├── Plugins/           # Docker, Spotify, calendar, system monitor, MCP
│   ├── System/            # Config, notifications, daemon, automation, task queue
│   └── UI/                # Electron renderer files
├── tests/                 # Jest tests
├── mint-cli.js            # Main CLI entry
├── mint-cli-logic.js      # CLI action executor
├── main.js                # Electron main process
└── package.json
```

## Development

### Run tests

```bash
npm test -- --runInBand
```

### Watch tests

```bash
npm run test:watch
```

### Build Linux packages

```bash
npm run build:linux
```

See [BUILD_AND_RELEASE.md](/home/pheem49/vscode/Project/Mint/BUILD_AND_RELEASE.md) for Linux packaging and release notes workflow.

## Security Notes

- API keys are stored in local config, not in source files
- Code Mode asks for approval before shell commands and file edits
- Workspace path resolution blocks writes outside the active workspace
- Several shell-based execution paths have been hardened to use argument-based process execution

This is still an actively evolving project. Review permissions and local configuration before using Mint against sensitive files or production systems.

## License

Mint is licensed under the GNU Affero General Public License v3.0.
See [LICENSE](/home/pheem49/vscode/Project/Mint/LICENSE).
