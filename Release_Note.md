# Release Notes - Mint Agent v1.11.0

We are excited to release **Mint Agent v1.11.0**! This version introduces major enhancements across CLI, Desktop, and Web: Core Rust Crates Integration (`ignore`, `grep-searcher`, `shlex`, `srtlib`, `tree-sitter`), **Interactive Native UI Widgets Suite (Weather, Stock & Financial, Calculation & Unit Conversion)**, Multimodal Video, Remote Messaging Bridges, Browser Automation, LaTeX Sanitization, **AI Image-to-Image & Inpainting Editing Capabilities**, and the **AI Video Editor Core & Auto Shorts Generator**.

## 🧠 Agentic Core Overhaul — Native Tool-Calling, Context Compaction & Safety Hardening

A ground-up rework of the coding-agent loop (`crates/mint-core/src/orchestration.rs`), moving from a hand-parsed JSON-in-text protocol to real provider-native tool-calling, plus a full pass of reliability and safety fixes — including two bugs found and fixed through live testing against real Anthropic, Gemini, OpenAI-family, and DeepSeek APIs.

- **Native Tool-Calling (Anthropic, OpenAI-family, Gemini, allowlisted Ollama models)**: Replaced the old "reply with one JSON blob" scheme with real `tools` / `functionDeclarations` API calls per provider (`crates/mint-core/src/chat.rs`), each backed by request/response golden tests. A per-tool JSON schema catalog (`crates/mint-core/src/prompts/tool_catalog.rs`) now enforces argument shapes at the API level for all 49 agent actions instead of one giant shared free-text struct. Providers without reliable native tool support automatically fall back to the legacy JSON-prompt path — with a **visible warning**, never a silent downgrade.
- **🐛 System Prompt Fix (found via live API testing)**: The system prompt was unconditionally instructing the model to reply with raw JSON text — even in native tool-calling mode — which actively sabotaged real tool calls for every native-capable provider. Fixed in `crates/mint-core/src/prompts/agent.rs` to build a leaner, tool-calling-appropriate prompt when native mode is active; verified live against Gemini and DeepSeek after the fix.
- **🐛 `read_file` Silent Truncation Fix (found via live API testing)**: Reading a file longer than 240 lines returned a truncated result with **no indication it was incomplete**, so the agent had no way to know it needed to keep reading. `crates/mint-core/src/code_tools.rs` now reports the file's total line count and tells the model the exact `startLine`/`endLine` to request next — verified end-to-end against a real 323-line file, which the agent now reads and summarizes in full.
- **Context Compaction**: Long-running agent conversations now track real token usage reported by each provider and automatically summarize older tool-call history into one condensed entry once usage crosses 80% of the model's context window, keeping the most recent steps verbatim.
- **Subagent Isolation (`dispatch_subagent`)**: New `.agents/subagents/*.md` file-based subagent definitions the top-level agent can delegate focused sub-tasks to. Each subagent runs in a fully isolated context (its own message history, optional restricted tool set, optional different model/provider) and returns only its final summary — its internal steps never leak into the parent conversation. Listable via `mint code subagents`.
- **Persistent Permission Rules**: Approval prompts for shell commands, file edits, plugins, and MCP tools can now be saved as "always allow" rules (per-project or global) so the same action doesn't need re-approving every time. Manageable via `mint safety permissions list` / `remove`.
- **Sandbox Visibility**: Shell commands that run unconfined because OS-level sandboxing (`bwrap` / `sandbox-exec`) isn't available now surface an explicit warning instead of silently skipping sandboxing; status visible via `mint config doctor`. Also hardened the destructive-command blocklist (`git push --force`, `find -delete`, `rsync --delete`).
- **Persistent MCP Sessions**: MCP servers no longer spawn a brand-new process for every single tool call — sessions are now reused across calls, with automatic reconnect if a server process dies. Added support for `resources/list`, `resources/read`, `prompts/list`, and `prompts/get`.
- **Atomic File Writes & Edit Uniqueness**: File edits now write atomically (temp file + rename) instead of a direct overwrite, and `apply_patch` rejects ambiguous edits that match more than one location in a file instead of silently editing the wrong spot.

---

## 🎨 Interactive Native UI Widgets Suite (Weather, Stock & Calculation)

Ported and integrated native interactive widgets across CLI, Desktop UI, and Web UI:

1. 🌤️ **Weather & Daily Forecast Widget**: Real-time Open-Meteo weather fetch, 5-day daily forecast grid, wind speed, humidity, and radial-gradient Blue Weather Card UI (`WeatherCard.tsx`).
2. 📈 **Stock & Financial Asset Widget**: Real-time market quotes via Yahoo Finance v8 API for stocks (AAPL, TSLA, NVDA) and cryptocurrencies (BTC, ETH, SOL) with positive/negative price gain/loss indicators, ticker resolution, and styled Stock Card UI (`StockCard.tsx`).
3. 🧮 **Calculation & Unit Converter Widget**: Expression evaluation, percentage calculations ("25% of 8500"), and unit/currency conversions rendered in a modern, transparent Glassmorphism Calculator Card UI (`CalculationCard.tsx`).
4. 📱 **Universal Cross-Platform & Dual Desktop/Web Parity**: Single-source-of-truth card components in `src/renderer/shared/components/` rendering seamlessly across Desktop Tauri App and Web Browser Client.

---

## ⚙️ Standard Crates Refactoring (URL Opening, Binary Lookup & HTML Scraping)

Replaced custom string manipulations and OS-specific subprocess calls with battle-tested standard Rust crates:

- **`open::that(url)`**: Replaced platform-conditional (`xdg-open` / `open` / `cmd`) process spawning in `crates/mint-core/src/mcp.rs` with `open` crate for seamless cross-platform URL opening across Linux, macOS, Windows, Flatpak, and WSL.
- **`which::which(command)`**: Replaced process-based `which` CLI calls in `crates/mint-core/src/shell.rs` with `which` crate for cross-platform binary executable lookup.
- **`scraper::Html`**: Replaced string matching (`.find("og:image")`) in `crates/mint-core/src/web_search.rs` with `scraper` CSS selector query (`meta[property='og:image']`, `meta[name='twitter:image']`) for robust Open Graph thumbnail extraction.
- **`AST & Syntax Boundary Chunking`**: Refactored `chunk_text()` in `crates/mint-core/src/semantic.rs` to break chunks at natural code boundaries (function/class declarations, doc comments, closing braces, empty lines) instead of fixed character positions, drastically improving code embedding vector quality.

---

## 🎨 Enhanced Settings UI (SVG Icons & Visual Theme Swatches)

Upgraded the Settings interface across Desktop (`src/renderer/src`) and Web (`src/renderer/src-web`) to improve visual hierarchy and scanability:

- **Section Group SVG Icons**: Added clean SVG icons to section headers (`GeneralTab.tsx`, `AutomationTab.tsx`) including AI Routing 🧠, API Keys 🔑, Web Search 🔎, Automation Engine 🌐, and Native Headless Queue ⚡.
- **Authentic Brand Plugin SVG Icons (`plugins.tsx`)**: Replaced generic outline icons in the Plugins tab (`PluginsTab.tsx`) with official multi-colored SVG brand logos for Spotify 🟢, Discord 🟣, Gmail ✉️ (Google 4-color M), Google Calendar 📅 (Google 4-color 31), Notion ⬛ (authentic N logo), YouTube Music 🔴, Vercel 🔺, and GitHub 🐱 across Desktop and Web UIs.
- **Expanded Settings Modal Width (`styles.css` & `desktop.rs`)**: Increased Settings window width from `920px` to `1180px` (`min(1180px, 95vw)`) in Web UI CSS and updated Desktop Tauri window dimensions to `1180.0` x `780.0`, providing significantly more horizontal space for plugin cards, API key inputs, and integration descriptions.
- **Action Buttons with SVG Icons**: Enhanced footer buttons (`SettingsWindow.tsx`) with inline SVG icons for Save Settings 💾, Reset Defaults 🔄, and Quit Application 🚪.
- **Dual UI Build Command (`package.json`)**: Updated `npm run build` script to execute both Desktop UI (`build:desktop:ui`) and Web UI (`build:web`) in sequence for complete platform parity.

- **Updated Google Veo Video Generation Models**: Upgraded Veo video generation presets across CLI, Desktop UI, Web UI, and Rust Core engine to `veo-3.1-generate-preview` ⭐, `veo-3.1-fast-generate-preview`, and `veo-3.1-lite-generate-preview`, removing deprecated Veo 2.x models.
- **Dedicated Black Forest Labs (FLUX API) Integration**: Added official direct FLUX API (`api.bfl.ml`) provider (`bfl` / `flux`) separate from Replicate across CLI, Desktop UI, Web UI, and Rust Core engine with Text-to-Image models (`flux-pro-1.1`, `flux-pro-1.1-ultra`, `flux-pro`, `flux-dev`, `flux-schnell`) and Image Editing models (`flux-kontext-pro`, `flux-kontext-max`, `flux-fill-pro`).
- **Updated Model Presets Across Providers**: Updated model option lists across CLI, Desktop UI, and Web UI for Gemini, Anthropic Claude, OpenAI, OpenRouter, Hugging Face, and Image Generation Studio (`gemini-3.1-flash-image` ⭐, `gpt-image-1` ⭐, `stable-image-ultra` ⭐, `Ideogram V3` ⭐, `FLUX 1.1 Pro` ⭐).
- **Centralized Model Manager & Bi-directional Sync (`modelManager.ts`)**: Built reactive Model Manager bus to synchronize active model selections in real-time between Veo Studio, Image Studio, Chat Panel, and Settings Window across CLI, Desktop UI, and Web UI.
- **OAuth 2.0 PKCE Popup Sign-In System (`oauth.rs`, `oauthManager.ts` & `lib.rs`)**: Built browser-based OAuth 2.0 PKCE authorization and local callback engine (`GET /api/oauth/callback`) with popup sign-in support for Google Services (Gmail, Calendar, YouTube Music), Vercel, GitHub, Spotify, and Notion across CLI, Desktop UI, and Web UI. Auto-spawns background API server on port 3000 during Desktop Tauri app launch for seamless single-click Sign In parity.
- **Single Source of Truth Architecture Refactoring**: Refactored `DEFAULT_CONFIG` (`config.ts`), AI model arrays (`models.ts`), aspect ratios, and studio style presets (`studio.ts`) into canonical `shared/` constants, eliminating code duplication across renderers.
- **Clean Path HTML5 History Routing (`App.tsx` & `MintDashboard.tsx`)**: Upgraded routing engine to HTML5 History API clean path URLs (`/chat`, `/pictures`, `/image-studio`, `/veo-studio`, `/settings`) with zero `#` symbols in the URL bar, while maintaining backwards compatibility for hash URLs (`#/pictures`). Fully supports direct URL linking, browser back/forward history navigation, and state preservation across F5 reloads across Desktop and Web UIs.

---

## ⚡ UI Performance & Composite Layer Optimizations (Desktop & Web)

Optimized rendering pipelines, state updates, and animation loops across both Desktop and Web interfaces:

- **Memoized Chat Message Items (`src/renderer/shared/components/ChatMessageItem.tsx`)**: Refactored `ChatPanel` message list to use `React.memo` with custom prop comparison and `useMemo` markdown parsing caches, preventing full message list re-renders and Regex re-parsing during AI response streaming.
- **Agent Activity Drawer Toggle Fix (`ChatMessageItem.tsx` & `ChatPanel.tsx`)**: Fixed an issue where clicking _"Working through task >"_ (historical agent activity drawer) did not open/expand. Added `openActivityIds`, `openReviewIds`, and `openFileDiffs` props and comparison checks to `ChatMessageItem`'s `React.memo` comparator, ensuring state updates correctly trigger message re-renders and drawer toggling across both Desktop and Web interfaces. Also restored historical activity fallback (`interaction.agentActivity`) when live snapshots are absent.
- **Throttled Live2D Pointer Move Tracking (`src/renderer/src/components/Live2DStage.tsx`)**: Capped pointer move events using `requestAnimationFrame` to 60FPS (~16ms), eliminating JavaScript main thread contention and micro-stutters from high-polling gaming mice (500Hz - 1000Hz).
- **Blob Object URL Attachment Previews (`src/renderer/shared/utils/ui.ts`)**: Replaced heavy Base64 Data URI strings in DOM preview `<img>` tags with lightweight `URL.createObjectURL(file)` blob previews, reducing React state diffing overhead and memory footprint.
- **CSS Layer Containment (`styles.css`)**: Added `contain: content;` and `will-change: transform;` to `.chat-container` in both Desktop and Web CSS stylesheets to isolate render composite boundaries and eliminate GPU repaint lag during chat scrolling.

---

## 🗑️ Picture & Video Deletion System (CLI, Desktop & Web)

Implemented full-stack media deletion across Rust core, Tauri IPC, Web REST API, and Desktop/Web UIs:

- **Rust Core & Disk Storage (`crates/mint-core/src/pictures.rs`)**: Added `delete_saved_picture(id)` function to remove picture/video media files on disk, delete generated thumbnails, and persist updated `pictures.json` metadata index.
- **REST & IPC Endpoints**: Exposed `DELETE /api/pictures/:id` HTTP REST endpoint in `crates/mint-core/src/api_server.rs` and `delete_picture` Tauri IPC command in `src-tauri/src/lib.rs`.
- **Interactive Deletion UI (`PicturesLibrary.tsx`)**: Added hover trash action button 🗑️ to picture/video cards and a confirmation modal dialog (_"Are you sure you want to permanently delete...?"_) across both Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`).

---

## 🖼️ Web Search Image Thumbnails + Inline Images in Response

Web searches now fetch and display representative images in two places:

1. **Sources strip** — horizontal thumbnail cards above the source chips
2. **Inline in the AI's answer** — images embedded between bullet points, just like the Dola AI app

### Changes

- **`crates/mint-core/src/web_search.rs`**: Added `image_url: Option<String>` to `SearchHit`. Google CSE extracts from `pagemap.cse_image[0].src`; Brave from `thumbnail.src`. Added **Open Graph fallback scraper** (`og_image_fallback`) — when the search API returns no thumbnail, fetches up to 8KB of each result URL in parallel (4 URLs, 4s timeout) and extracts `og:image` or `twitter:image` meta tags.
- **`crates/mint-core/src/orchestration.rs`**: Emits optional `Image: <url>` line per result. Updated the finish-summary instruction to tell the AI to embed `![title](url)` markdown image tags inline in its answer, placed immediately after the bullet point referencing each result. AI applies images only for visual topics (food, people, places, products) and skips them for code/text answers.
- **`src/renderer/shared/utils/agentActivity.ts`**: Added `imageUrl?` to `WebSearchSource`; `parseWebSearchSources()` detects and strips the `Image:` line.
- **`src/renderer/shared/utils/markdown.tsx`** (shared): Added two-tier image rendering — external `https://` URLs (web search OG images) use a compact `200px / objectFit: cover` preview card with title label. Internal `/api/` URLs (AI-generated images) keep the original `420px / objectFit: contain` style.
- **`src/renderer/src/components/ChatPanel.tsx` (Desktop)** & **`src/renderer/src-web/components/ChatPanel.tsx` (Web)**: `renderWebSearchSources()` renders a scrollable image card strip (max 4 cards) above source chips. Full platform parity.
- **`crates/mint-cli/src/markdown.rs` (CLI)**: `format_line()` now detects `![alt](url)` lines and renders them as `🖼  alt — url` with a cyan-colored clickable URL, instead of printing raw markdown syntax. Ctrl+Click / Cmd+Click the URL in supported terminals (iTerm2, WezTerm, VS Code terminal) to open in browser.

---

## 🎬 AI Video Editor & Auto Shorts Generator (Milestones 1–4 Complete)

Mint Agent now includes a full-featured **AI Video Editor Core** & **Auto Shorts Generator** available across CLI, Desktop UI, and Web UI! Every video operation is built as a first-class tool callable by both the user and the AI Agent.

### Features Added

- **Core FFmpeg Video Operations (`crates/mint-core/src/video_edit.rs`)**:
  - `video_trim`: Precise start and end timestamp clipping.
  - `video_resize`: Change aspect ratio and resolution (e.g. 1080p, 4K, 9:16 vertical).
  - `video_merge`: Concatenate multiple video clips cleanly via FFmpeg concat filter.
  - `video_extract_audio`: Extract high-fidelity audio tracks as WAV files.
  - `video_remove_silence`: Auto-detect audio quietness via `silencedetect` and trim out dead air.
  - `render_timeline`: Multi-clip non-linear JSON Timeline Engine.
- **Speech & Subtitles Engine (`crates/mint-core/src/speech.rs` & `subtitle.rs`)**:
  - Speech-to-Text (STT) transcription with OpenAI Whisper API integration, local `whisper` CLI fallback, or heuristic chunking.
  - LLM-based Subtitle Translation preserving SRT timing codes.
  - Styled Subtitle Burning with customizable ASS presets (`🔥 TikTok Bold`, `✨ Minimal White`, `📺 Standard`).
- **⚡ Make Auto Shorts (`crates/mint-core/src/auto_shorts.rs`)**:
  - Automatically analyze long video transcripts via LLM to extract top viral highlight moments.
  - Auto-crop and resize to 9:16 vertical (1080x1920) format.
  - Auto-generate and burn yellow TikTok-style subtitles onto output clips.
- **🖼️ & 🎬 Direct Image & Video Generation in Chat (Agent Mode)**:
  - Added **Generate image** and **Generate video** action options directly into the Chat plus (`+`) attachment menu dropdown.
  - Integrated `generate_image` (`DALL-E`, `Stability`, `NanoBanana`, `Replicate`) and `generate_video` (`Google Veo`) tool dispatch actions into `crates/mint-core/src/orchestration.rs`.
  - Added inline media rendering in `src/renderer/shared/utils/markdown.tsx` so when the AI Agent generates images or videos in Chat, the generated media (`![Image](...)` & `<video controls src="..."></video>`) renders directly inside the chat conversation bubble.
- **🎬 Fully Interactive Veo Studio AI Video Editor UI**:
  - Maintained header title **Veo Studio** with **AI Editor** badge across both Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`).
  - Expanded video preview canvas Viewport (`.capcut-video-viewport`) to dynamically fill 100% available height and width of the center workspace stage, maximizing video editing screen area.
  - Integrated Tauri native system file picker (`dialog.open()`) into the **Browse** button so picking a video file automatically captures the full absolute system path (`/path/to/video.mp4`), preventing `ffmpeg` file-not-found errors.
  - Fully integrated dynamic CSS application theme variables (`var(--accent)`, `var(--bg-color)`, `var(--panel-bg)`, `var(--input-bg)`, `var(--border)`) with a clean neutral dark (`#09090b`) video canvas stage, eliminating unwanted blue theme tinting around player preview.
  - Replaced all text emojis with high-quality, resolution-independent SVG vector icons across all control buttons, tool options, and timeline cards.
  - Interactive range scrubber & storyboard transcript cards carousel replacing multi-track lines.
  - Real-time player canvas controls (Play, Pause, Stop, Skip -5s/+5s) with formatted timecode.
  - Full Manual Tools control panel (Trim with playhead timestamp capture, Resize, Remove Silence, Extract Audio, Subtitles STT & burning, Auto Shorts, Export).
  - Instant Output Result vs Source Video toggle preview.
- **AI Agent Tool Registration & Platform Parity**:
  - Registered all video tools (`video_trim`, `video_resize`, `video_merge`, `video_extract_audio`, `speech_transcribe`, `subtitle_burn`, `make_shorts`) into `orchestration.rs` system prompt and dispatch arms so the AI Agent can execute video editing requests automatically from chat prompts.
  - CLI: `mint video load|trim|merge|resize|extract-audio|export|transcribe|subtitle|translate-subtitle|make-shorts` subcommands.
  - Full UI parity across Desktop (`src/renderer/src/components/VeoStudioPanel.tsx`) and Web (`src/renderer/src-web/components/VeoStudioPanel.tsx`).

- **🎨 Image Studio & Chat Image Generation Parity**:
  - Fixed `generate_image` tool dispatch in `crates/mint-core/src/orchestration.rs` to automatically invoke `crate::pictures::save_chat_images`, persisting generated images directly to physical disk (`~/.config/mint/Pictures/`) and indexing them into `pictures.json`.
  - Updated `generate_image` tool system prompt format by removing hardcoded `"provider": "dalle"`, allowing the AI Agent to respect the user's default selected provider in Image Studio / Settings (`NanoBanana (Gemini)`).
  - Added smart fallback from DALL-E to `call_nanobanana` (Gemini) when OpenAI API key is missing.
  - Updated `ImageStudioPanel.tsx` (Desktop UI & Web UI) gallery filtering to seamlessly display all AI-generated images from Chat (`chat`, `image_gen`, `cli`) while filtering out non-AI chat attachment uploads.
  - Implemented `resolveMediaUrl` in `src/renderer/shared/utils/markdown.tsx` to automatically resolve `/api/pictures/` URLs to origin server (`http://localhost:3000`), ensuring 100% image card rendering parity on Web UI (port 9000).
  - Added automatic media & model feedback attribution appending (`![Generated Image]`, `✓ Image generated successfully with model...`, and `Saved to: ...`) in `orchestration.rs` to guarantee complete model feedback display in chat bubbles.
- **💻 CLI Interactive Mode Slash Command Parity**:
  - Added `/generate-image <prompt>` and `/gen-image <prompt>` slash commands to CLI interactive `/help` menu and `AUTOCOMPLETE_COMMANDS` autocompletion list.
  - Connected `/generate-image` slash command directly to `SlashResult::ForwardToAgent`, sharing 100% of the native Agent Thinking animation engine (moon phase spinner frames, wave effect, real-time elapsed timer, and `Esc to interrupt`), central image persistence, and model attribution.

---

## 📦 Core Engine Crate Integrations (`crates/mint-core`)

- **`ignore` Crate Integration**: Replaced legacy `collect_files` with `ignore::WalkBuilder` in `list_code_files`, automatically respecting `.gitignore` and `.ignore` rules across workspaces.
- **`grep-searcher` & `grep-regex` Engine**: Refactored `search_code` to use `grep_searcher::Searcher` and `grep_regex::RegexMatcherBuilder` for fast, streaming code text searching powered by `ripgrep` internals.
- **`shlex` Shell Tokenization**: Upgraded `classify_shell_command` in `safety.rs` to use `shlex::split` POSIX shell lexing, ensuring quoted or escaped command strings (e.g. `rm\ -rf`) are properly tokenized before safety evaluation.
- **`srtlib` Subtitle Formatting**: Integrated `srtlib::Subtitle`, `Subtitles`, and `Timestamp` structs into `subtitle.rs` for standard `.srt` subtitle generation and millisecond timecode conversion.
- **`tree-sitter` AST Symbol Extraction**: Integrated `tree-sitter` (0.20.10) with `tree-sitter-rust` and `tree-sitter-typescript` grammars into `symbols.rs`, extracting function, struct, class, enum, trait, interface, and type declarations at the Abstract Syntax Tree level across Rust, TypeScript, JavaScript, and TSX files with automatic regex fallback.

---

## 🎨 Ecosystem Plugins, Utilitarian UI Redesign & OAuth 2.0 Client ID Engine

- **Single-Source-of-Truth Plugins Architecture (`src/renderer/shared/constants/plugins.tsx`)**: Extracted `BUILTIN_PLUGINS_LIST` and vector SVG logos (`renderMcpSvgIcon`) into shared constants, enforcing complete feature and visual parity across CLI (`crates/mint-cli`), Desktop UI (`src/renderer/src`), and Web UI (`src/renderer/src-web`).
- **Product-Native UI Redesign (Anti-AI Slop Aesthetic)**: Completely revamped `PluginsTab.tsx` in Desktop and Web UIs:
  - Stripped out emoji headings (`🧠`, `🔌`, `🧩`, `⚡`, `🟢`, `⚪`) and inline dashed card boxes.
  - Applied the clean Mint typography system (`section-kicker`, `section-title`, `section-description`).
  - Upgraded skill tiles and plugin cards with flat surface borders, high-contrast status tags (`Connected`, `Not Connected`, `Workspace`, `Global`), and high-resolution SVG brand logos.
- **Web UI Tauri IPC Crash Fix**: Resolved Web UI white screen crash by replacing direct `@tauri-apps/api` desktop IPC imports in `src-web/components/Settings/PluginsTab.tsx` with safe REST API HTTP endpoints (`src-web/tauri.ts`).
- **Dynamic OAuth Client ID Pass-Through (`api_server.rs`)**: Updated `/api/oauth/start` REST endpoint in Rust Core to dynamically inject user-configured Client IDs (`gmailClientId`, `googleCalendarClientId`, `spotifyClientId`, `notionApiKey`) into PKCE authorization flows.
- **Interactive CLI & Documentation**: Added `mint plugins` subcommand documentation and ecosystem configuration guide to `README.md`.

---

## 🎨 Dynamic Theme Engine, Management Views Redesign & Workspace Skills Detection

- **🎨 Dynamic Theme Management Views (`management-views.css`, `ui.ts`)**:
  - Consolidated standalone management stylesheets into `src/renderer/shared/css/management-views.css`.
  - Driven theme variables (`--accent`, `--accent-hover`, `--accent-glow`, `--accent-subtle`, `--accent-border`, `--text-main`) dynamically from Accent Color and Text Accent Color settings.
  - Set background of `/skills`, `/mcp`, and `/plugins` views in `MintDashboard.tsx` to `transparent`, allowing custom background gradients and wallpapers to show through seamlessly.
- **🎛️ Settings-Parity Toggle Sliders & Far-Right Controls (`McpServersView.tsx` & `PluginsView.tsx`)**:
  - Upgraded MCP and Plugin item rows to use iOS-style pill toggle sliders (`.settings-toggle-switch` & `.settings-toggle-slider`).
  - Added complete action button controls (`Sign In 🌐`, `Disconnect`, `Configure`, `Remove`).
  - Aligned right-side control containers to the far right edge (`marginLeft: 'auto'`, `flex: 1`) across all item cards.
  - Fixed MCP server toggle behavior to toggle `disabled: true / false` property instead of deleting the server configuration on toggle OFF.
- **✨ Vector SVG Icons (`plugins.tsx` & `DashboardSidebar.tsx`)**:
  - Replaced text emojis with vector SVG icons (`renderSkillsSvgIcon`, `renderMcpHubSvgIcon`, `renderPluginsSvgIcon`) in Sidebar `More` popover menu and page headers.
  - Updated section title from `Built-in Plugins & Integrations` to `Plugins & Integrations`.
- **🌐 Clean Routing & Form Submit Hash Cleanup (`SettingsWindow.tsx`, `MintDashboard.tsx` & `tauri.ts`)**:
  - Added `type="button"` and `e.preventDefault()` to `SettingsWindow` footer buttons.
  - Built automatic hash cleanup in `MintDashboard.tsx` and `closeSettings()`, preventing unwanted `#/settings` hash accumulation in browser address bar.
- **🦀 Rust Workspace vs. Global Skills Engine (`memory.rs`, `skills.rs` & `api_server.rs`)**:
  - Added `is_workspace` field (`#[serde(rename = "is_workspace")]`) to `LearnedSkill` struct in `mint-core`.
  - Updated `/api/learned-skills` REST endpoint in `api_server.rs` to compute whether a skill file (`.agents/skills/*`, `AGENTS.md`) is located within the active project workspace root (`current_dir`), rendering accurate teal **`• Workspace`** badges vs. purple **`• Global`** badges in the UI.

---

## 🎙️ Gemini Live Voice Fixes, Messaging Bridges & CLI Syntax Highlighting

- **🐛 Gemini Live Silent-Session Fix (found via live API testing)**: `crates/mint-core/src/gemini_live.rs` only matched `Message::Text` when reading server frames, but the Gemini Live WebSocket sends every event — `setupComplete`, audio replies, transcripts, tool calls — as `Message::Binary`. Every single server event was being silently dropped by a `_ => continue` arm, so Live Mode connected and streamed mic audio fine but the model's replies never reached the UI. Fixed to decode both frame types; verified end-to-end against the real Gemini Live API (setup handshake, a spoken text-turn reply, and clean session close).
- **🗣️ Gemini Live Voice Selection**: Added `speechConfig.voiceConfig.prebuiltVoiceConfig` to the Live session setup (`gemini_live.rs`, `resolve_live_voice`), configurable via a new `geminiLiveVoice` setting. Exposed as a dropdown in Settings → Audio (`AudioTab.tsx`, both Desktop and Web) and as a live voice-picker button directly inside the Live overlay (`GeminiLiveOverlay.tsx`) that reconnects the session with the new voice immediately, without losing the persisted setting.
- **📲 LINE & WhatsApp Messaging Bridges (real implementations, not just Settings UI)**: The Settings UI for LINE Messaging and WhatsApp Cloud API bridges existed with zero backend behind it — enabling either toggle did nothing. Implemented `line_webhook_loop()` and `whatsapp_webhook_loop()` in `crates/mint-core/src/channels.rs`: small hand-rolled HTTP servers (`lineWebhookHost:lineWebhookPort`, `whatsappWebhookHost:whatsappWebhookPort`) with HMAC-SHA256 signature verification (`X-Line-Signature`, `X-Hub-Signature-256`), the WhatsApp `hub.challenge` verification handshake, and replies routed through the same `answer_channel` path Telegram/Discord/Slack already use. Verified end-to-end over real loopback HTTP requests (valid/invalid signatures, verification handshake, message delivery).
- **🧩 Subagent Settings UI (`dispatch_subagent`)**: The file-based subagent-delegation system (`subagents.rs`, `.agents/subagents/*.md`) had a fully working backend and zero way to create one without hand-writing Markdown frontmatter. Added `save_subagent()`/`delete_subagent()` write functions to `subagents.rs`, exposed via Tauri commands (`list_subagents`, `save_subagent`, `delete_subagent` in `src-tauri/src/lib.rs`) and REST endpoints (`GET/POST /api/subagents`, `DELETE /api/subagents/:sourcePath` in `api_server.rs`), and built a full CRUD section in Settings → Agents (`AgentsTab.tsx`, both Desktop and Web) — create/edit/delete subagents with name, description, system prompt, allowed-tools list, model/provider override, and workspace-vs-global scope.
- **🎨 CLI Code Block Syntax Highlighting**: Code blocks in `mint`'s interactive/agent output got a bordered box with a language label but no coloring of the code itself. Integrated `syntect` (pure-Rust `fancy-regex` backend, no C toolchain required) into `crates/mint-cli/src/agent.rs`'s `format_markdown_bold` — the fenced block's language hint now drives real token-level syntax highlighting (`base16-ocean.dark` theme), with a clean fallback to plain unhighlighted text for unrecognized or missing language hints.
