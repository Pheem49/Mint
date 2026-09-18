# Release Notes - Mint Agent v1.15.0

## Custom Provider Fallback & Groq TPM Optimization (CLI, Desktop & Web)

Fixed provider failover tracking and optimized request payloads for custom providers (especially Groq):

- **Correct Fallback Provider Attribution Across All 3 Surfaces (CLI, Desktop, Web)**:
  - Fixed an inversion bug in `crates/mint-core/src/orchestration/mod.rs` where `final_fallback` stored the *new fallback provider* instead of the *original failed provider*. This previously caused the UI to render contradictory badges like `"Gemini unavailable, fell back to Gemini."`.
  - Propagated `fallback_reason` through `AgentResult` to `ChatResponse` across `api_server`, `src-tauri`, and `mint-cli`, allowing Desktop and Web UI to explain why failover occurred (e.g. `Groq unavailable (Groq payload too large (HTTP 413)), fell back to Gemini.`).
  - Updated CLI (`crates/mint-cli/src/agent.rs`) badge formatting to accurately reflect `original • original_model (reason) → fallback: new • new_model`.
- **Groq Token Limit & TPM Optimization (`openai_chat_payload`)**:
  - Dynamically throttles `max_tokens` for Groq custom endpoints (to `2048` when tools are present, or `4096` in chat) instead of hardcoding `8192`, preventing requests from instantly exceeding Groq free tier's strict 8,000 TPM limit with HTTP 413.
- **Provider Label Formatting**:
  - Updated `providerLabel` in `src/renderer/shared/utils/providers.ts` to automatically strip `custom:` prefixes and format provider names cleanly (e.g., `custom:groq` -> `Groq`).

## Theme Presets: 3 One-Click Visual Themes (Web & Desktop UI)

Added ready-to-use **Theme Presets** to the **Settings > Appearance > Theme & UI** panel for Web and Desktop UI:

1. **Dark Mint**: พื้นหลังสีดำ (`#0a0a0b`) · สีองค์ประกอบสีเขียว (`#10b981`) · สีตัวหนังสือสีขาว (`#ffffff`)
2. **Light Mint**: พื้นหลังสีขาว (`#f8fafc`) · สีองค์ประกอบสีเขียว (`#10b981`) · สีตัวหนังสือสีดำ (`#000000` / `#0f172a`)
3. **Dark Monochrome**: พื้นหลังสีดำ (`#0a0a0b`) · สีองค์ประกอบสีขาว (`#ffffff`) · สีตัวหนังสือสีขาว (`#ffffff`)

- **Interactive Preset Grid**:
  - Interactive cards featuring miniature preview windows showing simulated UI elements (window dots, accent pill badge, typography preview, and color swatch dots).
  - One-click application updating theme, accent color, system text color, and chat text color simultaneously.
  - Active checkmark badge and glowing border when the active settings match a preset.
- **Dynamic Contrast & Light Mode Audits**:
  - Automatically calculates `--accent-contrast` based on accent relative luminance, ensuring buttons and chips with white accent backgrounds (Preset 3) maintain dark, high-contrast text rather than white-on-white.
  - Fixed hardcoded `#ffffff !important` on `.chat-bold-highlight` across Web and Desktop CSS, resolving invisible bold text in light mode.
  - Fixed hardcoded `#ffffff` on `.chat-heading-1..3` and `.chat-section-title` in `chat.css` to use dynamic `var(--text-main)`.
  - Refactored `.btn-secondary` in settings (`base.css`) from hardcoded `#252526` to `var(--surface-strong)` and `var(--text-main)`, allowing "Reset Defaults", "Set active", and secondary buttons to adapt cleanly to light backgrounds.
  - Added dedicated light-theme styling for `.chat-inline-code` badges and code syntax highlight tokens.
- **White Accent Support**:
  - Added `#ffffff` directly to Accent Color swatches with high-contrast checkmarks.

## Next-Gen Rich Markdown System: UI Components, Concept Mockups & Callouts (Desktop, Web & CLI)

Inspired by modern generative UI design, overhauled the markdown presentation pipeline across all 3 platforms (**Desktop UI**, **Web UI**, and **CLI**):

- **Interactive UI Cards & Grids (`ui-grid` & `ui-card`)**:
  - **`UiGridCard.tsx`**: Renders 2x2 or responsive multi-column option cards with dynamic Lucide icons (`message-square`, `mail`, `smartphone`, `video`, `code`, `database`, etc.), titles, descriptions, and badge tags.
  - **`UiFeatureCard.tsx`**: Renders rich feature showcase cards with thumbnail image support, status pill badges (`green`, `blue`, `purple`, `amber`, `red`), and key-value specs (`ปัญหา`, `วิธีแก้`, `กลุ่มผู้ใช้`).
  - Interactive hover elevations, glassmorphic dark-mode backgrounds, and responsive collapsing.
- **Generative UI Mockup Widget (`ui-mockup`)**:
  - **`UiMockupWidget.tsx`**: Renders interactive UI prototype mockups directly inside chat responses.
  - Features macOS-style window header, interactive file dropzone area with simulated toggle, parameter metric badges, and interactive option toggles with concept disclaimer.
- **GitHub Alert Callouts & Inline Badges**:
  - **GitHub Alerts in Markdown (`markdown.tsx`)**: Custom blockquote rendering for `> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`, `> [!WARNING]`, and `> [!CAUTION]` featuring themed borders, tinted backgrounds, and Lucide icons (`Info`, `Lightbulb`, `AlertCircle`, `AlertTriangle`, `ShieldAlert`).
  - **Inline Badges**: Added syntax support for `[badge:color text]` (colors: green, blue, purple, amber, red) and default `[badge:text]`, rendered as clean pill badges.
- **Complete CLI Terminal Parity (`crates/mint-cli`)**:
  - **`markdown_render.rs`**: Built native ANSI and Unicode box-drawing renderers for `ui-grid`, `ui-card`, and `ui-mockup` code fences (`┌─┐`, `│`, `└─┘`, `[◆]`).
  - Implemented terminal callout formatting for GitHub alerts with colored left borders and clean status headers (e.g. `│ NOTE:`, `│ TIP:`, `│ IMPORTANT:`, `│ WARNING:`, `│ CAUTION:`).
  - Implemented inline ANSI badge styling with subtle background colors.
- **Dedicated Shared Stylesheet (`chat-ui-cards.css`)**:
  - Centralized all styling for UI feature cards, grid cards, mockup widgets, and alert callouts into [`src/renderer/shared/css/chat-ui-cards.css`](file:///home/pheem49/vscode/Project/Mint-CLI/src/renderer/shared/css/chat-ui-cards.css) with semantic classes, making theme customization and styling tweaks straightforward.
- **Agent Prompt Guidance (`crates/mint-core`)**:
  - Added Rule 11c guiding the model on when to leverage `ui-grid`, `ui-card`, `ui-mockup`, and GitHub callouts for rich answers.

## Collapsible TUI Reasoning & Interactive Thought Viewer (`Ctrl+T` & `/thought`)

Streamlined AI reasoning / thinking display in the CLI to keep chat history clean and focused while giving full access to deep thought processes:

- **Collapsed Thought Summary in Chat Stream**:
  - Instead of dumping raw reasoning text onto the terminal scrollback, thoughts are now neatly collapsed into a single muted line:
    `  • Thought for 2.4s (Ctrl+T to view)`
  - Tracks and formats duration accurately with sub-second precision (`{:.1}s`).
- **Interactive Scrollable Thought Viewer (`Ctrl+T`)**:
  - At the prompt box, pressing `Ctrl+T` opens an inline, scrollable Ratatui thought viewer displaying the full thought process.
  - Supports keyboard navigation: `↑`/`↓`/`j`/`k` (line scrolling), `PageUp`/`PageDown` (page scrolling), `Home`/`End`/`g`/`G` (top/bottom), and `Esc`/`Enter`/`q`/`Ctrl+T` to close.
  - Automatically restores any active draft input in the prompt box upon exit.
- **Cross-Platform Slash Command Parity (`/thought` & `/think`)**:
  - Registered `/thought` across CLI, Web, and Desktop interfaces in `slash-commands.json` and `mint-core`.
  - In CLI, running `/thought` opens the full thought viewer for the previous turn.
- **Smart Progress Notes & CoT Separation (Claude Code Style)**:
  - Intelligently distinguishes internal chain-of-thought (CoT) reasoning from actionable progress notes.
  - Deep multi-paragraph internal reasoning is collapsed into `  • Thought for Xs (Ctrl+T to view)`.
  - Concise progress notes and step-by-step narration (e.g. `เดี๋ยวจะลองเปิดอ่านไฟล์ Cargo.toml ดูก่อนนะคะ...`) are displayed directly on the timeline before each tool call for maximum visibility.

## CLI Conversation Isolation & Interactive Session Resumption (`/resume`, `mint -r`, Claude Code Style)

Overhauled session management in the CLI to give every conversation its own isolated lifecycle with Claude Code-style interactive session resuming across all 3 platforms:

- **Isolated CLI Sessions & Auto-Titling**:
  - Each CLI session now receives a unique, dedicated session ID (`cli::<short-uuid>`) rather than grouping all historical conversations into a single monolithic `"cli"` identifier.
  - Automatically captures workspace directory (`workspace_path`), current git branch (`git_branch`), primary project language (`main_language`), message turn counts, and total payload bytes in SQLite.
  - Automatically titles sessions according to the user's opening prompt (e.g. `> Scheduled Tasks bug`).
  - Existing legacy CLI history in `"cli"` remains 100% preserved and accessible.
- **Claude Code-Style Interactive Resume Picker (`resume_picker.rs`)**:
  - Built an interactive Ratatui + Crossterm TUI picker mirroring Claude Code's session resume interface:
    - **Strict CLI-Scoped Isolation**: Filters out general Web and Desktop conversations as well as background cron tasks, presenting only authentic CLI sessions in the terminal picker.
    - **Live Search**: `[⌕ Search...]` box with instantaneous substring/fuzzy filtering across titles, branch names, and IDs.
    - **Rich Session Cards**: 2-line cards displaying Title (`> Scheduled Tasks bug`) with relative timestamps, language tag, git branch, and total size (e.g. `2 weeks ago · Rust · 6.3MB`).
    - **Cross-Project Navigation (`Ctrl+A`)**: Toggle between filtering by the current workspace versus browsing sessions across all projects on the system.
    - **Branch Scoping (`Ctrl+B`)**: Quick-filter sessions to only those active on the current Git branch.
    - **Session Quick Preview (`Space`)**: Pop-up drawer displaying recent user/assistant turns from the highlighted session without leaving the picker.
    - **Inline Session Renaming (`Ctrl+R`)**: Change any session's title inline and persist it immediately to SQLite.
    - **Unicode / UTF-8 Multi-Byte Safety**: Replaced unsafe byte slicing with codepoint-aware truncation (`truncate_utf8`) for message snippets and previews, ensuring flawless rendering for non-ASCII languages (Thai, Japanese, Chinese, emojis) without panicking.
- **Dedicated CLI Flags & Subcommands**:
  - `mint --resume` / `mint -r`: Opens the interactive session picker on CLI launch before entering the prompt loop.
  - `mint resume [id]`: Direct subcommand to resume by ID or launch the picker.
  - `/resume [id|query]`: Slash command in interactive chat to hot-swap sessions mid-conversation.
  - **Exit Banner Resume Hint**: Upon exiting the interactive CLI session via `Ctrl+D`, `Ctrl+C`, or `/exit`, the closing banner displays the exact command to resume the session (`mint --resume <session_id>`), matching Claude Code's session exit experience.
- **Platform Parity across CLI, Desktop, and Web**:
  - Registered `/resume` in `slash-commands.json` for all three surfaces (`["cli", "web", "desktop"]`).
  - Integrated `/resume` dispatch in `slashCommandProcessor.ts` and `MintDashboard.tsx` to hot-swap active conversations seamlessly on Desktop and Web.



