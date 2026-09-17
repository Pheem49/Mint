# Release Notes - Mint Agent v1.15.0

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
  - Accumulates thoughts across all steps in a multi-step agent loop, allowing `Ctrl+T` to review the entire reasoning journey.


