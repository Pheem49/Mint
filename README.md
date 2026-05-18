# Mint

<p align="center">
  <img src="assets/icon.png" alt="Mint Icon" width="160">
</p>

<p align="center">
  <strong>Unified AI Desktop Assistant, Agentic CLI, and local-first automation workspace.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/badge/Node.js-LTS-green?style=for-the-badge&logo=node.js" alt="Node.js">
  <img src="https://img.shields.io/badge/Electron-40.x-47848F?style=for-the-badge&logo=electron" alt="Electron">
  <img src="https://img.shields.io/badge/CLI-Unified_Agent-orange?style=for-the-badge" alt="CLI Agentic">
  <a href="https://pheem49.github.io/Mint/guide.html"><img src="https://img.shields.io/badge/Documentation-View_Guide-00ffa3?style=for-the-badge" alt="Documentation"></a>
</p>

Mint is an AI assistant built to live in your desktop and terminal. It combines a transparent Electron desktop assistant, a unified agentic CLI, project-aware coding tools, local memory, automation, multi-provider AI routing, MCP extensions, and safety controls.

## What's New

- **Unified CLI Agent:** `mint` now routes every normal message through the same agent loop. It can think, answer conversationally, inspect projects, edit files, run tools, and finish directly for simple chat.
- **Fast Mode:** `/fast` switches the interactive CLI into a quieter `[Fast]` status that keeps the working indicator visible but hides internal `Thinking:` and tool-progress trace messages.
- **Live CLI Replies:** Mint responses now appear in one live-updating `Mint` message instead of waiting for the whole final answer to render at once.
- **Learned Skills:** `mint learn <path>` and `/learn <path>` import local `.md` or `.txt` files as persistent skill/instruction memory. Learned skills can be listed and deleted.
- **Provider Fallback:** The agent can fall back across supported providers, for example from local OpenAI-compatible backends to Gemini.
- **Provider Visibility:** Desktop and CLI responses show the provider/model that actually answered, including fallback results.
- **Google Workspace + Notion Integrations:** Gmail, Google Calendar, and Notion plugins can be configured from onboarding.
- **Safety Manager:** Central safety policy for shell commands and actions, including deterministic command blocking, permission tiers, path guards, and action logs.
- **Refactored Main Process:** Electron startup is split into focused modules for windows, IPC, proactive loop, screen capture, and action execution.
- **CI & Audit Baseline:** GitHub Actions runs install, tests, and security audit. Current local test baseline is `137` passing tests and `0` high vulnerabilities.
- **Dependency Hardening:** Removed vulnerable `google-tts-api` and `xlsx`; replaced with internal Google TTS URL generation and `read-excel-file`.

## Key Features

### Unified CLI Agent

Mint CLI is not just a chat wrapper. It is a workspace-aware agent loop.

- **Think Before Acting:** Every request goes through an agent decision step.
- **Fast Mode:** Toggle `/fast` to hide internal thought/progress messages while keeping the final answer, approvals, tools, and working indicator unchanged.
- **Live Answer Rendering:** Final answers are streamed into a single Mint message block as they arrive.
- **Conversational + Coding in One Flow:** Casual messages can finish directly; coding tasks can inspect, plan, edit, and verify.
- **Workspace Context:** Reads current path, git status, diff summary, package scripts, and previous workspace session memory.
- **Tool Use:** Supports web search, file listing, file reading, code search, path finding, shell commands, patch edits, file writes, opening folders, and asking the user.
- **Approval Flow:** Shell commands, patches, and full-file writes require user approval.
- **Provider Support:** Gemini, OpenAI, Anthropic, and local OpenAI-compatible endpoints for agent tasks.
- **Agent Collaboration Option:** Optional reviewer pass can be enabled for longer tasks.

### Desktop Assistant

- **Electron Desktop UI:** Transparent desktop assistant window with tray support.
- **Floating Widget:** Always-on-top quick access widget.
- **Spotlight Launcher:** `Alt+Space` quick prompt window.
- **Screen Vision:** Capture the screen and send selected regions to the AI.
- **Live Translation:** Continuously translate a selected screen area.
- **Proactive Suggestions:** Periodic screen/context analysis with behavior memory.
- **System Notifications:** Low battery, connection changes, and proactive notices.
- **Settings UI:** Configure provider, model, theme, keys, bridge options, MCP, and assistant behavior.

### Automation

- **Apps and Websites:** Open local apps, URLs, search queries, files, and folders.
- **Browser Automation:** Use Puppeteer-driven browser workflows.
- **File Operations:** Create folders, find paths, open files/folders, and move files to trash.
- **System Automation:** Volume, mute, brightness, suspend, restart, shutdown, and window minimization helpers.
- **Granular Automation:** Mouse move, mouse click, typing, and key tap actions.
- **Custom Workflows:** Process-monitoring rules loaded from local config.
- **Headless Agent:** Queue background tasks with `mint task`.

### Knowledge and Memory

- **Chat History:** Persistent local chat transcript.
- **Behavior Memory:** Stores recurring user context for proactive suggestions.
- **Long-Term Memory Store:** SQLite-backed user context, session memories, usage patterns, and response cache.
- **Learned Skill Files:** Import `.md` or `.txt` instruction files with `mint learn <path>` or `/learn <path>`. Mint remembers them as persistent skill/instruction context.
- **Knowledge Base / RAG:** Index and search local `.txt`, `.md`, `.pdf`, `.docx`, and `.xlsx` files.
- **Workspace Session Memory:** Remembers previous task summary and verification for each workspace.

### Multi-Provider AI

- **Gemini:** Main default provider with model selection.
- **OpenAI:** GPT-compatible cloud provider.
- **Anthropic:** Claude-compatible provider.
- **Local OpenAI Compatible:** LM Studio or other local `/v1/chat/completions` servers.
- **Ollama / Hugging Face:** Available in general provider configuration where supported.
- **Fallback Routing:** Agent provider selection can fall back when local providers are offline.
- **Response Badges:** Chat surfaces show the provider/model that produced the final response, such as `gemini • gemini-3.1-flash-lite-preview`.

### Messaging Bridges and Plugins

- **Discord Bridge**
- **Telegram Bridge**
- **Slack Bridge**
- **LINE Bridge**
- **WhatsApp Bridge**
- **Google Search and Brave Search Bridges**
- **Spotify Plugin**
- **Docker Plugin**
- **Obsidian Plugin**
- **System Monitor and Metrics Plugins**
- **Google Calendar Plugin:** List events and create calendar events via Google Calendar API, with browser fallback.
- **Gmail Plugin:** Search/read Gmail and create drafts safely. It does not send email automatically.
- **Notion Plugin:** Create notes/pages, read databases, and append page blocks through the Notion API.
- **MCP Manager**

### MCP Extensions

Mint supports the **Model Context Protocol (MCP)** so external tools can be added without hardcoding them into Mint.

```bash
mint mcp add <name> <command> --args <args...> --env <KEY=VALUE>
mint mcp list
mint mcp remove <name>
mint mcp clear
```

Example:

```bash
mint mcp add google-search npx --args -y @modelcontextprotocol/server-google-search --env GOOGLE_API_KEY=your_key GOOGLE_SEARCH_ENGINE_ID=your_id
```

## Safety System

Mint includes a central safety layer in `src/System/safety_manager.js`.

- **Permission Tiers:** `safe`, `approval`, `dangerous`, and `blocked`.
- **Deterministic Command Blocking:** Blocks known dangerous shell commands regardless of what the AI requests.
- **Blocked Examples:** `rm -rf`, `git reset --hard`, `git clean -f`, `mkfs`, raw disk writes, `shutdown`, `reboot`, `sudo`, `chmod -R 777`, `curl | sh`, and `wget | bash`.
- **Dangerous Actions:** `delete_file` and destructive `system_automation` actions require explicit permission.
- **Path Guard:** Prevents path traversal outside an allowed root.
- **Action Logs:** Writes JSONL records to `~/.config/mint/action-log.jsonl`.
- **Test Coverage:** Safety tests verify destructive command blocking, dangerous action classification, path traversal protection, and action executor enforcement.

## Screenshots

<p align="center">
  <img src="assets/Agent_Mint.png" alt="Mint Desktop UI" width="48%">
  <img src="assets/Settings.png" alt="Mint Settings" width="48%">
</p>

<p align="center">
  <img src="assets/CLI_Screen.png" alt="Mint CLI" width="100%">
</p>

## Installation

### Global Install

```bash
npm install -g @pheem49/mint@latest
```

### Local Development

```bash
git clone https://github.com/Pheem49/Mint.git
cd Mint
npm install
```

## Quick Start

```bash
mint onboard
mint
npm start
```

## CLI Commands

- `mint` / `mint chat` - Start the unified interactive agent UI.
- `mint chat "<message>"` - Start with an initial message.
- `mint chat --image ./screenshot.png "What is on this screen?"` - Attach an image to the initial chat message.
- `/image ./screenshot.png What is on this screen?` - Attach an image while inside the interactive CLI, then press Enter to send.
- `Ctrl+V` or `/paste What is on this screen?` - Attach clipboard images inside the interactive CLI, then press Enter to send.
- `mint learn ./skill.md` - Read a local `.md` or `.txt` file and remember it as a persistent Mint skill/instruction.
- `mint learn --list` - List learned skill files.
- `mint learn --delete <id|path|name>` - Delete a learned skill by ID, path, or file name.
- `mint code "<task>"` - Run a specific coding task in the current workspace.
- `mint code --image ./mockup.png "Build this UI"` - Attach an image as visual context for a coding task.
- `mint gmail auth` - Open Google OAuth and save a Gmail refresh token.
- `mint gmail auth --no-open` - Print the Gmail OAuth link without opening a browser.
- `mint task "<task>"` - Queue a background task for the headless agent.
- `mint agent [task]` - Run the background/headless agent.
- `mint mcp` - Manage MCP servers.
- `mint update` - Check npm and install the latest Mint CLI version.
- `mint update --check` - Check for a newer version without installing it.
- `mint list` - Display available features and commands.
- `mint onboard` - Configure Mint for first use.

## CLI Updates

Mint CLI checks for updates automatically on startup. The auto-check is enabled by default, uses a 24-hour cooldown, and updates from npm with `npm install -g @pheem49/mint@latest` when a newer package version is available.

Use manual update commands when you want direct control:

```bash
mint update
mint update --check
mint update --dry-run
```

You can skip the startup auto-check for one command:

```bash
MINT_SKIP_AUTO_UPDATE=1 mint
```

To disable automatic update checks, set `enableAutoUpdate` to `false` in your Mint config file.

## Integration Setup

Most integrations can be configured from:

```bash
mint onboard
```

### Gmail

Gmail uses Google OAuth, not a plain Gmail address/password. Configure the OAuth Client ID and Client Secret in onboarding, leave the refresh token empty if you do not have one yet, and keep `Gmail User ID` as `me` for the signed-in account.

After onboarding, run one of:

```bash
mint gmail auth
mint gmail auth --no-open
```

`mint gmail auth` opens the browser automatically. `mint gmail auth --no-open` prints the auth link for you to open manually. Both flows save `gmailRefreshToken` locally after Google redirects back to Mint. Recommended scopes are `gmail.readonly` and `gmail.compose`; Mint creates drafts only and does not send email automatically.

### Google Calendar

Google Calendar uses OAuth credentials and a refresh token. Onboarding stores:

- `googleCalendarClientId`
- `googleCalendarClientSecret`
- `googleCalendarRefreshToken`
- `googleCalendarId`, usually `primary`

The plugin can list events and create events through the Calendar API. If OAuth is not configured, it falls back to opening Google Calendar in the browser.

### Notion

Notion uses an internal integration secret. After creating an integration in Notion, share the target page or database with that integration, then configure:

- `notionApiKey`
- `notionDatabaseId`, optional default database
- `notionPageId`, optional default page
- `notionTitleProperty`, default `Name`

The plugin can create pages, query database pages, and append text blocks.

## Interactive Slash Commands

Inside `mint`:

- `/help` - Show commands.
- `/fast [on|off|status]` - Toggle Fast Mode. Fast Mode shows `[Fast]`, keeps `Mint is thinking...`, and hides `Thinking:`/progress trace messages.
- `/learn <path>` - Read a local `.md` or `.txt` file and remember it as a persistent Mint skill/instruction.
- `/memory skills` - Show learned skill files.
- `/memory skills delete <id|path|name>` - Delete a learned skill.
- `/image <path> [prompt]` - Attach an image from disk.
- `/paste [prompt]` - Attach an image from the clipboard.
- `/code <task>` - Force Code Mode.
- `/cd <path>` - Change active workspace directory.
- `/models [name]` - Show or switch model/provider.
- `/memory [cmd]` - Manage long-term memory.
- `/config` - Show current configuration.
- `/copy` - Copy last response.
- `/clear` / `/reset` - Clear conversation history.
- `/agent <type>` - Switch specialized persona.
- `/workspace` - Manage registered workspaces.
- `/stats` - Show system statistics.
- `/review` - Ask reviewer persona to critique the last answer.
- `/exit` - Exit.

## Development

```bash
npm test
npm test -- --runInBand
npm audit --audit-level=high
npm start
npm run build:linux
```

## Project Structure

```text
Mint/
├── main.js                         # Electron bootstrap and wiring
├── mint-cli.js                     # CLI entry point
├── mint-cli-logic.js               # CLI action executor bridge
├── src/
│   ├── AI_Brain/                   # Providers, memory, RAG, autonomous/headless agents
│   ├── Automation_Layer/           # File, app, website, and browser automation
│   ├── Channels/                   # Messaging and search bridges
│   ├── CLI/                        # Unified CLI UI, router, code agent, workspaces
│   ├── Command_Parser/             # Structured AI response parser
│   ├── Plugins/                    # Plugin manager and integrations
│   ├── System/                     # Config, IPC, safety, windows, screen capture, notifications
│   └── UI/                         # Electron renderer, settings, widgets, spotlight
├── tests/                          # Jest tests
├── docs/                           # Documentation site
└── package.json
```

## Runtime Notes

- Mint is currently a **Node.js + CommonJS** project, not TypeScript.
- API keys are stored locally in Mint config or environment variables.
- Google OAuth refresh tokens for Gmail and Calendar are stored locally in Mint config.
- Local OpenAI-compatible providers require a running local server such as LM Studio.
- Some desktop features depend on Linux tools such as `xdg-open`, `gio`, `xdotool`, `amixer`, `pactl`, `brightnessctl`, or `xbacklight`.
- Electron GUI behavior should be smoke-tested manually after large UI or main-process changes.

## Security & Privacy

- **Local Control:** Mint prioritizes local execution and local configuration.
- **User Approval:** Shell commands, patches, and file writes require explicit approval in the CLI agent.
- **Safety Manager:** Dangerous commands and actions are blocked or gated by deterministic policy.
- **Action Audit Trail:** Tool actions are logged locally for debugging and accountability.
- **Secure Config Practice:** Keys stay on the user's machine and are only sent to the selected AI/search provider.

## License

Mint is licensed under the **GNU Affero General Public License v3.0**.
See the [LICENSE](LICENSE) file for details.

---

<p align="center">Made with love by <a href="https://github.com/Pheem49">Pheem49</a></p>
