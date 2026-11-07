# Release Notes - Mint Agent v1.18.0

## Compact Chat Input & Toolbar Mode Selector (Desktop and Web)
- **Compact All-in-One Composer**: Completely redesigned the chat input area across Desktop and Web to reduce vertical idle footprint by over 45% (down from ~180px to ~90px), expanding visible chat viewport space so users can read significantly more conversation history without cramped viewports.
- **Slim Integrated Project Header**: Retained the Workspace (`📁 Mint-CLI ▾`) and Git Branch (`🌿 Rust ▾`) selectors at the top of the composer as a slim, unified 34px header that seamlessly connects to the chat input container without vertical bloat.
- **Eliminated Bulky Toggle Bar**: Removed the standalone middle `smart-context-bar` that previously consumed an extra ~35px of vertical screen real estate for two toggle switches.
- **Toolbar Mode Selector Pill (`ChatModeSelector`)**: Moved interaction mode switching down into the bottom action toolbar alongside the Model Selector as a modern pill (`[⚡ Agent ▾]` / `[📋 Plan ▾]` / `[💬 Chat ▾]`). Styled with clean, subdued text colors (without heavy green accent outlines) and positioned on the right side of the toolbar adjacent to voice and send controls. Clicking the pill opens a floating popover menu allowing users to switch between Agent mode, Plan mode (with read-only approval gating), and standard Chat mode.
- **Harmonized Git Branch Selector & Popover**: Redesigned `GitBranchSelector` across Desktop and Web to achieve complete visual consistency with `WorkspaceSelector` and `ChatModeSelector`:
  - Unified the trigger button in `.project-context-row` with identical pill styling (`var(--surface-strong)` background, `1px solid var(--border-subtle)`, rounded corners, and smooth hover states).
  - Replaced legacy raw Unicode glyphs (`⑂`, `⇄`, `＋`, `←`) with standard Lucide SVGs (clean Git branch, remote globe, plus, git graph, and chevron back icons).
  - Upgraded the popover dialog with glassmorphism backdrop blur, modern 16px corner radius (`var(--radius-xl)`), deep elevation shadow, uppercase category headers, rounded item hover pills, styled track badges, and a sleek search bar.
  - Styled search input focus and active branch selection with refined neutral gray borders and backgrounds instead of bright green accents.
- **Single-Line Initial Textarea**: Reduced initial textarea `min-height` to a clean single line (26px) with streamlined padding, while preserving smooth multi-line auto-expansion up to 140px+ as prompts grow.
- **Full Platform Parity**: Implemented across Desktop (`src/renderer/src/css/chat.css`) and Web (`src/renderer/src-web/css/chat.css`, `mobile.css`), backed by unit tests (`ChatModeSelector.test.tsx`, `ChatPanel.test.tsx`) and full launcher tests.

## Collapsible Document Attachment Cards & Rich File Mentions (CLI, Desktop, and Web)
- **Collapsible Attached Document Card in User Messages**: Dropping text, markdown, or code documents into the chat composer no longer dumps hundreds of lines of raw text and backend prompt instructions into the user message bubble. Instead, the user bubble cleanly separates authentic prompt text from the attachment, rendering a sleek, collapsed **Attached Document Card** with genuine language/file icons, line counts, and a smooth `[View content ▾]` accordion viewer with one-click copy.
- **Rich File Mention Chips in Message Stream**: Mentions referencing files (e.g. `@generate_harness_pdf.py` or `@src/app.tsx`) in the message stream now render as clean, modern file tag chips with dedicated Material Icon Theme language icons (Python, TypeScript, Rust, JSON, etc.) and stripped `@` prefixes (matching Cursor / VS Code style), while non-file mentions (e.g. `@web`, `@agent`) preserve their standard badge formatting.
- **Full-Width Artifact & Document Stretch**: Fixed artifact cards (PDF / PPTX presentations) and tool result containers to stretch across the full chat column width, matching `TurnFileChanges` without shrink-wrapping to the reply text width.
- **TUI Cleanliness & Parity**: Full-Screen TUI (`mint-cli`) automatically condenses document attachments in the transcript to single-line notices (`[Attached document: <filename> (<N> lines)]`) and pre-fills `/edit` and `/retry` drafts with clean prompt text, preventing giant file dumps in terminal buffers.
- **Full Platform Parity**: Maintained 100% parity across Desktop UI (`src/renderer/src`), Web UI (`src/renderer/src-web`), and CLI (`crates/mint-cli`), backed by automated Vitest and Cargo test suites.

## Modern Developer-Centric Activity Timeline (CLI, Desktop, and Web)
- **Developer-Centric Semantic Timeline**: Replaced the rigid boxed table layout in `AgentActivityTable` with a sleek, borderless semantic timeline inspired by modern AI coding assistants (Claude Code / Cursor / Copilot).
- **Elapsed Duration & Natural Action Header**: The activity drawer header now displays dynamic, human-readable execution status (e.g. `Worked for 2m 14s ⌄` or `Working...`) alongside an aggregated action summary (e.g. `Explored files, ran commands, edited files`), giving users immediate high-level context of agent work at a glance.
- **Semantic Verbs & File Targets**: Activity rows present clean developer verbs and context:
  - File Reads: `Analyzed <icon> filename.tsx#L12-45` with targeted line ranges.
  - File Edits: `Edited <icon> filename.tsx` paired with precise Git diff statistics (`+24` / `-8`).
  - Terminal Commands: `Ran <intent or command> >` with an expandable chevron to toggle output view.
  - Thoughts: `Thought for 12s >` with collapsible reasoning details.
- **Accurate Diff Extraction**: Extended `agentProgress.ts` to calculate real additions and deletions for `edit_file`, `write_file`, and `apply_patch` operations, displaying color-coded green `+N` and red `-N` badges in the timeline.
- **Dedicated Language & File Icons**: Expanded the centralized Material Icon Theme system (`fileIcons.ts`) with authentic file icons for Python (`.py`, `.ipynb`), Rust (`.rs`), Go, Java, C/C++, C#, TypeScript, JavaScript, Shell/Bash (`.sh`, `.bash`, `.zsh`), SQL/Databases, Dockerfiles, Git, Markdown, PDFs, and more. Both `AgentActivityTable` and `TurnFileChanges` now display dedicated, genuine language icons next to modified/analyzed file names instead of generic placeholders.
- **Preserved Productivity Interactions**: Retained full interactive capabilities including HTML5 file drag-and-drop from timeline rows into the chat composer, `@` quick-reference button on hover, and seamless subagent DAG view switching.
- **CLI Full-Screen TUI Parity**: Enhanced `interactive::tool_activity` in `mint-cli` to use developer-centric semantic verbs (`Ran`, `Searched`, `Analyzed`, `Edited`) and prefer human-readable intent summaries for shell executions.
- **Full Platform Parity**: Maintained 100% parity across Desktop UI (`src/renderer/src`), Web UI (`src/renderer/src-web`), and CLI (`crates/mint-cli`), backed by automated tests across Vitest and Cargo test suites.

## Inline Message Edit & Save & Resend (CLI, Desktop, and Web)
- **In-Place Inline Message Editor**: Clicking the ✏️ Edit button on any user message now transforms the message bubble into an inline auto-growing editor directly in context, eliminating the disjointed jump down to the composer.
- **Save & Resend Truncation**: Saving the edited message automatically truncates conversation history from that turn onwards in SQLite (`truncate_interactions_from`), clearing subsequent turns and triggering the model to answer the refined prompt with a completely clean context.
- **Keyboard Shortcuts & Quick Actions**: Equipped with `Esc` to instantly cancel and restore original text, `Ctrl+Enter` / `Cmd+Enter` to save and resend, auto-focus, and cursor placement at the end of the text.
- **Full-Screen TUI CLI Commands (`/edit` & `/retry`)**: In the Full-Screen TUI, users can enter `/edit` or `/retry` to pull the latest turn prompt directly into the active composer under an explicit notice banner `[Mode: Edit Turn #N — Resending will replace this turn and clear its previous response | Esc to cancel]`. Pressing `Enter` truncates the turn and subsequent responses from SQLite and immediately resends the updated prompt, while `Esc` or `Ctrl+C` cancels edit mode and restores the composer.
- **Unified Platform Bridge**: Implemented cross-platform endpoints via Tauri command `truncate_interactions_from` on Desktop, `POST /api/interactions/truncate` on Web, and reactive state management in `lifecycle.ts`.
- **Full Platform Parity**: Maintained across Desktop UI (`src/renderer/src`), Web UI (`src/renderer/src-web`), and Full-Screen TUI CLI (`crates/mint-cli`), backed by automated tests across Vitest and Cargo.

## Drag & Drop File Changes & In-Composer Mentions (CLI, Desktop, and Web)
- **Draggable Turn File Changes**: In `TurnFileChanges`, each created or modified file item is now fully draggable using standard HTML5 drag-and-drop. Users can drag files directly from the turn summary card (e.g. `generate_harness_pdf.py` or `CalculationCard.tsx`) into the chat composer.
- **Sleek Full-Width File Changes Bar**: Redesigned `TurnFileChanges` into a full-width, streamlined summary bar matching modern AI coding assistants (Claude Code / Cursor). Features a compact single-line collapsed header showing `N files changed +add -del >` on the left and a `[ 📄 Review ]` quick action button on the right, with smooth click-to-expand file details and full chat-column width stretch across Desktop and Web.
- **Live Tool Execution Dragging**: In `AgentActivityTable`, target files within active and historical tool rows (`write_file`, `edit_file`, `read_file`) are now draggable into the composer, letting users grab files as soon as the agent begins writing or modifying them.
- **Click-to-Reference Quick Action (`@`)**: Added a convenient "Reference in chat" button on hover for both file change items and activity tool rows, enabling laptop and trackpad users to instantly bind files into the prompt without dragging.
- **Direct In-Input Insertion**: Dragging a file or clicking `@` inserts `@relative/path ` directly into the text input box (`<textarea id="chat-input">`) at the cursor position and smoothly focuses it, keeping prompt drafting intuitive and fully editable inline without redundant chip containers.
- **CLI Full-Screen TUI Parity**: Upgraded `collect_mention_candidates_with_recents` in `crates/mint-cli` to detect recently modified session files (and active git working tree diffs) and prioritize them at the very top of `@` mention auto-completions with the `Recently edited in this session` badge.
- **Full Platform Parity**: Maintained 100% parity across Desktop UI (`src/renderer/src`), Web UI (`src/renderer/src-web`), and CLI (`crates/mint-cli`), backed by automated tests across Vitest and Cargo test suites.

## Modern AI Fluid Border Radii (Desktop and Web)
- **Contemporary Border Radii Tokens**: Upgraded the design token scale in `tokens.css` to the Modern AI aesthetic (`--radius-xs: 6px`, `--radius-sm: 8px`, `--radius-md: 10px`, `--radius-lg: 14px`, `--radius-xl: 18px`, `--radius-2xl: 22px`, `--radius-3xl: 26px`, `--radius-composer: 24px`), delivering fluid, approachable corners across cards, buttons, dialogs, and panels.
- **Fluid Chat Composer & Prompt Bar**: Elevated the chat form (`#chat-form`), `.smart-context-bar`, and `.project-context-row` to a smooth 24px radius (`var(--radius-composer, 24px)`) across Desktop UI (`src/css/chat.css`), Web UI (`src-web/css/chat.css`), and Mobile (`src-web/css/mobile.css`).
- **Floating Island Capsule Header & Controls**: Styled the floating island `.chat-header` with modern capsule curvature (`border-radius: var(--radius-full, 9999px)`), transforming breadcrumb `< Code` buttons, session tags (`cli::...`), `Tools` toggles, and action buttons into cohesive rounded pills across Desktop and Web.
- **Harmonious Model Spotlight & Popover Surfaces**: Updated model spotlight popovers and modal selector dialogs to use `var(--radius-2xl, 22px)`, providing seamless continuity with the new card and container curves.
- **100% Elimination of Hardcoded Border Radii (Single Source of Truth)**: Completed a repo-wide audit across all UI components and stylesheets. Migrated every remaining hardcoded pixel border radius in Markdown renderers (`markdown.tsx`), rich interactive cards (`ImageSearchCard.tsx`, `ImageGenCard.tsx`, `StockCard.tsx`, `MermaidCard.tsx`, `CalculationCard.tsx`, `WeatherCard.tsx`), modals & overlays (`CommandPalette.tsx`, `SubagentDagView.tsx`, `AgentActivityDrawer.tsx`, `MintDashboard.tsx`, `ModalErrorBoundary.tsx`, `ChunkErrorBoundary.tsx`, `WorkspacePdfPreview.tsx`), and stylesheets (`code-review.css`, `model-selector.css`, `background-terminals.css`, `library.css`, `slash-choice.css`, `queued-messages.css`, `veo-studio.css`, `desktop-tools.css`, `tool-surfaces.css`, `mobile.css`, and Settings stylesheets) to centralized Design Tokens (`var(--radius-*)` with `--radius-2xs: 4px`), guaranteeing full theme-driven consistency with zero hardcoded drift.
- **Tailwind Utility Synchronization**: Registered semantic radius tokens directly in `ui-tailwind.css` `@theme inline` block ensuring consistent styling for shared components.

## Text Color Customization & Modern Studio Design System (Desktop and Web)
- **Centralized Text Color Customization & Palette Swatches**: Added a centralized Text Color system in `ThemeTab` with 6 high-contrast curated swatches (Pure White `#ffffff`, Cyber Green `#86efac`, Electric Cyan `#67e8f9`, Retro Amber `#fde047`, Pastel Lilac `#d8b4fe`, Soft Coral `#fda4af`) and custom hex picker with native eyedropper support.
- **Deep Theme Variable Distribution**: Updated `themeManager.ts` to distribute `--text-main`, `--text-chat`, and `--text-primary` from a single source of truth, deriving `--text-soft`, `--text-secondary`, and `--text-muted` dynamically via `color-mix`, while preserving custom text colors across preset theme switches.
- **Modern Studio Design Tokens & Border Radii**: Refined base radii in `tokens.css` to a crisp, professional Studio scale (`--radius-xs: 4px`, `--radius-sm: 6px`, `--radius-md: 8px`, `--radius-lg: 10px`, `--radius-xl: 12px`, `--radius-2xl: 14px`, `--radius-3xl: 16px`), aligning the entire UI with Linear / Raycast developer ergonomics.
- **Subtle User Message Bubbles & Input Form**: Replaced exaggerated 22px bubbles with a refined 12px bubble (`border-radius: var(--radius-xl) var(--radius-xl) var(--radius-xs, 4px) var(--radius-xl)`), and standardized `#chat-form`, slash command popups, and UI cards to tokenized radii across Desktop and Web.
- **Code Hub Subtle Studio Refinements**: Redesigned `/code` session rows with neutral raised background (`var(--bg-surface-raised)`), clean left accent borders (`border-left: 2px solid var(--accent)`), subtle active badges with glowing accent status dots, muted prompt symbols, and polished `$ resume` chips.
- **Tools Panel & Session Breadcrumbs Polish**: Toned down excessive green accent styling in Tools tab launcher icons to neutral text tokens, refined active dropdown items to neutral surfaces, and styled the breadcrumb session badge (`cli::<id>`) to match the `< Code` button aesthetic.

## Drag & Drop Support for Side Panel Tools & Artifacts (Desktop and Web)
- **Automatic Image Attachment on Drag**: Dragging image files or image preview tabs from Tools / Side Panel now automatically resolves the image binary/dataUri and attaches it as an interactive Image Attachment chip (complete with trimmed thumbnail preview and removal button) directly above the composer, feeding vision data straight into multimodal LLMs without requiring extra read turns.
- **Side Panel Tools Tab Dragging**: Enabled HTML5 dragging on preview tabs within `ToolSurfacePage` (and the tabs switcher dropdown menu). Users can now directly drag tabs (such as generated slides, documents, images) from the Tools panel on the right and drop them into the chat input composer to mention `@relative/path`.
- **Artifact Preview Header Dragging**: Made the file icon and title container in `ArtifactPreviewPanel` draggable with grab cursors and informative tooltips (`title="<path> (Drag to chat)"`), allowing quick reference dragging directly from the artifact viewer header.
- **Chat Artifact Cards Dragging**: Enabled dragging on `.chat-artifact-card` items directly in the message stream, allowing users to drag inline presentation/PDF/document cards into the input field.
- **Deep Seam & Unified Drag Utility (`workspaceDrag.ts`)**: Built deep helpers `setWorkspaceDragData` and `formatWorkspaceReference` under `src/renderer/shared/utils/workspaceDrag.ts` unifying drag data transfer across `WorkspacePanel`, `ToolSurfacePage`, `ArtifactPreviewPanel`, and `ArtifactCards`, setting both `application/x-mint-workspace-path`, `application/x-mint-workspace-file`, and fallback `text/plain`.
- **Robust Drop & Dragover Ergonomics in Chat**: Upgraded `ChatPanel` and `CollapsiblePasteInput` to handle dragover seamlessly with copy cursors and drop handlers across the input textarea, chat form, and the main conversation area.
- **English Slide Pagination in Workspace Preview**: Updated slide indicator and page labels in `WorkspacePdfPreview` from Thai to standard English (`Slide {pageNumber} / {pdf.numPages}` and `Slide {pageNumber} of {pdf.numPages}`).

## Context Compaction & Observation Compression Harness (CLI, Desktop, and Web)
- **File-Backed Output Scratchpad (Cursor / Claude Code Pattern)**: Automatically offloads bulky tool execution logs (> 3,000 characters) to `.mint/scratch/tool_outputs/<timestamp>_<action>_<hash>.log`, presenting the model with clean head/tail previews and an actionable pointer path to prevent token bloat while keeping full raw outputs on-demand.
- **Progressive Observation Ghosting (Three-Tier Aging)**: Implemented deterministic turn-level aging for historic `ToolResult` messages without LLM invocation cost — retaining recent 3 turns in full (Hot Zone), truncating turns 4–10 to concise summaries (Warm Zone), and collapsing older turns (> 10 turns back) into 1-line tombstones (Cold Zone).
- **Semantic Tool Superseding**: Automatically detects file modifications (`apply_code_edits`, `write_file`, `patch_file`, `delete_file`) and replaces earlier `read_file` results targeting those modified paths with compact superseding notices, eliminating stale code hallucinations and saving thousands of prompt tokens.
- **Trajectory Buffer Compaction**: Compacts observation previews for older trajectory items in `rebuild_observation`, keeping the task history bounded and responsive across 50–100+ step autonomous runs.
- **Structured Milestone Compaction**: Upgraded `compact_native_messages` with structured markdown sections (Core Intent, Files Modified & Validated, Key Decisions & Findings, Pending Actions) ensuring critical state and verified paths are never lost during macro compaction.

## Deep Module Architecture & God Files Refactoring (`mint-core`)
- **Chat Provider Seam Decomposition (`agent/chat/`)**: Decomposed the 4,651-line `chat.rs` monolith into a deep, modular structure under `crates/mint-core/src/agent/chat/`. Each provider now owns an isolated seam with its own payload construction, thought signature handling, and response decoding:
  - `gemini.rs` (518 lines): Google Gemini 1.5/2.0 native function calling, thinkingConfig, schema sanitization, and multimodal parts.
  - `anthropic.rs` (318 lines): Anthropic Messages API client, prompt caching breakpoints, and tool response parsers.
  - `openai.rs` (582 lines): OpenAI, OpenRouter, and DeepSeek client with reasoning tokens and audio attachments.
  - `ollama.rs` (321 lines): Ollama native messages and streaming parsers.
  - `custom.rs` (149 lines): Custom HTTP endpoints and Hugging Face adapters.
  - `streaming.rs` (466 lines): Server-Sent Events (SSE) accumulator and progressive thought/chunk streams.
  - `tests.rs` (1,403 lines): Isolated 58-test unit verification suite.
  - `mod.rs` (856 lines): High-level provider-agnostic coordinator, fallback dispatchers (`send_chat`, `stream_chat`), and public types.
- **Orchestration Seam Deconstruction (`orchestration/`)**: Reduced the 6,653-line orchestration god file down to 2,587 lines (a 61% reduction) by isolating discrete responsibilities into cohesive modules:
  - `types.rs` (646 lines): Centralized execution types including `TurnLease`, `TurnStartListener`, `OrchestrationError`, `AskUserOption`, `AgentApproval`, `AgentProgress`, `ToolExecutionRecord`, and `AgentResult`.
  - `decision_parsing.rs` (736 lines): Deep parsing for LLM output, tool call preparation, schema sanitization, and retry tracking.
  - `tools/dispatch.rs` (899 lines): Tool execution router, parallel read-only batch execution, code review gatekeeper, and result appending.
  - `tests.rs`, `mcp_outcome_tests.rs`, `full_skill_result_tests.rs` (2,011 lines): Dedicated unit and regression suites.
- **100% Backward Compatibility**: Maintained exact API and struct compatibility across all public and crate interfaces for CLI (`crates/mint-cli`), Desktop (`src/renderer/src`), and Web (`src/renderer/src-web`). Fully verified with all 58 chat tests, 180 orchestration tests, and 241 CLI tests passing.

## Library Grid & High-Resolution Thumbnails (Desktop and Web)
- **Comfortable Card Layout**: Increased default grid card width to 260px (`minmax(260px, 1fr)`) with an expanded 1380px max-width container, delivering a spacious 3–4 column layout with clear preview details.
- **Grid Density Switcher**: Added a toolbar toggle (`Normal` / `Large`) with localStorage persistence, enabling a 2–3 column mode for closer visual inspection without opening preview modals.
- **Enhanced 4:3 Aspect Ratio**: Upgraded thumbnail cards from 16:10 to 4:3 with face-aware framing (`object-position: center 20%`) to keep character art, cosplay shots, and portraits centered.
- **Crisp 800px Previews**: Upgraded backend thumbnail resolution from 360px to 800px with automatic background migration for razor-sharp rendering on HiDPI displays.
- **Responsive Sidebar Alignment**: Connected `.library-page` offsets to the shell's sidebar expanded/collapsed state with smooth 0.22s sliding transitions.

## Graceful Web Shutdown & CLI Cleanup (`mint web`)
- **Clean Ctrl+C Termination**: Added shutdown signal handling to `mint web` in `mint-cli`, terminating frontend processes cleanly without false error messages.
- **Warning Suppression**: Injected `VITE_CONFIG_NATIVE_IGNORE_WARNING=true` to silence upstream Vite CommonJS deprecation notices during web launch.

## Test Database Isolation & Centralized Config Routing
- **Isolated Test Data**: Configured `mint_config_dir()` to route automated test storage to dedicated temporary paths (`/tmp/mint-test/`), preventing test runs from contaminating user databases or sidebar histories.
- **Unified Path Resolution**: Centralized store routing for `mint-knowledge.sqlite`, `cron-jobs.json`, and `tasks.json` through `mint_config_dir()`.
- **Regression Cleanup**: Added explicit session and subagent teardown hooks in test runners.

## Interactive Saved Plans Picker & Viewer (`/plan list`) (CLI, Desktop, and Web)
- **Interactive TUI Plan Picker**: Running `/plan list` in Full-Screen TUI opens an interactive selector showing saved plans sorted by timestamp with instant title previews.
- **Inline Plan Card Render**: Selecting a plan renders a borderless `#404040` card into the transcript complete with task checkboxes (`[ ]` / `[✓]`) and step statuses.
- **Shortcuts & Autocompletion**: Added direct viewing via `/plan list <n>` and `/plan show <n>`, plus multi-word subcommand autocompletion (e.g. `/plan `, `/plugin `) in the TUI composer.

## Unified Plan Review & Inline Card UI (CLI, Desktop, and Web)
- **Framed Transcript Cards**: Replaced raw text dumps for `/release-notes` and plan reviews with continuous borderless cards matching the dark theme.
- **Compact Approval Modal**: Condensed the plan review modal to a concise approval prompt while presenting the full plan in the transcript with unblocked scrolling.
- **Session Resume Persistence**: Reconstructed formatted plan cards when resuming past conversations across CLI, Desktop, and Web.
- **Command Aliases**: Added flexible slash command matching for `/release-notes`, `/release_notes`, `/release-note`, and `/releasenotes`.

## Universal Thinking & Multi-Provider Reasoning (CLI, Desktop, and Web)
- **Universal Thinking Toggle**: Enabled Thinking controls and effort levels (`Low` to `Extra High`) across all chat models (Gemini, DeepSeek, OpenAI, Anthropic, OpenRouter, Ollama, and Local).
- **Native Parameter Dispatch**: Sends provider-accurate payloads including Gemini `thinkingConfig`, OpenAI/OpenRouter `reasoning_effort`, Anthropic `budget_tokens`, and DeepSeek V4 reasoning specifications.
- **Prompt-Based Fallback**: Automatically injects effort-scaled `<think>...</think>` directives for standard chat models without native reasoning APIs.
- **Non-Text Filter**: Automatically disables reasoning options for image, video, audio, and embedding models.

## Code Block UI & Terminal Copy Ergonomics (CLI, Desktop, and Web)
- **Frictionless CLI Code Copying**: Replaced vertical border gutters (`│ `) with clean 2-space indentation so terminal mouse selection copies pure, runnable commands.
- **Smart Line Numbers**: Automatically hides line numbers on short snippets ($\le 5$ lines) with an optional `#` toggle on Desktop and Web.
- **Typography Enhancements**: Increased code size to 14.5px with 1.65 line height and proper Thai font fallbacks.

## Structure-Aware Progressive AI Streaming (CLI, Desktop, and Web)
- **Smooth Line-by-Line Reveal**: Streams markdown text, lists, and syntax-highlighted code blocks smoothly with adaptive pacing.
- **Atomic Block Buffering**: Holds markdown tables and custom UI/Mermaid cards in buffer until complete to eliminate broken syntax flashes mid-stream.
- **Viewport Ergonomics**: Preserves terminal scroll positions during generation and defers post-turn metadata until streaming finishes.

## Streamlined MCP Tool Activity UI (Desktop and Web)
- **Accordion Grouping**: Moved raw MCP execution logs out of chat bubbles into collapsible `Executed X mcp_tool` activity accordions.
- **Clean Conversation Surface**: Reserved chat bubbles strictly for downloadable artifacts, critical notices, and final AI responses.

## Modern Setup & Onboarding TUI Wizards (`mint setup` & `mint onboard`)
- **Full-Screen Setup Wizard**: Modernized `mint setup` with a 3-step Ratatui TUI categorized across 49 agent tools, native plugins, and launch targets.
- **High-Contrast Monochrome Styling**: Unified `mint onboard` with solid gray row highlights, active cursor indicators (`█`), and dynamic footer shortcuts.
- **MCP Shorthand Flags**: Added `-a` (`--args`) and `-e` (`--env`) shorthand options to `mint mcp add`.

## Headless Browser Automation & Anti-Loop Safeguards (CLI, Desktop, and Web)
- **Headless Chromium Automation**: Replaced legacy headed browser windows with headless Chromium commands (`--headless=new`) and deterministic `mint browser run` workflows.
- **System Default Browser Previews**: HTML artifacts and previews open directly in your system's default browser across all platforms.
- **Anti-Loop Protections**: Automatically intercepts stuck tool loops ($\ge 3$ repeated outputs) and deduplicates screenshot analyses using SHA-256 hashes.
- **Diagnostic Slash Command**: Updated `/browser` and `/browser status` to report detected Chromium binaries and sandbox readiness.

## Shell Execution & Background Job Safeguards
- **Explicit Background Status**: Clearly marks promoted background commands (>180s) as `exit: running_in_background (job_id: <id>)`.
- **System Anti-Retry Notice**: Injects background execution notices to prevent agents from triggering redundant retry loops.
- **TUI Activity Parity**: Ensured the Full-Screen TUI recognizes background processes as actively running.

## 🚀 Architecture Evolution & Future Roadmap
- **Decommissioned Legacy Browser**: Fully retired the headed Mint Browser subsystem (`mint auto`, port 9222 takeover, and embedded browser frames).
- **Headless Foundation**: Replaced with a fast, decoupled headless automation engine, establishing a reliable foundation for future web capabilities.
