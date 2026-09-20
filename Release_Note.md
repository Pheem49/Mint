# Release Notes - Mint Agent v1.15.0

## Desktop Workspace File Explorer UI

- Rebuilt the missing Workspace stylesheet as a compact desktop file explorer with a clear header, flat action toolbar, sticky project root, correctly sized material file icons, and dense scannable tree rows.
- Restored the intended two-pane Workspace layout with the file explorer on the left and chat composer on the right, while hiding the Live2D stage and preventing empty-chat positioning rules from overlaying the full workspace.
- Reworked the workspace selector from a prominent accent card into a compact project-context row integrated with the composer, using neutral surfaces and restrained hover/focus feedback while preserving folder selection and full-path tooltips.
- Added a compact Local Branch selector beside the active workspace, including branch search, current and detached-HEAD states, dirty-worktree warnings, safe Git switching, and immediate workspace-tree refresh after a successful switch.
- Extended the Branch selector with create-and-switch, locally known remote tracking branches, and an in-place Git graph showing the latest decorated repository history without performing an implicit network fetch.
- Replaced decorative gradients, glow effects, elevated cards, and AI-style status capsules with restrained surfaces, structural dividers, typography, and a small state indicator suited to a developer tool.
- Added keyboard focus treatments, disabled and active control states, long-name truncation, contained scrolling, narrow-window adjustments, reduced-motion behavior, and forced-colors support.
- Preserved the existing workspace file creation, folder creation, refresh, drag-to-mention, expand/collapse, delete, and automatic refresh behavior without changing component logic.

## Desktop Empty Chat Layout & Workspace Selector

- Fixed the empty-chat header so the Mint Agent identity and conversation actions stay anchored at the top of the workspace instead of moving into the centered composer area, including Tauri's WebKitGTK renderer where an inline positioned wrapper previously made the header follow the centered chat column.
- Rebuilt the desktop workspace picker as a full-width project-context control with the selected folder name, clear Choose/Change action, long-path tooltip, keyboard focus treatment, and light/dark theme support.
- Preserved the native folder picker and existing workspace-switch behavior while making the control visible in both empty and active chats.
- Constrained the desktop chat, workspace context, and composer to a centered 1100px content rail with responsive side gutters, preventing active conversations from touching the workspace edges.
- Kept active-chat headers aligned with the New Chat header by pinning the chat wrapper to the first workspace grid row when the Live2D model is hidden.
- Made the hidden-Live2D desktop chat wrapper a full-height layer anchored directly to the workspace bounds, giving Active Chat and New Chat one stable containing block so the composer remains visible and the message area fills the window consistently in WebKitGTK.
- Anchored the New Chat welcome/composer group above the footer instead of relying on WebKitGTK's inconsistent percentage-based vertical centering, keeping the complete composer visible at desktop window heights.

## Live Thought Streaming (CLI, Desktop & Web)

- Agent reasoning now appears incrementally while supported models are still thinking, instead of waiting for the entire provider response to finish.
- Added a provider-neutral reasoning stream that preserves native tool calls, token usage, stop reasons, Gemini thought signatures, and `<think>` blocks split across network chunks.
- Desktop and Web merge deltas into one live Thought card, throttle rendering to 75ms, show elapsed time and a streaming cursor, and persist only the completed Thought rather than every transport chunk.
- CLI updates the `Ctrl+T` Thought viewer live while keeping terminal scrollback compact, then finalizes the existing record without duplicating it.
- Providers that do not emit reasoning deltas retain the previous completed-Thought fallback behavior.

## Light Mint Theme Contrast & Clarity Overhaul (Desktop & Web UI)

Refined the **Light Mint** theme palette across Desktop and Web to eliminate blinding flat white surfaces and washed-out elements:
- **Soft Slate Canvas (`#f0f2f5`) & Crisp White Cards**: Replaced the glaring near-white canvas (`#f8fafc`) with a soothing soft slate tone, creating immediate card elevation and depth between the canvas, sidebar, floating header, and chat input.
- **High-Contrast Slate Typography**: Upgraded text tokens from flat black and washed-out grays to a rich slate scale (`--text-primary: #0f172a`, `--text-secondary: #334155`, `--text-muted: #475569`, `--placeholder: #64748b`), ensuring crisp, effortless readability for all labels, placeholders, and subtitles.
- **Clearly Defined Component Boundaries**: Boosted border definition (`--border-subtle: rgba(15, 23, 42, 0.08)`, `--border-default: rgba(15, 23, 42, 0.14)`) so the sidebar divider, input card, and header are distinct.
- **Input & Control Polish**: Replaced hardcoded dark-mode white alpha values in the model selector pill, smart context bar labels, toggle switch tracks, and tool action buttons with adaptive semantic tokens for clean light mode contrast.
- **Settings Modal & Cards Overhaul**:
  - **Zero Background Bleed-Through**: Rendered the modal container with a solid pure-white card and soft elevation drop shadow over a darkened blur scrim backdrop, preventing underlying chat text from bleeding through.
  - **Unified & Polished Card System**: Eliminated mismatched muddy-slate backgrounds on Temperature and Thinking cards; all setting cards now share consistent, crisp `#f8fafc` soft-slate containers with refined slate borders (`1px solid rgba(15, 23, 42, 0.1)`).
  - **Elevated Segmented Controls (`pill-segmented`)**: Upgraded provider selection and reasoning effort pills to modern iOS/macOS-style segmented controls featuring an elevated crisp white pill for the active selection and clear slate text for inactive options.
  - **Readable Search & Navigation**: Upgraded the settings sidebar with high-contrast group labels, clean search input styling, and active tab highlights featuring mint accent icons.
  - **Model Combobox Dropdown**: Restyled the searchable model picker in light mode with crisp white backgrounds, subtle slate borders, and readable search input.

## Interactive Profile Photo Crop & Rotate Modal (Desktop & Web UI)

Added an interactive image editor modal when updating the user profile photo:
- **Pan & Position Adjustment with Edge Clamping**: Allows drag-to-reposition with mouse and touch gestures to frame the exact part of the image desired, with intelligent boundary constraints that prevent dragging past the edges of the image (eliminating empty black margins).
- **Smooth Zoom Control**: Intuitive zoom slider (minimum zoom locked to 1.0 to prevent gaps) with step buttons, mouse wheel zooming, and 2-finger pinch-to-zoom on touch screens.
- **90° Stepped Rotation**: Instant rotate button with clean icon to orient sideways or inverted photos correctly, automatically re-clamping coordinates.
- **Visual Circular Avatar Framing & Corner Brackets**: Renders a circular vignette guide with white corner brackets (`┌ ┐ └ ┘`) matching modern photo crop standards.
- **High-Resolution Canvas Export**: Automatically processes and renders the transformed image through an HTML5 canvas at 512×512 resolution before saving.

## Floating Glass Capsule Chat Header (Desktop & Web UI)

Redesigned the top chat header across both Desktop and Web interfaces into a modern, aesthetic floating glass capsule (Island/Capsule design):
- **Floating Island Geometry & Glassmorphism**: Transformed the previously disconnected, flat rectangular header into an elegant floating capsule (`backdrop-filter: blur(20px) saturate(180%)`, rounded corners `border-radius: 14px`, sleek inner/outer border glows, and balanced top padding).
- **Contextual Conversation Title**: Displays the current conversation topic/title dynamically instead of an empty, redundant "Mint Agent" title, with automatic truncation and tooltip for long titles.
- **Clean Minimalist Balance**: Kept the floating capsule minimal and focused, avoiding artificial AI status badges to keep the conversation space calm and distraction-free.
- **Polished Glass Action Buttons**: Redesigned the clear conversation button and preview toggles with smooth micro-interactions, subtle glass backgrounds, and clean hover feedback.
- **Mobile Responsive Full-Width Integration**: Automatically transitions gracefully into a flush top navigation bar on screens under 760px without edge clipping.

## Unified Agent Loop Timeline & Unboxed Chat Stream (CLI, Desktop & Web UI)

Overhauled the agent execution flow and timeline UI/UX across all 3 platforms (**CLI**, **Desktop UI**, and **Web UI**), aligning with modern developer assistant standards (Codex Desktop & Antigravity/Claude style):

- **Unboxed Conversational Flow (Codex Desktop Standard)**:
  - **Removed Monolithic Bounding Container**: Eliminated the rigid dark bounding box around `.agent-activity-list` (`border: none; background: transparent; overflow: visible;`).
  - **Natural Unboxed Chat Text (`InlineThoughtNote`)**: Intermediate narration and speech spoken by the agent prior to tool calls (e.g. *"มิ้นจะลองดูไฟล์ CONTRIBUTING.md ให้ก่อนนะคะ ว่าเจอตรงไหน แล้วค่อยสรุปให้ฟังค่ะ"*) now flows directly as **normal, unboxed conversation text** in standard font size (`0.92rem`) and natural text color, completely freed from tool boxes.
  - **Standalone Collapsible Thought Box (`[Brain] Thought >`)**: Deep model reasoning (`extendedThinking` / Chain-of-Thought) is isolated into its own sleek, standalone collapsible card (`.agent-activity-thought-step`) with subtle borders, rather than trapping the entire conversation.
  - **Independent Compact Tool Rows**: Each tool call (e.g. `read_file`, `search_code`, `execute_command`) renders as an individual compact pill/row with smooth collapsible input/output drawers.
- **Deduplication & Automatic Greeting Cleanup**:
  - **Smart Greeting Stripping (`cleanIntermediateThought` & `strip_intermediate_greeting`)**: Automatically detects and strips repetitive conversational greetings (e.g. *"สวัสดีค่ะพี่ภีม 🌿"*, *"Hello..."*) from intermediate progress notes across Web/Desktop frontend (`agentActivity.ts`) and CLI live status (`crates/mint-cli/src/agent/live_status.rs`).
  - **Prompt Guidance (`crates/mint-core/src/prompts/agent.rs`)**: Updated system instructions so models jump directly to describing the technical action during intermediate turns without repeating opening greetings.
  - **Eliminated Redundant "Step" Badges**: Completely removed clutter labels like `"Step 1"`, `"(1 steps)"`, and `"{count} steps"` across `ThinkingBlock.tsx`, `AgentActivityTable.tsx`, and `agentActivity.ts`.
  - **Fixed Duplicate Thought Boxes**: Removed the redundant external `ThinkingBlock` wrapper in `ChatMessageItem.tsx` that previously caused double thought boxes.
- **Natural Multi-Language Reasoning (Chain-of-Thought)**:
  - Removed obsolete English-only restrictions in JSON fallback mode (`crates/mint-core/src/prompts/agent.rs`), allowing models to think naturally in their preferred reasoning language (e.g., DeepSeek/Claude English CoT) while keeping all user-facing communication in natural Thai.
- **Strict Platform Parity Across CLI, Desktop, and Web**:
  - **CLI (`crates/mint-cli`)**: Streamlined terminal timeline with clean progress narration, greeting stripping, and collapsed reasoning (`  • Thought for Xs (Ctrl+T to view)`).
  - **Desktop UI (`src/renderer/src`)**: Unboxed conversational stream, standalone thought boxes, and compact tool pills in `src/renderer/src/css/chat.css`.
  - **Web UI (`src/renderer/src-web`)**: 100% visual and functional parity matching Desktop UI in `src/renderer/src-web/css/chat.css`.

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
