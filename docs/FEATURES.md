# Features and integrations

[← Back to Mint](../README.md)

## <img src="../assets/features.svg" width="24" height="24" valign="middle" /> What Mint Can Do

Mint is a local-first AI assistant running on your machine, capable of handling tasks from a messaging app, the desktop application, or the terminal interface (CLI):

---

### 1. <img src="../assets/bridges.svg" width="18" height="18" valign="middle" /> Reach Mint From Anywhere — Messaging Bridges
- Message it like a person from **Telegram, Discord (Gateway + RPC), Slack, LINE, WhatsApp, Signal, and Email (Gmail)** — no desktop window required. Each bridge locks to whoever messages it first, and all of them share one continuous memory/conversation with the terminal CLI.
- Runs unattended 24/7 on a VPS via `mint gateway start`/`install` — a systemd service with a `GET /api/gateway/health` endpoint. See [Running Mint 24/7 on a VPS](GATEWAY.md#running-mint-247-on-a-vps-headless-gateway).

---

### 2. <img src="../assets/live2d.svg" width="18" height="18" valign="middle" /> Mint Companion — Optional Live2D App
- **Shiroko** lives in a separate floating desktop app, with gaze tracking, expressions, accessories, and interaction areas.
- Connect to a running Mint CLI, Desktop, or Web backend on the same machine. Select the instance and conversation to follow thinking, tool use, queued chat, completion, and failures.
- Chat shares the selected conversation's memory and queue. Tools and approvals stay in Mint. Companion waits offline when Mint is closed and never starts another backend.
- Development: `npm run dev:companion`. Build: `npm run build:companion`. Package: `npm run package:companion -- --bundles deb` (Linux), `nsis` (Windows), or `dmg` (macOS).
- See [Companion architecture and operation](MINT_COMPANION.md).

---

### 3. <img src="../assets/chat.svg" width="18" height="18" valign="middle" /> AI Chat & Multi-Providers
- Connect to **Gemini, OpenAI, Anthropic (Claude), Ollama (Local), Hugging Face**, and OpenAI-compatible custom endpoints — system instructions, temperature control, voice replies, and multimodal image analysis.

---

### 4. <img src="../assets/code.svg" width="18" height="18" valign="middle" /> Autonomous Code Agent & Subagents
- Run code-agent loops via `/code <task>`, `mint code agent "<task>"`, or direct one-shot prompt `mint "<task>"`: scan the workspace, plan multi-file changes, edit, run tests/shell commands, and verify before finishing.
- **Harness Engineering & Git Safety:** Automatic Git checkpoints before task execution, task-isolated branches, and one-click rollback if verification fails.
- **Code Intelligence:** AST symbol navigation (`find_definition`, `find_references`) across Rust, TypeScript, JavaScript, Python, and Go without external LSPs.
- **Safe Automated Verification:** Run tests, typechecks, and linters (`run_tests`, `run_typecheck`, `run_linter`) with zero-prompt pre-approval policies.
- **Observability & Checklist:** Real-time plan progress checklists and run telemetry summary dashboards across CLI, Desktop, and Web.
- Delegate focused sub-tasks to specialized subagents (`dispatch_subagent`), optionally isolated in a per-session Docker container (`sandboxBackend: "docker"`).
> [!IMPORTANT]
> **Safety First:** Risky actions and file writes require your explicit approval first.

---

### 5. <img src="../assets/memory.svg" width="18" height="18" valign="middle" /> Memory, Knowledge & Skills
- Persistent conversation memory (SQLite), a searchable local knowledge base, and semantic code search.
- After solving a hard, reusable problem, the agent can write its own skill (`.agents/skills/`) — and genuinely refine an existing one instead of duplicating it, the next time a similar task recurs.
- Install ready-made skills too, from a local file/folder or straight from a GitHub repo/URL — `mint skills add <source>` (or `/skill add <source>` in chat). See [Skills](USAGE.md#skills-mint-skills) below.

---

### 6. <img src="../assets/tools.svg" width="18" height="18" valign="middle" /> Scheduled Tasks & Linked Folders
- `mint cron` runs agent tasks on a schedule with no OS-level daemon — rides along on whatever's already open, or `mint gateway start` for always-on.
- Link a folder (e.g. "Food") and chat that touches its topic gets a short, cross-referenced note written into it automatically.
- Mint indexes supported files in linked folders and their subfolders to decide which conversations contain useful notes. Use `/link refresh <name>` after changing files, `/link notes <name>` to inspect saved notes, or `/link save <name> | <text>` to save text directly. Notes are stored in `<folder>/mint-notes/`.

---

### 7. <img src="../assets/tools.svg" width="18" height="18" valign="middle" /> Tool & MCP Integrations
- **Model Context Protocol (MCP)** servers supporting both **Local Command (`stdio`)** and **Remote Server (`sse` / `http`)** transports with Bearer Token and Custom Header authentication, plus local plugins for Spotify, Google Calendar, Gmail, and Notion — manage all of it interactively with `mint plugins` or `/mcp`.
- Dedicated **Image Search** tool and an **Auto GitHub Link Resolver** that injects a linked repo's metadata/README as context automatically.

---

### 8. <img src="../assets/screencapture.svg" width="18" height="18" valign="middle" /> Screen Capture & Translation
- Capture screen snapshots for instant visual analysis, or run real-time continuous overlay translation of a screen region.

---

### 9. <img src="../assets/imagegen.svg" width="18" height="18" valign="middle" /> AI Image Generation
- Generate images from chat or terminal using **DALL-E 3, Stability AI, Ideogram, Replicate (Flux)**, and Google NanoBanana — aspect ratio, negative prompts, and automatic local storage.

---

### 10. <img src="../assets/tools.svg" width="18" height="18" valign="middle" /> Browser Automation (`mint auto`)
- Drives a dedicated, isolated Chromium instance (port `9222`): open URLs, click, type, and extract page content — the agent registers these tools automatically once it detects the automation browser is running.

---

### 11. <img src="../assets/tools.svg" width="18" height="18" valign="middle" /> AI Video Editing via FableMint
- Connect **[FableMint](https://github.com/Pheem49/FableMint)** — a free, open-source browser video editor — as an MCP server, and Mint can cut, grade, caption, chroma-key, and export edits from plain chat, with the open editor tab live-reloading as it works:
  ```bash
  mint mcp add fablemint node --args "<path-to>/FableMint/mcp-server.js"
  mint mcp allow fablemint "*"
  ```
  See [FableMint's README](https://github.com/Pheem49/FableMint#driving-it-with-an-ai-agent) for the full tool list.

---

### 12. <img src="../assets/tools.svg" width="18" height="18" valign="middle" /> Real-Time 3D Avatar via Project Avatar
- Connect **[Project Avatar](https://github.com/projectavatar/projectavatar)** — a free, open-source 3D VRM avatar that reacts live to what the agent is doing — with `/avatar` in chat (or `mint avatar` from the CLI):
  ```
  /avatar          # pick Web or Desktop, get your share link / token
  /avatar status   # check connection, selected model, viewer count
  /avatar off      # disable
  ```
- The agent reacts as it works — typing/searching while a tool runs, celebrating on a finished image/video — and can call `avatar_signal` directly for anything a tool call can't express, like a greeting, a joke, or an apology.
- **Web viewing just works** — picking Web opens `app.projectavatar.io/?token=...`, a page the Project Avatar team already hosts for free. No install, no clone, nothing to build; any browser works.
- **Desktop viewing** needs Project Avatar's own desktop app, built separately from [their repo](https://github.com/projectavatar/projectavatar#desktop-app) — it isn't bundled with Mint. Its first-run screen generates its own token; paste the one `/avatar` gave you into its "Paste existing token" field instead so it joins Mint's channel rather than its own.
