# Release Notes - Mint Agent v1.17.0

Released: 2026-10-17

## Mint TUI Command Styling & Execution Status Highlighting (2026-10-20)

- **Green Accent Command Styling in TUI**: Styled shell command subjects in the full-screen Mint TUI activity cards using the Mint Green accent color (`terminal_theme::ACCENT`), matching the detail view in `Ctrl+O` for visual clarity and consistency across the transcript.
- **Execution Status Highlighting (`[Command]` & `[Check]`)**: Styled execution progress notices across CLI, Desktop, and Web:
  - `Succeeded` highlighted in bold Mint Green (`ACCENT`).
  - `Failed` highlighted in bold Red (`ERROR`).
  - Exit code `0` muted in dim gray, and non-zero exit codes highlighted in red.
  - Desktop and Web UI parity: Added semantic `.execution-state` and `.execution-exit` styling to `ExecutionResults.tsx` and `mcp-results.css`.
  - **Streamlined Execution Activity into Native Timeline (Web & Desktop Parity)**: Removed the redundant bottom `ExecutionResults` / "Verified Evidence" block from the chat message bubble. All command executions are now unified directly inside the chronological Activity Timeline (`AgentActivityTable`), flowing naturally right where each action occurs with Mint's native pill card styling, terminal icons, and exit status badges (`→ exit: 0`), eliminating out-of-order duplicate command blocks at the bottom while keeping backend reliability and verification contracts fully intact.
  - **Full-Screen TUI Transcript De-duplication**: Suppressed redundant raw `[Command] Succeeded` / `[Check]` text notices in the Full-Screen TUI (both during live runs and transcript restore). All command executions are now represented exclusively by native interactive `ToolActivity` cards (`🟢 run_shell completed · exit 0`), keeping the TUI transcript clean, focused, and dev-native without duplicate noise.

## Inline Viewed Image Previews in Agent Activity (2026-10-20)

- **Inline Auto-Preview under `view_image` activities**: Integrated image thumbnails directly beneath completed `view_image` tool rows in the Activity timeline across both Web and Desktop UI (`AgentActivityTable`). Provides immediate visual feedback of inspected screenshots/images in chronological context without requiring users to scroll down to the bottom of the conversation.
- **Click-to-zoom modal**: Each inline preview includes an interactive thumbnail card with hover zoom micro-interactions, dark glassmorphism styling with checkerboard pattern for transparent graphics, and full-screen image inspection modal with pan/zoom and close controls.
- **Deep module integration & Platform Parity**: Reused the shared `ViewedImage` artifact and data URI resolver so Desktop and Web maintain identical behavior and lifecycle cleanup.


## Mint Web Workspace Resolution (2026-10-20)

- **Fixed `mint web` launching outside the Mint project directory**: Centralized Mint project root resolution (`find_mint_root`) to inspect explicit environment overrides (`MINT_ROOT`, `MINT_PROJECT_ROOT`, `MINT_WEB_ROOT`), executable ancestors, compile-time manifest paths, and common installation locations. Validates Mint-specific markers (`vite.config.web.ts`, `out/web/index-web.html`, and `@pheem49/mint`) so running `mint web` or `mint web --tailscale` from home (`~`) or other project directories correctly locates Mint's web UI assets without crashing with `Failed to find project root directory containing package.json` or misidentifying other repositories as Mint.

## Agent Execution Reliability (Phase 1) & Browser Automation (Phase 2) (2026-10-19)

### Phase 1: Agent Execution Reliability & Evidence-Based Verification
- **Replaced unconditional duplicate-shell skips**: Turn-local execution evidence allows repeating commands after file modifications or with an explicit `retryReason`. Unchanged identical failures are bounded to 3 attempts before halting with a clear rationale.
- **Typed process outcomes**: Replaced stdout string parsing with structured execution states (`succeeded`, `failed`, `running`, `cancelled`, `unknown`), exit codes, job IDs, and call IDs. Running processes are not repeated automatically; background jobs are polled with `shell_output`. Skipped commands report `skipped` with rationale.
- **Evidence-based completion**: Added `completionStatus` (`verified`, `unverified`, `blocked`, `failed`) to `finish`. Model prose cannot promote failed or unrun checks to verified. Checks executed prior to the latest edit do not verify that edit. Tasks with constraints finish honestly with `unverified` or `blocked` and an explicit reason.
- **Removed premature finish hints**: Removed instructions prompting the agent to finish immediately upon launching an application (such as opening Chrome or background processes).

### Phase 2: Browser Automation on Shared Engine
- **Headless Chromium runtime by default**: Agent automation uses an isolated, headless Chrome/Chromium/Edge runtime with a temporary profile, dynamic loopback port, and retained CDP connections. The user's manual Mint Browser window and profiles remain completely untouched. Added settings for agent browser mode (`headless` vs `headed`) and custom `browserExecutable`.
- **Command-line workflows via shared engine**:
  - `mint browser run --file browser-task.json`
  - `mint browser run --file browser-task.json --headed`
  - Executes ordered actions (open, observe, click, type/fill, select, scroll, wait, tabs, screenshot) in a single process/session with JSON reporting, failure exit codes, step checkpoints, and 120s timeout. Foreground commands automatically import screenshot artifacts into the conversation.
- **Screenshot & durable image artifacts**: Added `viewport`, `fullPage`, and `element` capture modes with cursor suppression. Screenshots and `view_image` produce durable, conversation-scoped artifacts with URL, timestamp, and dimensions, viewable and downloadable in chat across Desktop, Web, and TUI.
- **CDP scope note**: Initial release covers Chromium-based browsers (Chrome, Chromium, Edge) via Chrome DevTools Protocol (CDP). Firefox and full desktop operating system control are outside the scope of this release.


## Skills & Workspace Transparency

- Added active workspace path display to the Skills management view in Web and Desktop UI (`SkillsView`), showing which project workspace is being inspected or indicating when no workspace is active.
- Added source file path indicator in the skill detail modal to reveal where each skill file resides on disk.
- Added workspace directory label to CLI skill listings (`/learn`, `/skill list`, and `mint learn --list`).

## Mint Companion UI

- Added separate Window width (340–2000 logical pixels) and Window height (560–2000 logical pixels) controls to Companion Settings. Changes resize the native app and retain full-window geometry across launches, including larger physical dimensions on high-DPI displays.

- Fixed the Live2D freeze after expanding Companion: collapse now pauses and hides the existing model instead of destroying/reloading it. Stage cleanup also removes its render callback from Pixi’s shared ticker before disposing the renderer.

- Replaced Companion’s permission-blocked OS minimize action with Collapse to menu button: the native window shrinks to 64×64 logical pixels, leaving a single circular menu button. Clicking it or choosing Show Companion from the tray restores the previous size, window style and maximized state; compact geometry does not replace saved full-window dimensions.

- Fixed speech readiness when connection and initial snapshot events arrive together, and made the latest-turn check atomic with automatic playback ownership so a new message cannot be overtaken by a stale read request.

- Collapsed the persistent connection and handoff bars into a small accessible status dot beside Chat. Conversation names and connection/navigation details live in Chat; Continue in Mint is available from the Conversation radial group when ready. Disconnection shows a four-second notice before returning to the dot.

- Moved Companion voice controls into the circular action menu: the ♪ button toggles a floating panel, closed initially, with Read, Stop, Mute, Auto and Volume. Close or Escape dismisses the panel without stopping playback or changing voice preferences.

- Added optional Companion speech with Read latest reply, Stop, Mute, automatic reading (off by default), and local volume controls. Mint prepares completed replies and retrieves bounded audio chunks; Companion plays the bytes and drives Shiroko's `ParamMouthOpenY` from measured audio amplitude with smoothing.
- Centralized Desktop/Web read-aloud and Companion speech around shared text preparation, saved TTS settings, cancellation, and a cross-process SQLite playback lease. An enabled Companion owns automatic reading for its selected conversation; other surfaces avoid duplicate automatic playback, with short-lived routing that expires when Companion disconnects. New messages stop existing speech; reconnect snapshots do not replay history. OS Native remains available on Desktop/Web; Companion requires the Google audio engine.

- Added “Continue in Mint” conversation handoff from Companion: Desktop restores its existing window and selects the saved chat, Web opens its host-provided conversation URL, and TUI tries the original identifiable terminal before offering an explicit new-terminal action with the same chat ID and workspace. Offline and deleted sessions show recovery states; Desktop retains pending navigation until its renderer acknowledges it.

- Companion now recovers a restarted local Mint host when exactly one authenticated instance matches its saved executable and launch directory, keeping the selected conversation. Previously observed sibling processes cannot take over the selection; ambiguous or older hosts require manual selection. The connection status dot opens Chat and its conversation selector.

- Reworked the Live2D Companion interface with Tailwind CSS, translucent blue glass controls, a fully transparent desktop canvas, and animated ambient light, orbital accents, connection feedback, and panel transitions. Reduced-motion preferences remain supported. Removed the duplicate message composer; Companion keeps the shared conversation history and character interactions while messages are sent from Mint Desktop or CLI.
- Changed the Companion title-bar control to minimize to the OS taskbar or dock instead of hiding the window to the tray, and clarified its accessible label.
- Consolidated the Companion's Chat, interaction, appearance, minimize, and quit actions into one circular tree menu. Category and nested action bubbles share one button style, reveal labels on hover, and connect with animated lines.
- Synchronized Live2D model updates and rendering on Pixi's shared ticker, removed whole-stage idle bobbing, and kept rendering at 60 FPS to reduce visible model jitter.
- Improved Live2D mouse tracking and gesture stability in Mint Companion: added Desktop-wide global cursor tracking via Tauri/Rust so the character can look towards the cursor anywhere on the OS screen even when the window is not focused, combined with zero-latency local pointer events inside the window, deadzone smoothing to prevent jitter when the mouse is stationary, natural decaying status gestures, and shared Pixi ticker preservation during stage remounts.
- Added a visible "Move" drag grip pill in the header and enabled direct window dragging across the character stage, Live2D canvas, and chat card header with intuitive grab cursors, allowing users to effortlessly move the companion across the desktop.
- Expanded the Live2D model stage to occupy the entire window height for an immersive, full-body character view, and redesigned the bottom chat card into a floating glass overlay toggleable via a dedicated `💬 Chat` button on the top-right header with an intuitive close button.
- Added a "Vertical position" slider in Companion Settings (with one-click reset) allowing users to adjust the character up or down, and dynamically synchronized the gaze tracking facial anchor so eye contact remains centered regardless of the chosen vertical offset.
- Added Tailwind's Vite integration to the Companion workspace.

## Terminal and Web launch

- Escape still cancels generation from the chat composer while Tools is open; Escape within Tools remains reserved for that panel. Added browser regression coverage for this distinction and corrupt audio/video fallback.

- Added direct PDF, audio and video previews in Tools without preview servers or Background tasks. PDF includes page navigation and zoom with a bundled local PDF.js worker; audio/video have playback and seeking controls. Desktop streams authorized workspace files with range reads; Web uses bounded Blob URLs (up to 256 MiB) and releases them when their last tab closes. Unsupported playback falls back to file information and an external-app button. HTML keeps its existing service for relative assets.

- Workspace image and SVG previews now read bytes directly through native IPC or the existing authenticated API, without starting a localhost preview service or Background task. Reload rereads the original image; HTML previews keep their existing service. Image reads enforce workspace/read permissions and a 32 MiB limit.

- Added Workspace file drag-and-drop into Tools with a Mint-colored “Drop to preview” target. Images, Markdown, HTML and text/code open in preview tabs; other file types show their name, type, path and source workspace with an external-app button. Repeat drops reuse existing tabs and simultaneous drops share one load. Previews retain their source workspace, drag cancellation clears feedback, and load errors can be retried. The file context menu also provides Preview in Mint for text and unsupported files.

- Workspace child folders now start collapsed on the first visit. Expanded/collapsed choices are saved locally per workspace and restored on reopening, switching workspaces, and app restarts; collapsing a parent retains its descendants’ saved choices.

- Removed Desktop’s Workspace context popover. Its titlebar folder button now opens the Workspace view directly and closes Tools if needed; the File menu and Ctrl+O retain their folder-picker behavior.

- Sources now displays every item in its scrollable Tools tab with per-group counts and two-line titles. Collections above eight items include a search field filtering names, domains, and URLs, with matching/total group counts and an empty-result message.

- Moved session Sources into the shared Desktop/Web Tools sidebar with a source count and a dedicated tab. Removed the Web header Sources popover and the duplicate Desktop workspace-popover section; source grouping, retry, live updates, and Library links remain available in Tools.

- The Ctrl+T panel title now shows total thinking time across rounds. Each round begins with a Mint-green numbered heading and its own duration (including the first round); active timing continues updating, and missing historical durations remain marked unavailable.

- The Ctrl+T thought panel now uses the shared Mint accent for Next Step separators and blue for footer key names, while preserving white thought text and muted hint descriptions.

- Tool details now use shared theme colors for status dots, blue Command/Input and Output headings, and Mint-green shell code, while keeping output text white and copied details unchanged.

- Tool details now always show previous/next command shortcuts (Shift+Tab/Tab with wraparound), scrolling, copying, and Esc/Ctrl+O closing hints, including when long content scrolls.

- Shell approval dialogs now display the command and multiline code in the shared Mint accent color; labels, execution metadata, and approval controls retain their existing colors.

- Replaced TUI tool-card success and failure check/cross icons with green and red dots, keeping the status text and other heading details in the normal text color.

- Added `/shells stop` as a selectable autocomplete entry in CLI, Desktop, and Web.

- `/shells stop` now stops all running background terminals across workspaces accessible to the current user. `/shells stop <id>` still stops only the specified job; individual failures are reported without interrupting the rest.

- Workspace pickers in chat and Code sessions now share a persistent removal preference. Each workspace has a floating `⋯` dropdown with English-only “Remove from list” text; removing a workspace leaves files, sessions, and the active project intact. Open folder… restores it to the list.
- `/autoskill` now marks the active setting as `on (current)` or `off (current)` in its selection menu.
- `/mint-web` now opens the default browser after both the Web UI and API are ready. It displays `Mint Web: http://localhost:9000` with the URL in blue in the CLI, followed by `Your browser will open automatically when ready.`
- Added `mint web --open` to open the browser automatically from the shell.
- Repeating `/mint-web` reuses the running server and clearly reports that it is already running instead of promising another automatic browser launch.

## Voice input

- Web dictation now retains all finalized phrases across speech-recognition updates without duplicating revised results.
- Desktop microphone input now appends the transcript to the current composer draft instead of automatically sending a chat message. Users can review and edit the text before pressing Send.
- Canceled recordings and empty transcripts leave the composer draft unchanged.
- Switching conversations cancels pending dictation and prevents a delayed transcript from entering another chat's draft.
- Browsers without speech recognition, such as Firefox, now show a popup recommending Chrome or Edge when the microphone button is pressed.
