# Release Notes - Mint Agent v1.18.0

## Code Block UI & Copy Ergonomics (CLI, Desktop, and Web)
- **Frictionless CLI Code Copying**: Removed the vertical gutter bar (`│ `) from CLI code blocks, replacing it with 2-space clean indentation. Terminal mouse selection or double-clicking commands now copies pure, runnable code without grabbing border characters or breaking shell commands (`bash: │: command not found`).
- **Smart Line Numbers on Desktop & Web**: Automatically hides line numbers for short commands and snippets ($\le 5$ lines), eliminating unnecessary 44px gutter clutter. Added an optional `#` toggle button for longer code blocks.
- **Enhanced Code Typography**: Increased code font size from `13px` to `14.5px` with a generous `1.65` line height and Thai font fallbacks across Desktop and Web, making code comments with Thai tone marks legible and comfortable to read.

## Structure-Aware Progressive AI Response Rendering (CLI, Desktop, and Web)
- **Line-by-Line + Block-Aware Streaming Engine**: Replaced all-at-once response dumps with a deep, structure-aware progressive reveal engine across Full-Screen TUI, Classic CLI, Desktop UI, and Web UI.
- **Smooth Line-by-Line Reveal**: Regular paragraphs, headings, lists, blockquotes/callouts (`> [!NOTE]` etc.), and standard syntax-highlighted code blocks stream and reveal smoothly line by line with adaptive catch-up pacing.
- **Atomic Block Reveal for Tables & Custom UI Cards**: Markdown tables (`| ... |`) and structured UI/JSON/Mermaid fenced blocks (`ui-grid`, `ui-card`, `ui-feature`, `ui-mockup`, `weather-json`, `stock-json`, `calculation-json`, `image-search-json`, `image-gen-json`, `mermaid`) are held in buffer while incomplete and released as a single atomic unit once complete, preventing broken table borders or raw JSON flashes mid-stream.
- **Full-Screen TUI & Viewport Ergonomics**: Preserves user scroll position (`scroll_from_bottom`) during live streaming, keeps mid-turn notices (`Thought · ...`) ordered above the active assistant message, defers post-turn notices (`Sources`, `Worked for ...`) until the reveal completes, and allows instant reveal completion via `Esc` / `Ctrl+C`.

## MCP Tool Execution UI Consolidation (Desktop and Web)
- **Eliminated Raw Execution Dumps from Chat Messages**: Suppressed raw unstyled MCP call logs (`Failed`, `Completed`, and internal `Tool definition ready` discovery lines) from leaking into the main chat message bubble.
- **Consolidated into Agent Activity Accordion**: Integrated detailed MCP tool invocations, statuses, outputs, and artifacts strictly inside the `Executed X mcp_tool` group accordion in `AgentActivityTable`, keeping the chat conversation surface completely clean and focused on user queries and synthesized AI responses.
- **Artifacts-Only Bubble Passthrough**: Allowed downloadable artifacts (images, files) and critical warning banners to pass through safely without polluting the interface with intermediate debug text.

## Tool & Environment Setup Wizard Redesign (`mint setup`)
- **Full-Screen Ratatui TUI Modernization**: Replaced the legacy raw ANSI script and aggressive clear-screen flickering in `setup.rs` with a smooth, double-buffered Ratatui Full-Screen TUI wizard adhering to the workspace rule.
- **Categorized 49 Agent Tools**: Grouped all 49 agent tools into 7 clear categories (Files & Workspace, Code & Repository, Search & Knowledge, Productivity & Utilities, Planning & Safety, Execution & MCP, and Media & Audio/Video) with auto-scrolling viewports and real-time enabled counts.
- **High-Contrast Monochrome Theme Parity**: Integrated the Solid Gray Bar highlight (`bg: Gray, fg: Black`), pill badges (`[ SELECTING ]`), and transparent category headers matching `mint onboard` and `resume_picker`.
- **Three-Step Stepper Flow**: Structured configuration across 3 distinct steps (`1. Agent Tools`, `2. Native Plugins`, `3. Launch Target`), giving users full control over tool policy, host plugin access, and subsequent launch targets (CLI, Web, Desktop, or Save & Exit).
- **MCP CLI Ergonomics**: Added short flags `-a` for `--args` and `-e` for `--env` in `mint mcp add` for convenient and quick server registration.

## Onboard Setup Wizard UI/UX High-Contrast Grey Redesign (`mint onboard`)
- **High-Contrast Monochrome Theme**: Unified the Setup Wizard styling with Mint CLI's `resume_picker` design system using `SELECTION_BACKGROUND` and `SELECTION_TEXT` to ensure clear visibility across transparent terminals and custom wallpaper backgrounds.
- **Solid Gray Bar Row Highlights**: In active model choice lists (Step 1 & Step 3) and the services checklist (Step 2), the focused row now receives a full-width solid grey highlight bar (`bg: Gray, fg: Black`) that instantly stands out without color ambiguity.
- **Active Text Cursor (`█`)**: Input boxes (Gemini API Key, custom model identifiers, and service config inputs) now render a block cursor (`█`) at the insertion point when focused, and hide the cursor when unfocused.
- **Explicit Focus Badges**: Box titles now feature distinct pill badges: `[ EDITING ]` for active input fields and `[ SELECTING ]` for active choice lists, with dimmed titles and markers (`› (selected)`) when unfocused.
- **Context-Aware Action Buttons & Footer**: Replaced static green action buttons with context-aware buttons (e.g. `[ Enter / Tab: Next Field ➔ ]` vs `[ Enter: Continue to Services ➔ ]`) and dynamic footer shortcuts that reflect the currently focused control.

## Headless Browser Automation & Anti-Loop Engine (CLI, Desktop, and Web)
- **Headless Chromium via Shell Playbook**: Replaced agent-facing `browser_*` tools and the headed Mint Browser UI (`mint auto`, `BrowserShell.tsx`, `BrowserSuggestionsPopup.tsx`) with high-reliability headless Chromium shell commands (`google-chrome --headless=new --dump-dom`, `--screenshot`) managed through `run_shell`.
- **System Default Browser Previews**: HTML artifact previews open directly in the user's default system browser across CLI (Full-Screen TUI / Classic), Desktop, and Web, eliminating custom embedded browser windows.
- **Anti-Loop Repetition Guard & SHA-256 Image Dedup**:
  - Multi-layered repetition guard in `mint-core` tracks execution outputs over a rolling 8-execution window, intercepting stuck loops ($\ge 3$ identical results) with actionable `InterceptStuckLoop` guidance.
  - Image artifact viewer computes SHA-256 hashes on `view_image` calls, suppressing repeated analysis of identical screenshots and steering the agent toward forward progress.
- **Deterministic Headless Workflow Runner (`mint browser run`)**: Streamlined workflow action names to concise verbs (`open`, `click`, `type`, `fill`, `screenshot`, `wait`, `tabs`, etc.) with full backward compatibility for legacy `browser_*` aliases. Retained the 14-action deterministic browser workflow engine for scripted multi-step automation and testing, strictly headless with per-run temporary user profiles and artifacts stored under `.mint/browser-runs/`.
- **Diagnostic Slash Command (`/browser` & `/browser status`)**: Streamlined `/browser` across CLI TUI, Desktop, and Web to report detected Chromium binary paths, version info, and sandbox readiness.
- **Information Sufficiency & Follow-Up Search Gate**: Prevented over-agentic crawling and runaway token consumption by enforcing an Information Sufficiency rule on web searches (stopping and synthesizing immediately once target links/answers are found), requiring interactive user confirmation via `ask_user` before conducting follow-up searches on incomplete topics, and restricting headless Chrome strictly to explicit user requests or unblocked direct page reading.
- **Clean Architecture Sweep**: Fully removed `mint auto` CLI command, headed Tauri browser capabilities, port 9222 takeover logic, and legacy browser-mode toggles across Desktop and Web settings.
- **Browser Subsystem Decoupling & Dead Code Pruning**: Removed zombie `run_session` wrappers from the agent loop and Gemini Live, pruned 12 dead public exports from `mint_core::lib`, removed obsolete multi-process tab lease lockfiles (`TabLease`, `.mint-browser-leases`), decoupled Tauri's background task queue from legacy CDP actions, and updated Desktop titlebar navigation to 'Open in default browser'.
- **Search Sources Popover Ergonomics**: Improved domain source popover positioning with dynamic viewport detection, scroll/resize tracking, and containment within message bubbles across Desktop and Web.
- **Error Recovery Preview Route (`#/error-preview`)**: Added a dedicated preview route across Desktop and Web to inspect the live error recovery interface anytime with seamless full-viewport centering and uniform background styling without triggering an actual runtime crash.

## Shell Execution & Background Job Promotion Safeguards
- **Background Shell Status Reporting**: When synchronous `run_shell` commands exceed 180s and are promoted to background jobs, the output status is now explicitly formatted as `exit: running_in_background (job_id: <job_id>)` instead of ambiguous `exit: unknown`.
- **System Guard & Anti-Retry Notice**: Injected an explicit `[SYSTEM NOTICE]` into the tool output alerting the agent that the process is actively running in the background and strictly prohibiting redundant re-runs or alternative scripts.
- **Agent System Prompt Rules**: Added rule 12b and updated execution evidence guidelines in `agent.rs` to prevent retry loops on long-running tasks and enforce `shell_output` monitoring.
- **TUI Tool Activity Status Parity**: Updated Full-Screen TUI's `ToolActivity` in `mint-cli` to recognize `running_in_background` as active rather than misclassifying it as a failure.

## 🚀 Architecture Evolution & Future Roadmap
- **Retirement of Legacy Mint Browser**: We have officially decommissioned the legacy headed Mint Browser subsystem (including the `mint auto` CLI command, CDP port 9222 takeover, and custom embedded browser window frames in Desktop and Web).
- **Modern Headless Browser Foundation**: In its place, Mint v1.18.0 establishes a clean, decoupled **Headless Browser** architecture powered by deterministic shell automation and the headless workflow runner (`mint browser run`). Web previews and generated HTML artifacts now open cleanly in your system default browser.
- **Future Vision**: This headless transformation eliminates UI bloat and execution instability while establishing a rock-solid, production-grade foundation. In future releases, we plan to extend and build upon this headless foundation to develop a reimagined, vastly superior Mint Browser.
