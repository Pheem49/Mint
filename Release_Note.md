# Release Notes - Mint Agent v1.10.0

We are excited to release **Mint Agent v1.10.0**! This version introduces major enhancements across CLI, Desktop, Web, Multimodal Video, Remote Messaging Bridges, Browser Automation, LaTeX Sanitization, and **AI Image-to-Image & Inpainting Editing Capabilities**.

---

## 📋 Message Action Bar & UI Clean Up (Web & Desktop UI)

- **Message Copy Button & Action Bar**:
  - Added a Copy Button to AI responses in both Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`).
  - Interactive status feedback: copies message text to clipboard with instant visual checkmark (✓) and copied tooltip state.
- **Mint Web & API Server Terminal Logging & Diagnostics**:
  - Implemented real-time filtered terminal logging for both `mint web` and `mint api` modes in `mint-cli` and `mint-core` (`crates/mint-core/src/api_server.rs`).
  - Terminal now displays real-time colorized API Requests (e.g. `[14:30:15] [API] POST /api/chat -> 200 OK`, `/api/status`, `/api/image-generate`, `/api/video-generate`) and Error Logs (`[14:30:15] [ERROR] API /api/video-generate error`) while keeping verbose frontend asset logs silent.
- **Google Veo 2 Video Generation Fix & Diagnostics**:
  - Updated Google Veo default model endpoint in `mint-core` (`crates/mint-core/src/video_gen.rs`) from deprecated preview string `veo-2.0-flash-exp` to official model identifier `veo-2.0-generate-001`.
  - Added automatic model fallback to `veo-2.0-flash-001` and descriptive error guidance (`ModelNotFound`) explaining Google Cloud / AI Studio API Key whitelisting requirements if 404 is returned.

---

## 🎨 Image-to-Image & Inpainting (Image Editing) Support

Mint Agent now supports full AI Image Editing (Image-to-Image and Inpainting)! Users can provide an existing image, specify modification instructions (such as *"remove background objects"*, *"change coat color to red"*), and generate edited images directly.

### Features Added & Enhancements
- **Multi-Provider Image Editing**:
  - **Replicate**: Instruction-based editing via `timbrooks/instruct-pix2pix` and mask inpainting via `black-forest-labs/flux-fill-dev`.
  - **Stability AI**: Integrated Stability Image Edit / Inpaint REST API endpoints (`/v2beta/stable-image/edit/inpaint`).
  - **DALL·E**: Integrated OpenAI `/v1/images/edits` endpoint when image input is provided.
  - **Provider Validation**: Automatically returns clear guidance if text-only image models (like Gemini NanoBanana) are selected for image editing.
- **New `/edit-image` Slash Command & Redesigned Image Studio UI**:
  - Usage in CLI: `/edit-image <image_path> <instruction>`
  - Image Studio UI (Web & Desktop): Redesigned image upload area into a premium **Drag & Drop Zone** with custom upload styling, SVG icons, hover animations, thumbnail previews, and instant file removal.
  - Parity: Implemented across both Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`).
- **`AGENTS.md` Workspace Rules & Skills Autoloading**:
  - Automatically scans and loads workspace rule files (`.agents/AGENTS.md`, `AGENTS.md`, `~/.gemini/config/AGENTS.md`) into system instruction prompt contexts, REST API endpoint (`GET /api/learned-skills`), Tauri Desktop IPC (`list_learned_skills`), and UI autocompletes.
  - Full Parity: Verified and supported across CLI (`mint`), Desktop UI (`src/renderer/src`), and Web UI (`src/renderer/src-web`).
- **File Approval Target Path, Line Numbers & Diff Stats (`+additions -deletions`)**:
  - Enhanced file modification and creation approval cards to display exact target file paths, line numbers on the left margin (parsed from Unified Diff hunk headers), and line addition/deletion counts (e.g. `path/to/file.rs (+15 -3)`) matching Codex-style diff previews.
  - Integrated `similar` (Myers Diff Algorithm) into `mint-core`'s `full_file_diff` generator to produce clean line-by-line diff previews with 3 context lines, eliminating whole-file replacement previews.
  - Full Parity: Supported across CLI (`crates/mint-cli`), Desktop UI (`src/renderer/src`), and Web UI (`src/renderer/src-web`).

---

## ⚡ Functional Slash Commands & `@` Context Autocomplete in Web & Desktop UI

This update brings full CLI slash command execution capabilities and `@` context mention autocomplete to both Web and Desktop interfaces.

### Features Added & Enhancements

- **Direct Keyboard Triggers (`/` and `@`)**: Type `/` to open the slash commands menu or `@` to trigger workspace context mentions directly inside the chat textarea.
- **Fluid Micro-Animations & Elastic Physics**: Added cubic-bezier spring pop-in animations (`cubic-bezier(0.16, 1, 0.3, 1)`), smooth hover translations (`translateX(4px)`), rotation transitions (`transform: rotate(45deg)`), and active button click scaling for all chat tools, popups, and autocomplete items.
- **Persistence Fix for System Response Cards**: Updated `save_system_interaction` in Rust (`src-tauri` & `mint-core`) and frontend IPC APIs (`tauri.ts`) to persist `aiText` output into the SQLite database. Previously, reopening the app restored commands like `/stats` or `/help` with an empty response card because `ai_text` was saved as an empty string. Now system outputs are fully preserved across app restarts.
- **CLI Parity Command Handlers**:
  - `/help`: Displays formatted interactive command table in chat.
  - `/fast [on|off]`: Toggles fast mode and thinking trace visibility.
  - `/code <task>`: Enables code-agent mode and submits `<task>`.
  - `/cd [path]`: Changes active workspace directory or opens folder picker.
  - `/models [name]`: Lists active providers & models or switches active model.
  - `/stats`: Displays rich session statistics system card (workspace, interactions, provider, model).
  - `/multi-agent [on|off]`: Toggles Multi-Agent Collaboration system.
  - `/image` & `/paste`: Triggers file dialog or clipboard image attachment.
  - `/veo <prompt>`: Routes to Veo video generation studio.
- **`@` Context Mention Autocomplete**: Added popup suggestion menu for `@workspace`, `@file`, `@docs`, and `@memory` mentions when typing `@` or clicking the `@` toolbar button.

---


## 🛠️ Deduplicated Settings System Events

This update fixes an issue where saving settings without changing AI providers or models logged redundant `provider_change` system event badges in the chat timeline.

### Bug Fixes & Enhancements

- **Backend Deduplication (`set_active_model`)**: Updated `set_active_model` in `config.rs` to compare previous provider and active model against new values, ensuring system interaction events are only written to the database when provider or model actually changes.
- **Frontend Deduplication (`SettingsWindow.tsx` & `MintDashboard.tsx`)**: Updated settings save handlers and provider/model switch handlers in both Desktop and Web renderers to verify whether provider or model changed before invoking `setActiveModel` or `saveSystemInteraction`.

---

## 🖥️ Minimalist CLI Security Approval UI Redesign

This release overhauls the interactive CLI approval prompt (`AgentApproval`) for MCP tools, local shell commands, file edits, note creation, and plugin executions into a modern, emoji-free **Minimalist Divider Badge** card layout.

### Features Added & Enhancements

- **Minimalist Divider Badge Cards**: Formatted security checks into clean 2-column key-value tables (`Server │ gmail`, `Tool │ list_tools`, `Command │ git status`) bounded by sleek ANSI divider lines.
- **Emoji-Free Modern Terminal Style**: Removed distracting emoji icons for a crisp, high-contrast developer terminal aesthetic (`BRIGHT`, `BLUE`, `DIM`, `MINT`).
- **Descriptive Action Options**: Updated the interactive selection menu with explicit choice descriptions:
  - `1. Approve (Once)` - Allow single execution
  - `2. Approve (Entire Session)` - Auto-approve throughout session
  - `3. Deny` - Cancel action

---

## 🇹🇭 Thai Language Typography & Chat Panel Spacing Enhancement

This update addresses cramped text rendering, missing paragraph line breaks, unformatted ordered/bullet lists, and unclickable markdown links in the chat interface.

### Features Added & Enhancements

- **Thai Typography & Line Height**: Increased line-height from tight defaults to `1.7` and added font fallbacks (`Prompt`, `Sarabun`, `Noto Sans Thai`, `Kanit`) for clear rendering of Thai upper/lower diacritics and vowels.
- **Structured Block Rendering**: Refactored `markdown.tsx` to group text into clean block paragraphs (`.chat-paragraph`), section titles (`.chat-section-title`), and structured list items with distinct margins.
- **Ordered & Bullet List Formatting**: Lines starting with `1.`, `2.`, `(1)`, `-`, `*`, or `•` are parsed into dedicated flex list elements (`.chat-list-item`) with custom centered vector CSS circle dot markers for pixel-perfect vertical alignment across all Thai/English fonts.
- **Interactive Markdown & Raw URL Links**: Added inline link parsing for `[label](url)` and raw `https://...` URLs, converting them into clickable external links (`.chat-link`).
- **UI Font Family Selector**: Expanded the Font Family dropdown in **Settings > Theme & UI** with Google Fonts support for `Prompt`, `Sarabun`, `Kanit`, `Mitr`, `Noto Sans Thai`, `Mali`, `Inter`, and `Fira Code`.

---

## 🔣 LaTeX Symbol & `ightarrow` Sanitization

This update enhances markdown and CLI sanitization to automatically convert unescaped LaTeX symbols into Unicode symbols even when printed without LaTeX delimiters `$`.

### Features Added & Enhancements

- **Bare LaTeX Symbol Conversion**: Updated `sanitizeLatex` in `markdown.tsx` and `sanitize_latex` in `agent.rs` to convert unescaped symbols like `ightarrow`, `\rightarrow`, `\leftarrow`, `\Rightarrow`, etc. directly into Unicode arrows (`→`, `←`, `⇒`).
- **Standalone `ightarrow` Cleanup**: Fixes garbled outputs from LLMs where string escape sequences swallow `\r` from `\rightarrow`, automatically replacing plain text `ightarrow` with `→`.
- **Frontend & CLI Parity**: Ensures both UI message rendering (`markdown.tsx`) and CLI terminal outputs (`agent.rs`) sanitize LaTeX math symbols consistently.

---

## 🧩 New Agent Skills Installation

Installed new external agent skills to enhance the workspace's capability and discipline.

### Skills Added
- **Gridgeist**: Installed the `gridgeist` skill from `ohmiler/gridgeist` repository into `.agents/skills/gridgeist`, including agents, assets, and design references.

---

## 🔄 Unified AI Model Switch & Real-time Synchronization

This update standardizes model switching across **CLI**, **Desktop (Tauri)**, and **Web** into a single unified implementation, eliminating out-of-sync status badges, inconsistent format strings, and outdated model indicators.

### Features Added & Enhancements

| Feature | Description |
|---|---|
| **Single Source of Truth (`set_active_model`)** | Added `set_active_model(provider, model)` to `MintConfig` in `mint-core` as the unified function for updating active AI providers and models. |
| **Unified Event Log Format** | Standardized the system event interaction message format across CLI, Desktop, and Web: `<provider> • <model>` (e.g. `gemini • gemini-2.5-flash`). |
| **MCP & Plugins Custom SVG Icons** | Replaced emoji icons with high-resolution vector SVGs for Docker, Git, GitHub, Node.js, Spotify, Discord, and Servers. Added custom SVG code, Image URL, and Preset icon support for Custom MCP Servers in Settings UI. |
| **MCP Server Inline Edit Drawer (✏️)** | Added inline expandable configuration panel directly inside each MCP Server card to easily modify Command, Arguments, SVG Icon, and Environment Variables in place. |
| **Real-time Event Broadcast (`settings-changed`)** | Emits real-time setting change events to Tauri listeners and HTTP clients whenever the model is changed from CLI, Settings UI, or API endpoints. |
| **Window Focus & Post-Send Sync** | Frontend UI automatically refreshes runtime status on window focus (`window.addEventListener('focus', ...)`) and post-message completion so status badges update instantly without manual app restart. |
| **General Tab Collapsible Accordion** | Added collapsible section headers (AI Routing, Search, Productivity, Image Gen, Video Gen, Custom Providers, Desktop Updates) with "Expand All" & "Collapse All" quick controls for a clean and readable Settings experience. |

---

## 🌐 Remote Mint Agent — Messaging Bridges & Remote Workspace Control

This update enhances **Telegram**, **Discord**, **Slack**, **LINE**, and **WhatsApp Cloud** bridges to enable full remote control of the Mint Agent, active workspace path resolution, agent loop execution, session isolation, and configurable webhook endpoints.

### Features Added & Enhancements

| Feature | Description |
|---|---|
| **Active Workspace Path Resolution** | Automatically resolves the active Desktop workspace path when requests arrive from Telegram, Discord, Slack, LINE, or WhatsApp (`workspace_path`), allowing the AI to inspect and read local files relative to the project directory. |
| **Session Isolation (`chat_id`)** | Assigns deterministic platform chat IDs (`telegram:<id>`, `discord:<channel_id>`, `slack:<channel_id>`, `line:<user_id>`, `whatsapp:<phone>`) so conversations and memory history remain isolated per channel/user instead of merging into default chat. |
| **Agent Loop Remote Intent Execution** | Automatically detects action/execution intents (e.g. *"แก้โค้ด"*, *"ดูไฟล์"*, *"รัน test"*, *"fix"*, *"build"*) from channel messages and invokes `orchestrate_agent_loop` to perform multi-step file reads and tool executions on the local machine. |
| **Instant Remote Ack Notification** | Immediately sends typing indicators and acknowledgement messages (`[Mint Agent] Remote command received, processing...`) when remote messages arrive via Telegram, Discord, LINE, or WhatsApp so users get instant feedback. |
| **Configurable Webhook Host & Port** | Added `lineWebhookHost`, `lineWebhookPort`, `whatsappWebhookHost`, `whatsappWebhookPort` to `MintConfig` and updated Tauri webhook handlers to allow binding to custom hosts (e.g. `0.0.0.0`) for public tunnel exposure. |
| **Automation Settings UI Redesign** | Redesigned the Native Channel Bridges UI in Settings > Automation into premium modern cards with platform color badges (Telegram ✈️, Discord 💬, Slack 💼, LINE 🟢, WhatsApp 📞), active status indicators (`Active` vs `Disabled`), toggle switches, styled inputs, and global Instant Ack notification options. |

### Files Changed

- `crates/mint-core/src/config.rs` — Added `active_workspace_path()`, `bridge_ack_enabled()`, `bridge_ack_message()`, and webhook host/port helpers to `MintConfig`.
- `crates/mint-core/src/channels.rs` — Added `answer_channel` with workspace path resolution, action intent classification, agent loop execution, instant typing indicators, and platform session `chat_id` forwarding.
- `src-tauri/src/webhooks.rs` — Updated LINE and WhatsApp handlers to send instant acknowledgement notifications, use configurable host/port, extract user/phone IDs for session isolation, and delegate to `answer_channel`.
- `src-tauri/src/lib.rs` — Updated `send_chat_message` and `stream_chat_message` to automatically persist `activeWorkspacePath` into `MintConfig` when a workspace is opened or used.
- `src/renderer/src/components/Settings/AutomationTab.tsx` & `src/renderer/src-web/components/Settings/AutomationTab.tsx` — Added UI input fields for LINE & WhatsApp Webhook Host and Port settings.

---

## 📹 Multimodal Video Attachments & Web Veo Studio Integration

This update introduces full support for sending video attachments, saving sent videos to local storage, filtering photos vs. videos in the Gallery, and enabling Veo Studio in the web build.

### Features Added

| Feature | Description |
|---|---|
| **Multimodal Video Support** | Attached videos (`.mp4`, `.webm`, `.mov`, `.mkv`) are sent as base64 `inlineData` payloads to Google Gemini API. Supported in Desktop/Web compose panels. |
| **Save Attached Videos to Disk** | Videos sent through the chat interface are automatically saved to `<config_path>/../Pictures/` with timestamped names, bypassing standard image thumbnail generation, and indexed in `index.json`. |
| **Photo/Video Gallery Tabs** | Added **Photos** 📷 and **Videos** 📹 tabs to the Saved Pictures gallery (both Desktop and Web builds). Video files render using native HTML5 `<video>` tags with controls. |
| **Web Veo Studio Integration** | Ported and enabled the **Veo Studio** video generation panel in the web build (`src-web`). Added a sidebar navigation entry and integrated the React workspace. |
| **Video Placeholder & Agent Mode** | Displays `[Video #1]` visual indicator in the user's chat bubble during transmission. Enabled full video support in `Agent Mode` loops and Tauri API server. |

### Files Changed

- `crates/mint-core/src/chat.rs` — Added `video_data_uri` to `ChatRequest` and updated `gemini_parts` payload builder
- `crates/mint-core/src/api_server.rs` — Exposed `video_data_uri` on HTTP routes, updated `orchestrate_agent_loop` calls, added saving of sent video attachments in `/api/chat` and `/api/chat-stream` endpoints, and added a `/api/thumbnails` route to serve generated video/image thumbnails on Web.
- `crates/mint-core/src/pictures.rs` — Added video MIME parsing, thumbnail extraction, and fixed temporary video/frame file leakage under `/tmp`
- `crates/mint-core/src/orchestration.rs` — Added `video_data_uri` parameter to `orchestrate_agent_loop` and forwarded it to fallback chat requests
- `crates/mint-cli/src/main.rs` — Connected the Veo command to the real video generation backend.
- `crates/mint-cli/src/image.rs` — Created `load_video_as_data_uri` helper
- `crates/mint-cli/src/interactive.rs` — Handled saving pending videos, and connected the `/veo` interactive command to the real video generation backend.
- `crates/mint-cli/src/agent.rs` — Updated `orchestrate_agent_loop` call
- `src-tauri/src/lib.rs` — Updated `orchestrate_agent_loop` calls with `video_data_uri`, fixed a compilation error (mismatched types E0308) in `upload_file` command, and registered `upload_file` in `generate_handler!`.
- `src/renderer/shared/platform.ts` — Updated `sendChatMessage` / `streamChatMessage` signatures
- `src/renderer/src/tauri.ts` & `src/renderer/src-web/tauri.ts` — Implemented video parameter forwarding and save commands
- `src/renderer/src/components/PicturesLibrary.tsx` & `src/renderer/src-web/components/PicturesLibrary.tsx` — Added Photo/Video filter tabs, replaced playable HTML5 `<video>` tags with static thumbnail cards featuring a video badge, and set a fixed `4:3` aspect ratio on containers to prevent placeholder collapsing and ensure uniform sizing.
- `src/renderer/src/components/MintDashboard.tsx` & `src/renderer/src-web/components/MintDashboard.tsx` — Wired video attachments, integrated VeoStudioPanel, and added `sendingVideoCount` state
- `src/renderer/src/components/ChatPanel.tsx` & `src/renderer/src-web/components/ChatPanel.tsx` — Added video picker, previews, and `[Video #1]` placeholder rendering during sending
- `src/renderer/src-web/components/VeoStudioPanel.tsx` — **[NEW]** Ported video generator workspace for web renderer

---

## 🎬 Veo Studio — AI Video Generation Panel

This release introduces **Veo Studio**, a new dedicated panel for AI-powered video generation, accessible via the "More" menu in the sidebar.

### Features Added

| Feature | Description |
|---|---|
| **Veo Studio panel** | New `VeoStudioPanel.tsx` component mirroring Image Studio's layout — left controls pane + right video results pane |
| **Prompt input** | Full text prompt & optional negative prompt (collapsible) |
| **Style chips** | Quick-add style suggestions: `cinematic`, `slow motion`, `time-lapse`, `aerial view`, `documentary`, `animation`, `action`, `nature` |
| **Aspect ratio** | Toggle between `16:9`, `9:16`, and `1:1` video formats |
| **Duration selector** | Choose between `5s` or `8s` video length |
| **Provider & model dropdowns** | Google Veo 2.0 Flash (default) and Veo 3.0 Flash (preview) |
| **Video preview player** | Native `<video>` player with controls for reviewing generated videos |
| **Send to Chat** | One-click to send the prompt back to the Chat panel |
| **Recent prompts history** | Tracks last 8 prompts for quick reuse |
| **Purple/violet theme** | Distinct visual identity separating Veo Studio from Image Studio |
| **`mint veo` CLI command** | A new CLI command to generate videos from text prompts (e.g., `mint veo "a bird flying" --aspect 16:9 --duration 5`) |
| **`/veo` Slash Command** | A new slash command in interactive CLI chat mode to quickly trigger video generation (e.g., `/veo a dragon flying --aspect 16:9 --duration 8`) |


### Files Changed

- `src/renderer/src/components/VeoStudioPanel.tsx` — **[NEW]** React component
- `src/renderer/src/css/veo-studio.css` — **[NEW]** CSS with purple accent palette
- `src/renderer/src/index.css` — Added `@import './css/veo-studio.css'`
- `src/renderer/src/components/DashboardSidebar.tsx` — Added `'veo'` view type + sidebar entry
- `src/renderer/src/components/MintDashboard.tsx` — Imported and wired `VeoStudioPanel`
- `src/renderer/src/tauri.ts` — Added `VideoGenRequest`, `VideoGenResponse`, `VideoGenEntry`, `VideoGenProviders` types + stub `generateVideo()` / `getVideoGenProviders()`

> **Note:** The backend video generation API (Veo REST integration) is stubbed and will be fully connected in the next update.

---

## 🖱️ Browser Automation — Native Mouse & Keyboard Control

This release significantly upgrades `mint auto` browser automation with real native input control via Chrome DevTools Protocol (CDP), matching the behavior seen in advanced AI browser agents.

### New Tools

| Tool | Description |
|---|---|
| `browser_mouse_move` | Move the real mouse cursor to absolute (x,y) coordinates |
| `browser_mouse_click` | Native mousePressed + mouseReleased at (x,y) with configurable button |
| `browser_key_press` | Press real keyboard keys (Enter, Tab, Escape, F1–F12, etc.) via CDP |
| `browser_screenshot` | Capture the current page as a base64 PNG image |

### Upgraded Tools

- **`browser_click`**: Now uses native CDP mouse events (gets element coordinates via `getBoundingClientRect`, then dispatches `mousePressed`/`mouseReleased`), falling back to JS `.click()` for off-screen elements.
- **`browser_type`**: Upgraded to use `Input.insertText` CDP command (native keyboard), typing character by character like a real user. Also clicks the target element first to focus it.

### Visual Cursor Overlay

- Browser pages controlled by `mint auto` now show an **animated mouse cursor** (SVG arrow with green stroke) that moves in real-time as the AI controls the mouse.
- The cursor has a smooth CSS transition and a click animation (scale shrink) when clicking.
- The green aura border remains and now coexists with the new cursor overlay.

### Internal Architecture

- Added `cdp_call_raw()` — lightweight CDP call without overlay injection, used for all Input.* and Page.captureScreenshot methods to avoid unnecessary JS round-trips.
- Added `get_element_coordinates(selector)` — helper that returns viewport-relative (x,y) center of any CSS selector element.
- Added `type_text_native(text)` — pure CDP keyboard input using `Input.insertText`.
- Added `key_to_cdp_params(key)` — maps key names to `windowsVirtualKeyCode` and CDP code strings.
- **`AgentInput`** struct extended with `x: Option<f64>`, `y: Option<f64>`, `button: String`, `key: String` fields.
- Log display in `mint auto` updated with new emoji indicators: 🖱️ (mouse move), 🖱️● (click), ⌨️ (key press), 📸 (screenshot).

---



### 🖥️ 1. Rich Terminal Spinner Integration (`indicatif`)
- Added steady-tick green spinner loaders to keep the CLI interactive and visually responsive during blocking background operations:
  - Repository metadata fetching and AI analysis (`GithubOverview`).
  - Codebase indexing & syntax tree parsing (`Command::Symbols`).
  - Semantic vector database embedding indexing (`SemanticCodeCommand::Index`).
  - AI image generation (`Command::Imagine`).
  - Wait for OAuth browser redirection flow (`GmailCommand::Auth`).
  - Application updates check and NPM dependency installations (`updater`).

### 🤖 2. Dynamic Moon Walk Thinking Loader
- Replaced standard loaders with an elegant Moon phase vector animation loop (`🌑`, `🌒`, `🌓`, `🌔`, `🌕`, `🌖`, `🌗`, `🌘`) forced into a clean text-presentation style and glowing in a mint-green color.
- Implemented trailing dynamic dot padding (`""` -> `"."` -> `".."` -> `"..."` -> `""`) to keep line length constant and completely prevent terminal jitter.

### 🎨 3. Glowing Bold Wave Text Scanner
- Added a floating light wave effect (`apply_wave_effect`) that slides color gradients (Cyan `BLUE`, Mint `MINT`, and Gray `DIM`) dynamically across the letters of the thinking text from left to right.
- Changed the font weight to bold (`\x1b[1m`) and characters to full-width (`Ｔｈｉｎｋｉｎｇ` / `ｉｓ  ｔｈｉｎｋｉｎｇ`) to match CJK character scaling, making the text physically larger and highly prominent in the console.

### 📦 4. Codebase Modularization & Refactoring
- Split `main.rs` into modular helper components (`markdown.rs`, `actions.rs`, `interactive.rs`) to improve structure and readability.
- Cleaned up unused compiler warning imports and standardized process exit codes.

### 🧪 5. Automated CI/CD Workflow Releases
- Reconfigured the GitHub Actions compiler pipeline (`release.yml`) to automatically parse and publish this `Release_Note.md` directly as the release description body.

---

## 🛠️ Codebase Changes Summary
- **Tauri Backend**: Refactored learned skill directory resolver, process suggestion backgrounds, and added global active task cancel hooks (`ACTIVE_AGENTS`).
- **Web/Desktop Frontend**: Refactored component layout structures, consolidated client-side Speech-to-Text hooks, integrated Material Icon SVGs for agent file explorations, and cleaned up clipboard paste warnings.
- **CLI Agent**:
  - Reorganized autocomplete commands alphabetically and paginated console inputs to 5 commands max per page.
  - Added dynamic skill prompts (`$`).
  - Added a Crossterm-based interactive arrow-key selection menu (themed in active Blue/Cyan highlight) to `/models`, `/image-provider`, `/fast`, `/multi-agent`, and `/clear` commands.
  - Implemented a custom 24-bit Truecolor Mint-to-Blue gradient renderer for the ASCII welcome banner logo.
  - Upgraded the "Thinking" status loader with a smooth 24-bit Truecolor sine-wave gradient wave animation, and restored standard English characters for cleaner rendering.
  - Improved the MCP safety policy error with direct instructions to run `/mcp allow` to authorize blocked tools.
  - Implemented background `stderr` and `stdout` monitoring for MCP servers to automatically detect OAuth URLs, increased default tool timeout to 30 seconds for slow `npx` resolution, and launch the default system browser with extended timeouts.
  - Injected the list of available MCP servers directly into the system prompt's `mcp_tool` description in `orchestration.rs` to prevent agent naming hallucinations.
  - Implemented the `mcp_list_tools` capability to allow the AI agent to query the complete list of registered tools from any configured MCP server dynamically, enabling self-discovery of tool APIs.
  - Upgraded `/mcp` command to use a two-step interactive menu (themed in active Blue/Cyan highlight) to select a configured server first, show its status, and authorize all tools on the spot.
- **Workspace Skills Loading**: Modified skill loading to only supply metadata (name, description, path) for workspace-relative skills in the initial context, forcing the AI agent to explicitly invoke `read_file` to read the skill files. This makes skill reading visible as tool call logs in the user interface. Added chat history check to mark skills as READ on subsequent turns, preventing redundant reads on every turn.
