# Release Notes - Mint Agent v1.15.0

- Added the active model's thinking level to the Full-Screen TUI footer, including an `off` indicator when thinking is disabled.

## Mint Auto Browser Reliability

- Gemini Live voice sessions now keep one shared browser session across tool calls and spoken turns. Selected tabs, DOM references, screenshots, and recovery stops remain available until the voice task ends; ending the task releases tab leases, and a new voice session starts independently.
- Screenshot capture now retains DOM/layout snapshots from before and after the image. Coordinate validation checks only the intended target in both snapshots and retries a current capture once if that target changes. Continuously updating distant tickers and carousels no longer invalidate stable targets, while target replacement, movement, obstruction, and local pixel changes still prevent clicks.
- Coordinate hit tests now account for document scroll offsets, so a fresh screenshot taken after scrolling remains usable.
- Added headless Chrome regressions through the production Gemini Live background runner and agent executor for session continuity, recovery, screenshot reuse, lease release, and continuously changing unrelated DOM/layout.

- Fixed empty/default tab URLs to open `about:blank`; invalid supplied URLs are rejected before tab creation. Recovery stops now cover opening and closing tabs and survive tab selection.
- Browser retry counts follow the action and DOM target, so clocks, carousels, and refreshed references cannot hide ineffective retries. Navigation, new tabs, target value/checked/expanded changes, verified scrolling, and explicit confirmation waits establish progress for the relevant attempt.
- Typing and filling pin the original editable node and check focus after clicking and keyboard preparation. Focus redirects and replaced fields stop before text insertion or deletion.
- Coordinate input now validates screenshot target identity, geometry, current hit testing, and a 64×64 pixel region, including another check after hover. Distant page updates remain allowed; Mint cursor/aura overlays are excluded from evidence.
- Added isolated headless Chrome regression coverage through real agent argument parsing and tool execution for tab defaults, recovery, changing pages, redirected focus, shadow inputs, replaced targets, modals, hover overlays, and canvas changes.

- Browser agent runs now use dedicated tabs with pinned CDP targeting and exclusive tab leases across Mint processes. Popup tabs are reported for explicit selection; closed tabs fail instead of silently switching pages.
- Added structured browser observations with accessible element names, fresh DOM references, open shadow-root discovery, pagination, password-value masking, and explicit unsupported-frame counts.
- Added browser tab management, verified field replacement, native dropdown selection, scrolling, and bounded waits for URL, text, and element visibility conditions. Existing plain-text reads and selector arguments remain supported.
- Browser clicks now scroll targets into view and reject hidden, disabled, or covered elements. Removed the JavaScript click fallback; CDP errors, script exceptions, navigation failures, and stalled responses no longer count as successful input.
- Browser actions return page observations and distinguish dispatched input from verified outcomes. Repeated attempts without progress stop for manual takeover, and browser task completion requires a verification statement. Coordinate actions require a recent screenshot from the selected tab.
- Desktop browser commands accept optional tab IDs. Browser launch, tool availability, and CLI status honor the configured debugging endpoint. Browser typing logs contain character counts instead of field contents.
- Added unit checks and opt-in headless Chrome acceptance scenarios for forms, search, delayed content, popups, stale references, tab isolation, takeover, and recovery.

## Sidebar Projects

- Added a three-dot menu to each project in the sidebar. New chat actions are available from the menu, and projects can be removed from the sidebar without deleting their conversations; those conversations move to Recents.
- Moved conversation rename, move, and delete actions into a three-dot menu in both project lists and Recents.

## Linked Folders

- Linked folders now index supported files in nested directories and use relevant excerpts to decide where to save useful chat notes. The index refreshes on link, periodically during note processing, or with `/link refresh <name>`.
- Added `/link save <name> | <text>` and `/link notes <name>` across CLI, Desktop, and Web. A single turn can save separate notes in multiple relevant folders.
- Notes receive unique IDs and are appended under a cross-process file lock. A SQLite job and note ledger records pending, saved, skipped, and failed work and allows interrupted work to resume.
- The Linked Folders view shows index status, recent notes, errors, and note previews. Desktop can open the source note file; Web previews it in the app and can copy its path.

## Opaque Chat Surfaces

- Made source cards, their links popover, and markdown callouts use solid theme surfaces when Opaque is selected, while keeping their translucent treatment in Glassmorphism mode.
- Raised an open source-links popover above later chat messages so callout text and backgrounds do not overlap its links.
- Removed the remaining fixed blur on chat headers and rich grid cards while Opaque mode is active.

## Consistent Provider Model Lists

- Made CLI, Desktop, and Web use the same Rust model-list fallback policy. When a provider responds, its model IDs are shown as returned; static presets are used only if the fetch fails. Desktop and Web no longer append separate frontend presets, and their initial DeepSeek options match the Rust fallback, so models absent from the provider response no longer appear only in the app picker.
- Reused the fetched provider lists in the Desktop and Web Agents settings model picker, so its Gemini, Anthropic, OpenAI, OpenRouter, DeepSeek, and local model choices follow the main picker.

## Monochrome Theme Contrast

- Corrected text and icon contrast on accent-filled controls in Skills, MCP management, Image Studio, Veo Studio, chat, and Settings. These controls now use the theme's text-on-accent color, including selected, hovered, and disabled states.
- Made the custom checkbox checkmark follow the same contrast token and fixed selected sidebar items and light-theme management hover text.

## Live Translate

- Rebuilt the Desktop live translation picker as a transparent overlay: select a screen region, choose a target language (including a custom language), and read the translated text over the selected area.
- Live translation now samples only the selected region, checks for meaningful visual changes, and sends a new request only when the region changes. Sampling pauses cleanly when the user pauses, changes area or language, or closes the picker.
- The selected target language is remembered between sessions. Mint stays hidden while the live overlay is open so it does not cover the app being translated, then reappears when the overlay closes.
- Passed the initial screen preview through Tauri's in-memory handoff instead of browser localStorage, avoiding storage limits for large screenshots.

## Manual Screen Capture

- Rebuilt the Desktop and Web manual screen capture flow around a shared review dialog: capture, select an area or full image, retake, and explicitly attach to chat.
- Fixed the Web capture button, which previously called a no-op adapter. Web now requests browser screen sharing and shows capture errors in the dialog; Desktop uses native capture on Linux and browser screen sharing on other systems. Native capture briefly hides Mint to avoid capturing its own window.
- Replaced the ambiguous eye button icon with a screen capture icon and kept captured images out of the conversation until the user attaches them.
- Kept the existing Desktop live translation picker reachable from the new capture dialog.
- Prevented native capture retries from reading a stale screenshot left by an earlier failed command.
- Fixed screen capture button text contrast in the Dark Monochrome theme, including the disabled Attach to chat state.

## Chat Composer Controls

- Removed the inactive Smart Context Auto-Screen toggle from Desktop and Web chat. Manual screen capture and the Agent/Plan mode controls remain available.
- Removed the dormant Proactive Assistant screen-capture interval and suggestion-cooldown controls from Desktop and Web Settings; existing saved config values remain intact.

## Mint Persona

- Made Mint's female persona explicit in the shared system prompt used by chat and agent modes across CLI, Desktop, and Web. Mint now follows the user's current language or an explicitly requested language instead of defaulting to Thai; Thai feminine polite particles apply only to Thai replies.
- Removed Thai/English-only Web Search text appended after agent answers, so the model can explain search results and failures in the user's language without a mismatched-language suffix.
- Identified Mint explicitly as an AI coding agent in both native-tool and JSON-action agent prompts, while keeping plain chat as a general AI assistant and allowing non-coding tasks in Agent mode.

## Web Search Image Navigation

- Linked Web Search thumbnails to their actual result pages using the image and page URLs paired in agent progress, with a separate “View full image” link for opening the image file. The same rendering applies to Desktop and Web, including live replies and chat history.
- Added a separate “View full image” link to Image Search tiles while keeping a click on the thumbnail directed to its source page. CLI search results continue to show both URLs in text.
- Made both “View full image” links use the theme's interactive text color; the Image Search link background also follows the active surface theme.

## Shared Conversation Turns

- Sent prompts now enter the shared session immediately; the same turn receives the complete reply when it finishes. TUI, Web, and Desktop show queued, running, failed, and interrupted turns.
- Turns submitted to the same session run in order across processes. A short lease marks abandoned work interrupted and lets later turns continue.
- Added a change feed and paged snapshots for cross-surface updates. The TUI redraws when shared content changes, while Web and Desktop refresh the active conversation and session list and can load older messages.
- Web and Desktop receive the persisted turn ID as soon as a response starts, so their live reply is matched to the shared turn without showing a duplicate prompt or answer.
- Kept late replies and history loads from an earlier session out of the currently open chat, attached agent activity by persisted turn ID, and preserved identical prompts from other surfaces while a local reply is pending.
- Redrew the Full-Screen TUI once when a timed notice expires, so it disappears even while the terminal is idle.
- Made the conversation snapshot and change cursor authoritative for Web/Desktop history loads, preventing an older request for the same session from replacing newer turns.
- Kept tool approvals associated with their originating chat across session switches; returning to that chat restores its approval card, including for Desktop approval events.
- Preserved the chronological order of tool activity cards, timeline notes, and turn completion summaries in the Full-Screen TUI by switching live sync to an incremental feed that appends incoming external turns without wiping local in-memory transcripts or displacing notices to the bottom.
- Advanced the CLI's shared conversation cursor and interaction ID immediately upon local turn completion, preventing idle sync checks from treating a completed local turn as an external change.
- Fixed scoped session ID resolution in the TUI live sync poller so workspace-scoped conversations query their matching change sequences.
- Persisted agent activity timelines (tool execution cards, file changes/diffs, extended thinking, and run telemetry) directly from the engine level (`TurnLease`) into SQLite upon turn completion, failure, or interruption, establishing full cross-surface visibility across CLI, Desktop, and Web.
- Enabled real-time merging of agent activity snapshots during live conversation polling on Web and Desktop, dynamically rendering tool cards, file diffs, and thinking blocks for CLI/TUI turns without requiring a manual page reload.

## Shared Terminal Color Palette

- Centralized terminal and TUI colors in `terminal_theme.rs` and consolidated similar accent colors into Mint green, with shared blue, text, muted, warning, error, selection, and panel tokens.
- Kept code syntax highlighting on its dedicated theme so language tokens remain distinguishable.
- Restored the original four-line `Mint` ASCII wordmark in the Full-Screen TUI header, styled with the shared palette.
- Restored the animated white shimmer on the TUI thinking status, with its dim and bright endpoints defined in the shared palette.
- Corrected transcript scrollbar position mapping so the thumb reaches both ends of its track.

## Private Mobile Web Access with Tailscale

- Added opt-in `mint web --tailscale` to serve the production Web UI over a private Tailscale HTTPS URL suitable for installing the existing PWA on a phone.
- Bound the Web UI and API to localhost in this mode, checked for an existing Serve configuration before starting, and tied the Serve session to the Mint command. Normal `mint web` behavior is unchanged.
- Documented Tailscale, HTTPS, production build, and mobile installation requirements.
- Added a separate English setup and usage guide in `docs/MOBILE_PWA_TAILSCALE.md`.

## Interactive CLI Mode Switching

- Refreshed `/resume` in Classic CLI and Full-Screen TUI with compact one-line session rows, search, project/branch filters, Updated/Created sorting, and a restrained monochrome palette. Both views mark the current session.
- Added a clickable "↓ Back to bottom" button to the full-screen TUI transcript. It appears only while the transcript is scrolled above the latest message, disappears at the bottom, and keeps `End` available for the same action while typing or waiting for an agent turn.
- Added `$` skill and `@` mention suggestions to the full-screen TUI composer. Up/Down selects a match and Tab completes it; `@` completion replaces only the word at the cursor, including mentions within a sentence. The existing Enter submission behavior remains available.
- Press F6 at the prompt to choose whether to switch between the full-screen TUI and Classic CLI without ending the current session. The default choice keeps the current interface; an unfinished prompt is preserved whether the switch is confirmed or cancelled.
- Fixed TUI choice dialogs clipping the last option when their description spans several lines. Dialogs now use the available terminal height to show every choice that fits, including three-option approvals and lists longer than eight choices.
- Choice dialogs now measure wrapped command text at the current terminal width and reserve a footer row below the options, so approval choices remain visible in narrower terminal windows.
- Kept live `run_shell` activity visible while parallel tools are active: a thought from another tool no longer commits the in-flight status, and TUI tool-start events are retained during approval dialogs.
- Kept completed shell-command labels visible while Mint prepares its reply: the eight-row TUI live-status area now omits long stdout/stderr previews, which remain available in the completed turn's transcript.

## Full-Screen Interactive Chat TUI (Complete Default Interface)

- **Native-Feeling TUI Mouse Selection**: Added Mint-managed click-drag selection across the entire rendered TUI while preserving mouse-wheel transcript scrolling and physical `Up`/`Down` prompt-history navigation. Selected screen cells stay highlighted after mouse release and are copied with right-click, using the native system clipboard first with OSC52 fallback for terminal/remote sessions.
- **Markdown Table Rendering (TUI)**: Fixed tables showing as raw `| # | ไฟล์ | สถานะ |` / `|---|---|---|` in the TUI. `format_markdown_bold()` now detects consecutive `|...|` lines (using `is_table_line()`), buffers them, and flushes through `render_markdown_table()` — the same box-drawing Unicode renderer used in classic terminal mode. Tables at any position in the response (middle or end) are handled correctly. Code blocks containing `|`-separated content are exempt.

- **Status Line Color Refinement**: Improved visual hierarchy of live status lines and agent timeline notes.
  - **Tree branch connector `└`**: Styled in `DarkGray` (dim) — fades into the background as a subtle structural guide.
  - **Tool action text** (`[read_file] Read file.rs #L1-50`, etc.): Styled in **Plain White** (`Color::White`) without bold styling — clean, readable, and perfectly balanced with the dim branch connector.
  - **Short Thoughts & Timeline Notes**: Step-by-step thinking notes (e.g. intermediate thought records) now render in **Plain White** (`Color::White`) instead of dim gray, keeping AI notes clean and clearly legible across TUI transcript and live status.
  - Applied consistently across both **TUI mode** and **classic terminal mode**.
- **Thinking Line Shimmer Animation**: The `Pondering (5s · …)` / `Thinking…` status line renders with a smooth left-to-right shimmer effect in TUI mode. Each character is individually colored: the bright spot (full **#ffffff** white) sweeps from left to right across the verb text, fading to a dim gray at the trailing edge. The animation is driven by wall-clock time (≈12 fps), independent of the render loop speed. The timer suffix `(elapsed · Esc)` remains in dim gray and does not participate in the shimmer.
- **Turn Completion Summary & Token Statistics (`Worked for`)**: Restored the turn completion footer in full-screen TUI mode. After every turn, a styled rule (`─ Worked for 1.2s • ↑ 1.5k · ↓ 42 • anthropic • claude-3-5-sonnet ──────`) displays elapsed seconds, input tokens (`↑`), generated tokens (`↓`), and provider/model information directly beneath the assistant's response. Fallback providers and reasons are clearly noted if a fallback occurred.
- **Tool Activity Summary & Layout Alignment**:
  - **Active Tool Spinner & Real-Time Elapsed Timer**: The tool currently in flight now features an animated Braille spinner (`⠋ ⠙ ⠹ ...` in bold `Cyan`) and a live elapsed timer (`(1.8s)`) directly on the command line. Users get immediate feedback that a command is running without feeling frozen.
  - **Completed Tool Checkmarks (`✓`)**: Tools that have completed execution render with a crisp Mint Green checkmark (`✓` in `#69e6a6`), cleanly distinguishing finished actions from in-flight ones. Once committed to history, all executed actions retain the `✓` badge.
  - **Eliminated Generic `● activity` Fallback**: Expanded `activity_summary_line` to count all tool operations (editing files with `write_file`/`apply_patch`, git checks, background shell jobs, subagents, etc.). Fallbacks now display meaningful action counts (e.g. `Running 1 tool…` or `Editing 1 file…`) instead of the bare word `activity`.
  - **Single Tree Branch Connector**: All child tool actions under an activity group now share a single `    └` tree connector on the first item, with subsequent items cleanly indented by 6 spaces (`      [tool] ...`) rather than restarting multiple disconnected `└` branches.
  - **Bullet & Continuation Styling**: Activity bullets (`●` / `○`) highlight in bold `Cyan`, header text in bold `White`, continuation tool lines in plain unbolded `White`, and the commit divider line (`───`) is aligned with a 2-space margin to match the activity indentation.
  - **Dynamic Status Viewport**: Increased status height clamp to 8 lines and automatically displays the tail of status lines so the active tool and thinking/deliberating spinner are never clipped at the bottom.
- **Numbered Image Placeholders (`[Image #1]`, `[Image #2]`)**: Pasting images (`Ctrl+V`) into the TUI composer now sequentially numbers attached images (`[Image #1]`, `[Image #2]`, etc.) matching the classic CLI behavior. All image and pasted-text placeholders are styled with a bold Cyan badge in the input composer.
- **Auto-Dismissing Footer Notices (2.5s)**: Temporary status bar notices (`Image attached`, `Image saved to gallery (filename)`, `Transcript copied with OSC52`, `No image found in clipboard`, etc.) automatically expire after 2.5 seconds, cleanly restoring the default workspace and agent model info bar (`[Agent] <model>  path: <workspace>`). Notices are also dismissed immediately as soon as the user starts typing in the prompt composer. Eliminates raw stdout `Saved image: ...` bleeding into the prompt composer (`› `). Undersized terminal warnings (`Terminal too small`) persist until the terminal window is resized.
- **Docked Thought Process Panel (`Ctrl+T`)**: Relocated the Thought Process modal from floating in the dead center of the screen to a docked bottom panel (sharing the layout mechanism with the `/model` dialog). The chat transcript remains fully visible in the top area while viewing thought traces. Supports keyboard navigation (`↑`/`↓`/`PgUp`/`PgDn`), mouse wheel scrolling, and quick close (`Esc`, `Enter`, `q`, or `Ctrl+T`).

- **Default Full-Screen Interface**: Made Ratatui the complete, default interface for interactive `mint` sessions, keeping the entire workflow inside the terminal alternate screen. Retained `--tui` for compatibility and `--classic` (plus automatic fallback on non-TTY or undersized terminals `< 60x12`).
- **Eliminated Terminal Suspensions**: Completely removed `terminal.suspend()` and deleted the disruptive `"Press Enter to return to Mint TUI"` prompt. All interactions remain inside the TUI session from start to finish.
- **Unified `CommandUi` Abstraction**: Routed all slash commands (`/branch`, `/resume`, `/palette`, `/mcp`, `/plan`, `/fast`, `/clear`, `/jobs`, `/shells`, `/stats`, etc.) and `$skill` execution through a unified `CommandUi` trait. Interactive prompts, confirmations, and selections render as native, centered Ratatui modal dialogs with arrow-key navigation, numeric shortcuts (`1`–`9`), viewport scrolling, and type-to-filter support.
- **Clean Startup & Session Picker**: Eliminated raw stdout banner pollution prior to alternate screen initialization. Running `mint -r` (no ID) now launches the interactive session picker directly within an alternate-screen modal dialog, seamlessly switching sessions and re-populating the transcript.
- **Enhanced Transcript & Roles**: Added styled roles for command outputs (`Command ›` in magenta) and system notices (`System ›` in yellow), providing clear visual separation between user prompts, agent turns, tool activities, and command results.
- **Parity Across Interfaces**: Verified full feature consistency across CLI (`crates/mint-cli`), Desktop (`src/renderer/src`), and Web (`src/renderer/src-web`), maintaining parity in workspace diffing, branching, session management, and MCP configurations.

## Real Git Workspace Diff and Code Review

- Implemented real Git workspace diffing across CLI, Desktop, and Web using `git diff -U3 HEAD` and `git status --porcelain` to detect uncommitted, staged, and untracked file changes.
- Removed mock and sample diffs entirely; Code Review now inspects actual files within the active workspace.
- Added working tree scope toggling between `Workspace Git` (all project-level repository changes) and `AI Edits` (edits accumulated during the current chat session).
- Integrated live branch switching via `GitBranchSelector` in the review header, allowing users to view and switch branches directly.
- Added clean working tree detection with a refresh action when no uncommitted changes exist.

## Sidebar Workspace Projects and Session Organization

- Grouped conversations into collapsible project workspace folders under **Projects**, with unassigned chats organized under **Recents**.
- **Stable Project Order**: Eliminated project list jumping when selecting conversations; project order remains deterministic.
- **Drag-to-Reorder**: Added drag-and-drop reordering for project folders with smooth visual indicators, persisted to `localStorage` (`mint_project_order`).
- **Unified Smooth Scrolling**: Enabled full-height scrolling on `.sidebar-section`, ensuring `Code`, `Projects`, and `Recents` scroll together smoothly without being cut off behind the bottom account footer.
- **Recents Stability**: Conversations in Recents strictly remain in Recents when viewed or active, avoiding unintentional auto-linking to projects.
- **Move Conversation Popover**: Added an inline move action to easily transfer chat sessions between Projects or back to Recents.
- **Inline Project Chat Creation**: Added a `+` button on hover for each project folder in the sidebar to create new chats directly scoped to that project.
- **Project Context Indicators**: Added a subtle `📁 ProjectName` badge in the chat header and empty chat welcome screen across both Desktop and Web to clearly indicate the active project context.

## README Positioning

- Reworked the top of the README around the primary use case: assigning coding tasks from Telegram or other messaging apps while Mint runs locally, verifies changes, and requests approval before risky actions.
- Added a concrete interaction example, focused value proposition, quick-start commands, and a short description of the intended audience.

## Installation

- Added SHA-256 checksum assets for standalone CLI release binaries.
- Updated `install.sh` and `install.ps1` to install a matching verified prebuilt CLI binary first, with automatic fallback to the existing source-based npm installation when no matching release asset exists.
- Added `MINT_SOURCE_INSTALL=1` to force the source-install fallback when needed.

## Community and Contributions

- Added a public roadmap covering near-term installation, onboarding, messaging, contributor, and platform priorities.
- Added `CONTRIBUTING.md` with setup, validation, issue, good-first-issue, and pull-request guidance.
- Added GitHub issue and discussion templates plus links from the README to the project community resources.

## Tool Surface Tabs & Modernized Terminal UI

- **Unified Single Header**: Removed the duplicate inner terminal header (`.terminal-dock-header`); path, active cwd badge (`~/.../folder`), and window controls are now seamlessly unified into the topmost Tab Bar.
- **Inline `+` New Terminal Tab**: Added a dedicated `+` button directly after the tabs to open additional terminal sessions quickly.
- **Project-Scoped Tab Titles**: Terminal tabs now clearly display the active project/folder name (e.g., `>_ Mint-CLI`, `>_ Mint-CLI (2)`) along with a modern SVG prompt icon (`>_`).
- **Maximize & Split Toggle (`⤢`)**: Added a maximize/restore button in the header actions allowing instant expansion across the full workspace or collapsing back to the split side view, with automatic `xterm` terminal re-fitting via `ResizeObserver`.
- Removed the legacy Terminal/Browser dropdown from the chat header; desktop tools are now opened through the Apps launcher and right-side tool panel.
- Added a draggable resize handle to the Apps/tool panel, with the selected width persisted for the next opening.
- Changed the active tool switcher into a working floating menu that opens Review, Terminal, Browser, Files, or Side chat without leaving the current tool page.
- Removed the horizontal bottom-docked Terminal layout; Terminal now opens only as a resizable right-side tool page.
- Fixed the Switch tools button being blocked by the active Terminal surface; the tool header now stays above Terminal content for reliable clicks.
- Fixed the Apps launcher button being pushed onto a second titlebar row by the titlebar grid; it now stays in the main toolbar and opens the Tools panel correctly.
- Removed the redundant Open tools launcher button; Open tools panel is now the single titlebar control for the Tools surface.
- Fixed the Switch tools menu being hidden behind the active Terminal surface; the tool header now renders above Terminal content.
- Improved Browser surfaces with loading/error states, Home and Stop controls, clearer external-browser fallback, persistent per-tab history while switching tools, same-surface iframe navigation, and Duplicate/Rename/Close-other tab actions.
- Replaced the Browser address bar's text `Go` action with a compact arrow icon while preserving Enter-to-navigate behavior.
- Changed the Apps Browser action to open the native Tauri Mint Browser window, allowing sites such as Google and YouTube to run with normal cookies, JavaScript, navigation, and login behavior.
- Added a native Mint Browser toolbar with URL input, Home, Back, Forward, Reload, and direct navigation through the embedded native page WebView.
- Refined the native Mint Browser shell with a browser-style tab row, full-width URL toolbar, active-site label, and functional tab close/new-tab controls.
- Fixed the native browser toolbar WebView overlapping the page WebView and leaving a black/blank tab area; the shell now occupies only its 92px toolbar region.
- Added browser-style URL autocomplete suggestions for web searches, domains, and direct addresses.
- Fixed URL suggestions being clipped or moving the toolbar below the page; the native layout now keeps the toolbar first and temporarily moves the page below the expanded suggestion area.
- Replaced the expanded browser suggestion area with a separate floating Tauri popup, keeping the page WebView fixed while suggestions open, update, and close independently.

- Fixed the native window Minimize, Maximize, and Close controls being pushed out of narrow titlebars after adding the Tools and Apps launcher buttons; window controls now stay pinned to the far-right edge.
- Restored a dedicated Apps launcher button in the desktop titlebar; it always opens the right-side tool launcher even after the panel has been collapsed.
- Added a visible collapse button inside the right-side Tools panel, including the empty launcher state, so the panel can be dismissed without reaching back to the titlebar.
- Replaced the tool dropdown with a right-side tool panel launcher; the titlebar Tools button opens the panel and presents Review, Terminal, Browser, Files, and Side chat as a vertical launcher before a surface is selected.
- Added Review, Files, and Side chat actions alongside Terminal and Browser; Review opens the current conversation-wide change set, Files returns to the workspace view, and Side chat returns to the conversation.
- Updated Review to aggregate code changes across the entire conversation instead of only the latest turn; repeated edits to the same file are combined into one review entry with accumulated additions, deletions, and hunks.
- Changed the Review surface to render every changed file in one scrollable conversation-wide diff, with the changed-file list on the right acting as an anchor navigator.
- Added a compact composer status pill for the current or latest Agent run: it shows files changed with green additions and red deletions, switches to a live editing state while work is running, and opens Review when clicked.
- Moved preview, code review, and desktop terminal into dedicated tool pages with a shared tab strip.
- Added per-tool close buttons and a close-all action so open tool pages can be removed without clearing chat history.
- Preserved terminal resizing and bottom/right layout controls inside the dedicated terminal page.
- Added a plus-menu beside the tool tabs for opening the existing Terminal and Browser actions.
- Terminal entries from the plus-menu now create independent terminal tabs and sessions, with numbered titles when more than one is open.
- Added embedded Browser surfaces with independent tabs, URL navigation, reload, and per-tab back/forward history.
- Deferred xterm and embedded-page initialization until after the surface header paints, reducing perceived lag when opening Terminal or Browser.
- Added an `Open in Mint Browser` fallback button for sites that refuse embedded previews.
- Fixed the Terminal orientation control so it actually switches the terminal surface between the right-side and bottom layouts.
- Fixed desktop minimize and maximize/restore controls by calling the Tauri window API directly and surfacing failures in the console.
- Added transparent edge and corner hit areas so the frameless desktop window can be resized by dragging its borders or corners.
- Reduced chat-stream jank by batching automatic scrolls to one animation frame, skipping them while the user reads older messages, and throttling live Markdown rendering to 120 ms.
- Deferred loading workspace, tools, media studios, management views, and the command palette until each is opened, reducing the initial chat bundle and avoiding background effects from hidden panels.
- Split route-specific styles into their own lazy-loaded panels, so settings, media studios, pictures, spotlight, widget, and tool-surface CSS no longer block the initial chat UI.
- Reduced Live2D's idle renderer budget to 24 FPS, raising it only while the pointer is over the model and limiting pointer tracking to the model itself.
- Kept opened terminal tabs mounted while inactive so changing tabs preserves the existing shell process and scrollback instead of restarting them.
- Reduced chat and workspace background polling: refreshes now run only while the app is visible and focused, prevent overlapping reads, refresh immediately when returning to the app, and use a lighter 15-second chat / 30-second workspace cadence.
- Reduced initial font downloads by removing duplicated Google Fonts requests and cutting the self-hosted font bundle from 25 broad imports to Thai/Latin Prompt plus Latin/symbol Fira Code subsets; Prompt now covers the default Thai and Latin UI while Fira Code remains available for code and terminals.
- Split dashboard CSS from the app bootstrap for desktop and web: chat, sidebar, desktop tools, observability, model selector, and UI-card styles now load only with the lazy dashboard route, while page-specific panels retain their own CSS chunks.
- Fixed right-side terminal resizing so dragging its divider updates the outer tool surface and available chat width, not only the terminal's internal viewport.
- Increased chat density: the default UI scale is now 16px, conversation text and composer use 16px, code blocks use 13px, and message metadata uses a compact 12.5px-equivalent scale. Existing 18px default settings migrate once to the new 16px default; any size selected afterward is retained. The typography controls now clearly identify Compact, Default, and larger sizes.
- Fixed chat-panel overflow from agent telemetry and tool activity: activity cards, KPI summaries, and timeline labels now shrink, truncate, or wrap within the available panel width, and the chat container no longer exposes a page-level horizontal scrollbar.
- Changed the default surface style to Opaque for clearer text and lower compositing cost; Glassmorphism remains available, with selectable blur intensity, in Theme Settings.
- Separate desktop launch modes: `npm run dev` keeps the Tauri/Vite development session with HMR, while `npm start` opens the local production binary without leaving Vite running (building only when that binary does not exist); `npm run start:rebuild` explicitly refreshes it.
- Fixed web tool actions: Browser now opens the requested URL in a real browser tab, Preview and Review use the chat split view, and the native-shell Terminal remains desktop-only instead of exposing an unusable web control.
- Fixed the web Browser launch menu losing its styling after the dashboard CSS split by loading its shared menu stylesheet in both the desktop and web bundles.
- Fixed web file-change cards falling back to unstyled Review controls by sharing the current summary, action, and list layout CSS between the desktop and web dashboards.
- Fixed the web full-page Review view rendering as raw HTML by loading its toolbar, diff, and changed-file-list styles from a shared dashboard stylesheet.
- Fixed the web Settings modal rendering without its layout or tab styles by bundling the settings base and tab styles with its lazy-loaded web component.
- Made code-review diffs semantic in Dark Monochrome: added lines and counts now use green success styling, while deleted lines and counts use red error styling instead of inheriting the white accent color.
- Applied the same green-added and red-deleted semantics to inline file-change previews in the normal chat timeline.
- Improved Dark Monochrome contrast across shared controls by separating interactive text/icon color from the white accent surface; primary actions remain white with dark labels, while accent-colored labels use zinc shades that remain readable on selected or light surfaces.

## Desktop Terminal and Browser

- Added an interactive desktop terminal backed by a persistent pseudo-terminal session, with workspace-aware startup, live output, keyboard input, and terminal resizing.
- Added bottom/right terminal docking with a header control; Mint remembers the selected dock position across launches.
- Added drag and keyboard resizing for the terminal dock and persisted bottom height/right width separately. Switching workspaces restarts the terminal in the newly selected folder.
- Added a desktop tools menu for opening the terminal or launching a URL in a native Mint Browser window. Browser navigation is limited to HTTP and HTTPS pages and does not receive Mint's Tauri IPC access.

## Agent Reply Completion and CLI Rendering

- Wait for the authoritative completed Agent response before displaying assistant text across CLI, Desktop, and Web, while tool activity and working status remain live.
- Added the native `finish` tool and require Agent mode to put its final response in `finish.summary`, ensuring every surface renders the same complete answer.
- Render complete Markdown documents in the CLI so long paragraphs, tables, and rich option cards are not split at provider chunk boundaries.
- Render Markdown tables through the CLI terminal-table renderer instead of showing raw pipe-delimited source.
- Measure final CLI scrollback with Ratatui's actual Unicode-aware wrapping before insertion, preventing Thai text, emoji, and rich option cards from clipping the end of a complete response.
- Removed the redundant CLI `Verification:` footer; verification remains part of the internal agent result while relevant test outcomes and limitations stay in the main response.
- Print collected web sources after the completed response.
- Added a consistent two-column left margin to CLI progress notes, including wrapped Thai text, so live activity no longer touches the terminal edge.
- Replaced the verbose CLI Agent Run card and duplicated tool timeline with a one-line completion summary; changed files and failed tools appear only when relevant.
- Made terminal-table width tests deterministic by injecting an explicit width while production rendering continues to use the live terminal size.

## JSON Card Parsing

- Recover all JSON-backed chat cards (weather, stock, calculation, image, UI grid, feature, and mockup) when a simple field is accidentally emitted outside its object; unrelated malformed JSON still falls back safely rather than being guessed at.
- Strengthened card-generation instructions and added regression coverage for the malformed three-option payload and detached scalar fields.

## Assistant Presence Widget

- Made the floating assistant open the main chat when clicked and added a hover-only control that turns Assistant Presence off.
- Fixed widget dragging by starting Tauri's native drag from the mouse-down event and granting the widget window the required permission; clicking still opens chat.
- Persisted the Assistant Presence toggle immediately, so its enabled/disabled state survives closing Settings and restarting Mint.

## Model Provider Icons

- Added bundled brand logos for supported AI providers beside models in the selector, with larger logos in group headings and the detail pane. Providers without a dedicated logo keep the generic fallback icon.
- Fixed monochrome logos rendering black on the dark selector by using available color variants and provider-colored masks for monochrome marks.
- Reused the same provider brand logos in General settings' AI provider cards.
- Added optional custom-provider logos in General settings. PNG, JPEG, and WebP files up to 256 KB are saved in configuration and shown in the provider card and model selector.
- Fixed logos disappearing after save in the web settings flow by preserving the logo data URL through backend config serialization.
- Refresh the chat's provider configuration and availability immediately after web settings are saved, so custom providers and models appear without reloading the page.

## Skills List Descriptions

- Replaced YAML frontmatter separators in skill list previews with each skill's description, falling back to the first content line when no description exists.

## Empty Chat Welcome Alignment

- Centered the empty-chat welcome message above the composer on desktop while keeping the composer anchored at the bottom.
- Centered the web empty-chat welcome message in the main workspace while keeping the composer anchored at the bottom.

## Workspace Chat Width

- Let chat messages use the available pane width when Workspace and Chat are open together, instead of keeping the standalone chat's 620px message limit.

## Workspace Opening Motion

- Smoothed the Workspace transition by keeping grid tracks interpolable, fading the Live2D stage out instead of removing it instantly, and animating the file panel's entrance; respects reduced-motion settings.

## Image Studio Send-to-Chat Draft

- Changed Image Studio's Send to chat action to attach the selected full-size image to Chat input and keep its prompt editable as a draft; it no longer only fills text or sends automatically.

## Sentence-Case UI Copy

- Standardized English interface copy across Desktop and Web to sentence case, removing forced all-caps/capitalize styling and adjusting common navigation, settings, workspace, gallery, and action labels while preserving product names, acronyms, and model identifiers.

## Image Studio Sentence-Case Copy

- Removed forced uppercase styling from Image Studio labels and provider badges, and standardized visible actions and section titles to sentence case with clearer image-upload wording.

## Skills Workspace Scope Labels

- Fixed Skills and Plugins views labeling workspace skill files as Global by recognizing Tauri's `location` field as well as the Web API's `is_workspace` flag.

## Image Studio Gallery Image Loading

- Fixed saved image thumbnails failing to load in the desktop gallery by preferring the filesystem thumbnail path over the `file://` URL when converting image sources for Tauri.
- Added a one-time fallback to the original image when a saved thumbnail cannot be loaded.

## Desktop Workspace File Explorer UI

- Rebuilt the missing Workspace stylesheet as a compact desktop file explorer with a clear header, flat action toolbar, sticky project root, correctly sized material file icons, and dense scannable tree rows.
- Restored the intended two-pane Workspace layout with the file explorer on the left and chat composer on the right, while hiding the Live2D stage and preventing empty-chat positioning rules from overlaying the full workspace.
- Reworked the workspace selector from a prominent accent card into a compact project-context row integrated with the composer, using neutral surfaces and restrained hover/focus feedback while preserving folder selection and full-path tooltips.
- Added a compact Local Branch selector beside the active workspace, including branch search, current and detached-HEAD states, dirty-worktree warnings, safe Git switching, and immediate workspace-tree refresh after a successful switch.
- Extended the Branch selector with create-and-switch, locally known remote tracking branches, and an in-place Git graph showing the latest decorated repository history without performing an implicit network fetch.
- Replaced decorative gradients, glow effects, elevated cards, and AI-style status capsules with restrained surfaces, structural dividers, typography, and a small state indicator suited to a developer tool.
- Added keyboard focus treatments, disabled and active control states, long-name truncation, contained scrolling, narrow-window adjustments, reduced-motion behavior, and forced-colors support.
- Preserved the existing workspace file creation, folder creation, refresh, drag-to-mention, expand/collapse, delete, and automatic refresh behavior without changing component logic.
- Bound workspace mutations to an explicit workspace root and relative path, rejected traversal and symlink escapes, and carried the workspace revision through consecutive file operations.
- Unified workspace reads and mutations around `{ root, relativePath, revision }`; every operation now returns a complete snapshot containing the canonical root, tree, Git state, and revision.
- Added grouped renderer platform seams for authentication, conversations, catalogs, media, runtime capabilities, and workspaces, removing direct runtime-adapter imports from shared UI modules.
- Replaced generic conversation field setters with semantic transitions for composing, attachments, workspace selection, run lifecycle, streaming, progress, approvals, cancellation, and session switching.
- Added interface tests for Git branch outcomes, Workspace containment and revisions, renderer platform routing, and conversation transitions.
- Restored immediate Workspace snapshot refresh after branch changes, removed stale polling revisions, strengthened typed platform dispatch, and included interface tests in the default test command.

## Git Branches in CLI & Desktop

- Moved shared branch inspection and switching into `mint-core`, then added `mint git status|list|switch|create|track|graph` so CLI and Desktop follow the same branch, dirty-worktree, and remote-tracking rules.
- Added the current Git branch beside the workspace path in the interactive CLI prompt, `/cd` update, session stats, and exit summary.
- Added the CLI-only `/branch` slash command with an interactive local/remote branch picker, direct `/branch <name>` switching, and dirty-worktree confirmation.
- Preserved the semantic Git branch-change outcome through Tauri so Desktop requests dirty-worktree confirmation only after `mint-core` returns `confirmation_required`.

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
# Unreleased

- Changed the Full-Screen TUI watermark to turn from M to the leaf in 0.75 seconds, pause there for two seconds, then turn back to M in 0.75 seconds.
- Smoothed the Full-Screen TUI watermark rotation to about 33 frames per second and kept its timing steady while mouse or keyboard events arrive.
- Matched the Web/Desktop TUI theme cards' previews to the `/theme` code sample, including syntax colors and added/removed diff rows.
- Separated the TUI color choice into `tuiTheme`, preserving the previous terminal choice when loading older configs. Web and Desktop Theme & UI now offer Auto, Dark, and Light TUI previews without changing their own appearance.
- Added `/theme` to the Full-Screen TUI with Auto, Dark, and Light choices, a changing color preview, and saved theme selection.
- Fixed the Mint watermark's front M by making the emblem face opaque to its rear leaf pattern, so the two symbols no longer overlap.
- Set the terminal tab/window title to `Mint Agent | <workspace>` while the Full-Screen TUI is active.
- Replaced the empty CLI chat wordmark with a layered dotted Mint emblem: an M on the front, a leaf on the back, and a shaded circular rim. Clicking rotates it once in 3D; clicking during the turn restarts it.
- Fixed CLI TUI mouse selection so Ctrl+C copies selected text, while Delete/Backspace remove selected characters from the input composer.
- Fixed the TUI mouse-selection hint overlapping the copy confirmation; the hint now names Ctrl+C and yields the footer to copy status messages.
- Fixed long pasted input wrapping at the wrong positions for Thai and wide Unicode text; the composer now wraps by terminal display width and scrolls to keep the cursor visible.
- Changed CLI TUI update alerts from a brief footer message to a persistent bordered card with the current and latest versions, `mint update --approve`, and the release notes link.
- Kept the mobile chat header anchored inside the chat panel during iPhone keyboard panning, and made keyboard detection use the pre-focus viewport height so the empty prompt and footer make room for typing.
- Fixed the iPhone keyboard layout by keeping Mint Web aligned with the visual viewport as Safari pans it, while reclaiming chat space from the footer during typing.
- Fixed the mobile sidebar's More menu to expand inside the drawer instead of beyond the screen, and added a visible close button and roomier menu targets.
- Added pull-to-refresh on mobile Mint Web content pages, with a visible release indicator and protection against accidental refresh while editing Settings or generating a chat response.
- Reworked Web Settings on phones so form rows and provider cards use the available width, Thinking controls wrap into readable rows, effort buttons form a two-column grid, the close button sits in the header, and fields no longer cause page-wide horizontal scrolling.
- Reflowed the mobile Chat composer into distinct message, full-width model, and action rows so icon buttons no longer overlap the model label. The mobile model picker now fits the composer width and offers Models/Options tabs without a desktop-width panel spilling past the screen.
- Matched the mobile Chat navigation button to the new 44px rounded menu button used on other Web pages, including its icon size, themed surface, border, and keyboard focus state.
- Added a mobile-only rounded surface treatment that follows the existing Opaque and Glassmorphism theme setting. Opaque chrome is fully solid; Glassmorphism frosts navigation and control bars while the composer, message bubbles, studio content, settings, and confirmation modals stay opaque for readability.
- Added a consistent mobile navigation button to every non-chat Mint Web page, including Image Studio, Veo Studio, Pictures, and management views. Image and Veo Studio now use one vertical scroll flow for controls and results on phones, with compact headers and mobile-sized form fields.
- Reworked Mint Web's phone layout: the chat now follows the visible viewport when the mobile keyboard opens, the composer and header have larger touch targets and safe-area spacing, artifact/code previews use the full screen instead of a cramped split pane, and the Pictures gallery has compact responsive spacing and filters.
- Made Web settings full screen on phones with horizontally scrollable section navigation and persistent Reset/Save actions; the mobile navigation backdrop is now a keyboard-accessible button and closes with Escape.
- Fixed the web Pictures page not loading its gallery stylesheet, which caused saved images to render at their intrinsic size instead of inside the responsive card grid.
- Changed `npm start` to reuse the existing desktop binary and build only when it is missing; `npm run start:rebuild` now explicitly refreshes the fast local release.
- Changed `npm run cli` to use the existing compiled CLI binary; added `npm run cli:rebuild` for an explicit release rebuild and `npm run cli:dev` for Cargo development runs.
- Added a desktop Workspace context popover in the titlebar for on-demand workspace, Git branch/status, recent agent-change review, terminal-tab, attachment-source, and refresh information without consuming chat space.
- Fixed Theme & UI's live Opaque/Glass, preset, and accent choices so they save as one complete configuration immediately and synchronize the main window instead of leaving stale surface-style values behind.
- Expanded the desktop File, Edit, View, and Help menus with usable workspace, tool-surface, settings, zoom, fullscreen, update, clipboard, and keyboard-shortcut actions. Edit commands now disable when focus is outside an editable field, and application shortcuts are owned by the titlebar to prevent duplicate Terminal toggles.
- Fixed the Git branch menu clipping behind the Workspace pane by positioning it within the conversation panel and aligning to the side with more available room.
- Added a searchable Workspace menu with up to eight recent folders, a current-workspace checkmark, and an Open folder action that reuses the native folder picker.
- Restyled Workspace and Git branch selection as a compact project-context bar with a distinct workspace chip and adjacent branch control, preserving folder selection, branch menus, and responsive truncation on Desktop and Web.
- Matched the Chat composer frame's top and bottom corner radii at 16px across Desktop and Web, including the workspace selector, mode bar, and input form.
- Refined the Chat composer mode controls into unified, fully clickable options with clearer active, hover, keyboard-focus, and narrow-screen states; preserved the existing Smart Context, Agent Mode, and Plan Mode behavior across Desktop and Web.
- Centralized Git branch-change safety decisions in mint-core; CLI, interactive /branch, and Desktop now use the same workspace-change module before switching, creating, or tracking a branch.
- Added a shared workspace platform module. Shared Git and workspace renderer modules now cross one interface, while Desktop and Web install their own adapters at startup.
- Started moving workspace tree and relative-path policy into mint-core's Workspace module.
- Deepened conversation coordination with a shared reducer and React adapter. Conversation run state, composer attachments, draft/workspace persistence, streaming callbacks, approvals, progress, and cancellation now cross one coordinator interface; ChatPanel now receives only a view model and actions.
