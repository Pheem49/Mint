# Release Notes - Mint Agent v1.17.0

Released: 2026-10-17

## Desktop & Web Frontend Rendering, Bundle Splitting & Transport Optimization (2026-10-26)

- **Phase 1 — Frontend Rendering, Memoization & Code Splitting**:
  - **Isolated Thinking Timer (`src/renderer/shared/components/chat/ChatPanel.tsx`)**: Extracted the 1-second elapsed-time counter (`elapsedSeconds`) into a dedicated `<ThinkingElapsedMeta />` leaf component so the 1,900-line `ChatPanel` no longer re-renders every second while streaming.
  - **Conditional Ollama Probing (`src/renderer/shared/components/chat/ChatPanel.tsx`)**: Added `shouldProbeOllamaModels` so `ChatPanel` only probes `http://localhost:11434/api/tags` and `fetchProviderModels('ollama', '')` when `status?.activeProvider === 'ollama'` or an Ollama host/model is configured in settings.
  - **Dashboard Render Cleanup & Sidebar Memoization (`src/renderer/shared/components/app/MintDashboard.tsx`, `DashboardSidebar.tsx`)**: Computed `conversationReviewChanges` on-demand when opening Code Review instead of on every render, removed dead per-render session date-grouping calculations (`filteredSessions`, `groupSessionsByDate`, `groupedSearchSessions`), wrapped `DashboardSidebar` in `React.memo`, and stabilized sidebar callbacks via `useLatestCallback`.
  - **Adaptive Background Polling (`BackgroundTerminals.tsx`, `ModsPanel.tsx`, `useSessionSources.ts`, `MintDashboard.tsx`)**:
    - Added `shouldPollSessionSources` so `/api/conversation-sources` only polls when the Tools panel is on the launcher or the `sources` tab.
    - Added `backgroundPollIntervalMs` (3s active / 10s idle) in `BackgroundTerminals.tsx` and `modsPollIntervalMs` (3s active / 12s idle) in `ModsPanel.tsx`.
  - **Markdown & Tool Card Hot-Path Optimization (`src/renderer/shared/utils/markdown.tsx`, `src/renderer/shared/components/cards/ToolResultCards.tsx`, `src/renderer/shared/components/cards/MermaidCard.tsx`)**:
    - Added an O(1) character-presence fast-path to `sanitizeLatex` so plain prose skips 22 sequential regex passes.
    - Consolidated 5 separate `useMemo` loops over `progress` in `ToolResultCards` into a single-pass `extractToolCardPayloads` helper.
    - Multiplexed `MermaidCard` theme-change `MutationObserver` instances on `document.documentElement` through a single shared `subscribeThemeChanges` observer.
  - **On-Demand Lazy Chunk Splitting (`markdown.tsx`, `ToolResultCards.tsx`, `ChatPanel.tsx`, `ToolSurfacePage.tsx`, `src/App.tsx`, `src-web/App.tsx`)**:
    - Converted specialized rich cards (`WeatherCard`, `StockCard`, `CalculationCard`, `ImageSearchCard`, `ImageGenCard`, `MermaidCard`, `UiGridCard`, `UiFeatureCard`, `UiMockupWidget`) and secondary surfaces (`CodeReviewPage`, `RewindModal`, `ArtifactPreviewPanel`) to `React.lazy` wrapped in `<Suspense fallback={null}>`.
    - Deferred background preloading of `SettingsWindow` to `requestIdleCallback` (with a 3.5s fallback timer) across both Desktop (`src/renderer/src/App.tsx`) and Web (`src/renderer/src-web/App.tsx`).
- **Phase 2 — Conversation Transport, Web API Framing & SQLite IPC Optimization**:
  - **Zero-Allocation 32 MiB Request Validation (`src/renderer/shared/conversation/transport.ts`)**: Replaced `new TextEncoder().encode(JSON.stringify(request))` with an in-place UTF-8 byte-size estimator (`utf8ByteLength` / `assertWithinLimit`), eliminating a redundant 32 MiB `Uint8Array` allocation on Web and eliminating double `JSON.stringify` before Tauri `invoke()` on Desktop.
  - **Lightweight Submission Deduplication Fingerprinting (`src/renderer/shared/conversation/lifecycle.ts`)**: Replaced `JSON.stringify([prompt, images, videos, options.documentAttachment])` in `sendPrompt` with a compact head/tail + byte-length `uriFingerprint` signature.
  - **O(1) Per-Chunk HTTP Request Framing (`crates/mint-core/src/api_server.rs`)**: Added `parse_http_request_framing` and `parse_request_line` so the Web API server parses `\r\n\r\n` and `Content-Length` once on the header slice (`&request_bytes[..header_end]`) instead of running `String::from_utf8_lossy` across the accumulated body on every 8 KB socket read.
  - **Non-Blocking Async SQLite IPC & Single-Connection Expiry (`src-tauri/src/commands/conversation.rs`, `crates/mint-core/src/agent/memory.rs`)**: Converted Desktop SQLite Tauri commands (`get_conversation_snapshot`, `get_conversation_changes`, `list_chat_sessions`, `save_interaction_agent_activity`, etc.) to `async fn` backed by `tokio::task::spawn_blocking`, and reused the open SQLite `Connection` for `expire_abandoned_turns` inside `conversation_changes`, `latest_conversation_sequence`, and `conversation_snapshot`.
- **Phase 3 — Web API Non-Blocking I/O, GPU Compositing Relief, Composer Isolation & Adaptive Polling**:
  - **Non-Blocking Web API Server Routes & Low-Latency Socket Tuning (`crates/mint-core/src/api_server.rs`, `crates/mint-core/src/api_server/routes/sessions.rs`, `status_health.rs`, `skills_subagents.rs`, `profile_oauth.rs`)**: Offloaded all synchronous SQLite `MemoryStore` queries and system subprocesses to `tokio::task::spawn_blocking`, and enabled `TCP_NODELAY` on incoming TCP connections to eliminate 40ms delayed-ACK packet pauses.
  - **Scrollable Chat Card GPU Compositing Relief (`MermaidCard.tsx`, `StockCard.tsx`, `CalculationCard.tsx`, `ImageSearchCard.tsx`, `ImageGenCard.tsx`)**: Removed inline `backdropFilter: 'blur(16px)'` from in-stream message cards, eliminating repeated full-pass GPU blur readbacks during virtualized scrolling on Linux WebKitGTK and mobile Web browsers.
  - **Composer Input Draft State Isolation (`useConversationCoordinator.ts`, `MintDashboard.tsx`, `ChatPanel.tsx`)**: Decoupled message draft updates from `MintDashboard`'s snapshot selector and connected `ChatPanel`'s composer via `useComposerDraft`, ensuring that typing in the chat input never triggers re-renders of `MintDashboard`, `DashboardSidebar`, or top-level modals.
  - **Adaptive Conversation Lifecycle Polling (`lifecycle.ts`, `lifecycle.test.ts`)**: Added `conversationPollIntervalsMs` to relax polling from 1.5s/5s down to 5s/15s when idle, dynamically re-arming to 1.5s/5s whenever an active prompt runs.
- **Unit Test Coverage**:
  - Added tests in `transport.test.ts`, `lifecycle.test.ts`, `ChatPanel.test.tsx`, `markdown.webImages.test.tsx`, `BackgroundTerminals.test.tsx`, `ModUi.test.tsx`, and `crates/mint-core/src/api_server.rs`.

## Tools Surface Image Preview & Routine Polling Log Filter (2026-10-25)

- **Tools Surface In-Panel Image Preview**:
  - **In-Panel Image Preview (`src/renderer/shared/components/SessionSources.tsx`)**: Clicking an image attachment in the Sources list now opens the image directly in an artifact preview tab within the Tools Surface side panel instead of navigating away to `/library` and closing the panel.
  - **High-Resolution Library Fetch**: Resolves the original full-resolution image from Library storage via `libraryPlatform.readLibraryFile` as an object URL, gracefully falling back to thumbnail data URIs if unavailable.
  - **Unique Tab Tracking**: Formats preview artifact paths as `sources/${source.id}/${source.name}`, enabling separate preview tabs even when multiple attachments share the same filename.
  - **Platform Parity Across Desktop and Web (`MintDashboard.tsx`)**: Wired `onOpenArtifact={openArtifactSurface}` into `SessionSources` across Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`).
- **Routine API Polling Log Suppression (`crates/mint-core/src/api_server.rs`)**:
  - Added `/api/conversation-sources` to the `should_log_api_route` exclusion list in `mint-core`, silencing routine 2-second polling log spam in the `mint web` terminal while preserving all operational API logs.
  - Added unit test assertion in `api_server::tests::test_should_log_api_route_exclusions`.
- **TypeScript & CSS Standards**:
  - Added `react-dom` module declaration in shared and environment definition files (`env.d.ts`).
  - Added standard `line-clamp: 2;` property in `session-sources.css`.

## Plan Mode Proactive Subagent Reconnaissance (2026-10-25)

- **Mandatory Proactive Subagent Reconnaissance in Plan Mode**:
  - **System Prompt Rules (`crates/mint-core/src/prompts/agent.rs`)**: Updated Rules 8b and 8c instructing the agent that when in Plan Mode (or asked to plan, structure, or audit), it must ALWAYS delegate broad investigation, facts gathering across multiple files, and architecture verification to parallel read-only subagents (`dispatch_agents` with base `explorer`) instead of recursively reading dozens of files in the main turn.
  - **Read-Only Profile Filtering (`crates/mint-core/src/prompts/tool_catalog.rs`)**: `dispatch_agents` is now enabled in Plan Mode with a curated set of read-only base profiles (`explorer`), and its description specifically directs the model to perform parallel multi-file reconnaissance before finalizing the plan.
- **Engine-Level Zero Side-Effects Safety Clamp**:
  - **Plan Mode Base Profile Enforcement (`crates/mint-core/src/swarm/batch.rs` & `orchestration/mod.rs`)**: In Plan Mode, the swarm batch planner automatically clamps all task base profiles to `DEFAULT_BASE` (`explorer`), guaranteeing that even if a model requests a write-capable profile, it is strictly sandboxed as read-only.
- **Interface Parity Across CLI, Desktop, and Web**:
  - **Full-Screen TUI (`crates/mint-cli/src/interactive/chat_tui.rs`)**: Docked `Running Subagents` panel animates and displays live reconnaissance subagents right above the `[Plan]` prompt and composer.
  - **Desktop and Web GUI (`src/renderer/shared/`)**: `SubagentStatusStrip` and `SubagentDagView` render subagent progress and DAG inspection seamlessly while in Plan Mode.
- **Unit and Regression Test Coverage**:
  - Added unit test in `tool_catalog.rs` verifying `dispatch_agents` is offered in Plan Mode with read-only profiles.
  - Added unit test in `batch.rs` verifying that Plan Mode forces `explorer` base even when `general-purpose` is requested.
  - Added regression assertion in `review_regressions.rs` verifying `plan_mode_allows` allows `dispatch_agents`.
  - Added TUI unit test in `chat_tui.rs` verifying swarm panel renders cleanly with the `[Plan]` indicator.

## Clean Subagent Transcript Isolation & Hallucination Guard (2026-10-25)

- **Clean Isolated Transcript (Claude Code Experience)**:
  - **Full-Screen TUI (`crates/mint-cli/src/interactive/chat_tui.rs`)**: Subagent micro-tool calls (`read_file`, `search_code`, errors, retries) and child context compactions are now cleanly excluded from the main conversational transcript and tool card feed. Child progress streams strictly to the dedicated docked `Running Subagents` panel, preserving a focused, calm main chat area where only parent orchestrator tools (`dispatch_agents`) and final summaries appear.
  - **Main Timeline Live Status Scoping (`crates/mint-cli/src/agent.rs`)**: Subagent context compactions no longer hijack the main agent's thinking bar or print spurious compaction messages to the main conversation timeline.
  - **Desktop & Web GUI Parity (`src/renderer/shared/utils/agentActivity.ts`)**: Segregated subagent tool calls and compactions from the main chat activity drawer; subagent progress and DAG inspection remain cleanly accessible via `SubagentStatusStrip` and `SubagentDagView`.
- **Hallucination & `"null"` Argument Guard**:
  - **Task Planning Sanitization (`crates/mint-core/src/swarm/batch.rs`)**: Pre-emptively rejects tasks where `instruction` is empty or `"null"` with a clear diagnostic reason, and sanitizes role seeds to prevent invalid agent creation.
  - **TUI Action Formatter (`chat_tui.rs`)**: Filtered out `"null"` path and command values from subagent tool descriptions and labels, avoiding misleading `[role] null` indicators.
- **Unit and Interface Test Coverage**:
  - Added transcript isolation assertions in `chat_tui::swarm_panel_tests::record_tool_progress_updates_swarm_state`.
  - Added unit test in `batch::tests::null_or_empty_instruction_is_rejected_cleanly`.
  - Added vitest unit test in `agentActivity.test.ts` verifying subagent event isolation in renderer.

## Subagent Swarm Rebuild & Live Status Strip (2026-10-24)

- **Swarm Execution Engine (`crates/mint-core/src/swarm/`)**: Rebuilt subagent orchestration from scratch to support parallel multi-agent swarms via `dispatch_agents(tasks: [{ role, base, instruction }])`.
  - **Isolated Concurrency & Lanes**: Read-only tasks run concurrently up to a configurable parallel cap (`swarm_max_parallel`, default 4, range 1–8). Write-capable tasks serialize safely in a dedicated write lane to prevent filesystem races.
  - **Scoped Permissions & Profiles**: Subagent tasks inherit from base profiles (`explorer` by default, read-only). Child agents cannot mutate files or spawn nested swarms unless explicitly granted write permissions.
  - **Bounded Dossiers**: Each subagent returns a compact, capped structured dossier (`status`, `steps`, `summary_preview`, `findings`, `artifacts`) to prevent context bloating in the main conversation.
  - **Backward Compatibility**: Fully preserves historical transcript rendering and handling for older `dispatch_subagent` calls.
- **Full-Screen TUI Swarm Panel (`crates/mint-cli/src/interactive/chat_tui.rs`)**:
  - Added a dedicated, live swarm panel docked below the composer:
    - Real-time animated spinner (`⠋`), agent label badges (`[recon]`, `[config]`), live action descriptions, and right-aligned elapsed timing (`· 14s`, `· done 3s`, `· failed 5s`).
    - Handles overflow with `+N more subagents...` indicator when more than 5 agents run in parallel.
    - Responsive compact single-line mode on short terminal displays (< 24 rows).
    - Automatically clears when the user submits their next prompt.
- **Desktop & Web GUI Parity (`SubagentStatusStrip.tsx` & Settings)**:
  - Created shared `SubagentStatusStrip` docked above the composer in `ChatPanel.tsx` across Desktop and Web.
  - Clicking any subagent row or the strip focuses and expands the subagent activity drawer and DAG view (`SubagentDagView`).
  - Added a configurable `Max Parallel Subagents` slider (1 to 8) to `Settings > Subagents & Swarm Execution` in `AgentsTab.tsx`.
- **Comprehensive Test Coverage**:
  - Added 31 unit tests in `crates/mint-core/src/swarm/` covering permissions, labels, dossiers, and batch runner.
  - Added 6 unit tests in `crates/mint-cli/src/interactive/chat_tui.rs` covering TUI rendering, overflow, compact mode, and tool progress lifecycle.
  - Added vitest unit tests in `src/renderer/shared/components/SubagentStatusStrip.test.tsx` and `SubagentDagView.test.ts`.

## Hard-Truncate Context Compaction Fallback (2026-10-24)

- **Deterministic Hard-Truncate on Compaction Failure**: Added `hard_truncate_native_messages` in `crates/mint-core/src/orchestration/context_render.rs`. When LLM-based compaction summarization encounters provider errors, timeouts, rate limits (HTTP 429), or context length exceeded, the agent loop no longer retains oversized messages and crashes with a generic `This turn failed.` turn failure. Instead, it seamlessly applies a structural hard-truncation fallback that extracts key tool calls and parameters from older steps into a compact synthetic summary pair while preserving the initial user task, the most recent steps (`COMPACTION_KEEP_RECENT_STEPS`), and strict alternating role semantics.
- **Pre-emptive Transcript Safeguard**: Bounded individual `ContentBlock::ToolResult` outputs inside the compaction summarizer prompt to `MAX_TRANSCRIPT_TOOL_RESULT_CHARS = 2,000` via `truncate_for_context`. Prevents large tool outputs (such as massive file contents, large diffs, or verbose web search results) from blowing up the summarizer prompt and causing timeouts or context rejections in the first place.
- **Clear Lifecycle & Status Notification**: Updated `report_context_compaction` to report structural fallback completions with a transparent notification (`Context compacted ...; hard-truncate fallback (...)`) across CLI Full-Screen TUI, Desktop, and Web UI.
- **Comprehensive Unit Testing**: Added unit tests covering oversized tool result truncation, structural role-alternation retention, and automatic fallback activation upon provider failure.

## Web API Server Workspace Parity & Scoping (2026-10-24)

- **Parity with Desktop Workspace Resolution**: Added `resolve_agent_root` in `crates/mint-core/src/api_server.rs` to validate and resolve `workspace_path` provided by the client, canonicalizing valid existing directories and falling back to `current_dir()`.
- **Eliminated Involuntary CWD Overwrites**: Updated `POST /api/chat-stream` in `routes/chat.rs` and `run_web_agent_loop` in `api_server.rs` to pass the resolved client workspace root to `orchestrate_agent_loop`. Agent mode conversations started in a selected project folder (e.g. `Cosplay`) in Mint Web now correctly persist their chosen workspace and remain under their intended project in the sidebar instead of being overwritten to the server process's launch directory.
- **Unit and Regression Test Coverage**: Added comprehensive test coverage verifying `resolve_agent_root` for `None`, empty, non-existent, and valid directories.

## Safe Temp Directory Access for Agent Tools (2026-10-23)

- **Symmetric Temp Directory Read/View Access**: Added `std::env::temp_dir()` and `/tmp` to default `allowed_paths` in `MintConfig`.
- **Decoupled Tool Inspection from Strict Workspace Root**: Updated `view_image` and `read_file` to resolve paths via `agent_read_path` instead of hard workspace-root-only bounding across all tool entry points (both `tools::project` and the primary orchestration interception loop in `orchestration/mod.rs`). Agents can now inspect screenshots, artifacts, and output files created in `/tmp` by shell tools (`chrome --headless --screenshot`, `ffmpeg`, scripts) without `path is outside workspace` errors.
- **Persistent Temp Bind Mount in Linux Sandbox**: Replaced isolated ephemeral `--tmpfs /tmp` in Bubblewrap (`bwrap`) with host bind-mounting (`--bind /tmp /tmp`), ensuring files written to `/tmp` in one shell command remain persistent across commands and can be inspected by agent tools.
- **Safety Policy Temp Invariant**: Enhanced `assert_path_capability` and `writable_roots` to always recognize and permit `temp_dir()` for read/write even when deserializing existing user configs from disk that omit `/tmp`.
- **Preserved Strict Security Guardrails**: Sensitive files (`.env`, `~/.ssh`, `id_rsa`, `.gnupg`) remain strictly blocked by `assert_path_capability`, and code mutation tools (`write_file`, `apply_patch`) remain anchored to the project workspace root.

## Context & Prompt Cache Optimization (2026-10-23)

- **Rolling Image Eviction in LLM Context**: Implemented `prune_historic_images_from_native_messages` with `MAX_RETAINED_IMAGE_STEPS = 2`. Retains full vision pixels for the initial 2 steps of inspection and drops historical pixels in subsequent turns, eliminating runaway cumulative prompt cache growth and cutting token volume in vision-heavy agent tasks by over 90% while keeping durable original artifacts intact on the UI.
- **Model Vision Image Downscaling**: Capped model vision payloads in `tool_images.rs` to `MAX_MODEL_DIMENSION = 1280px` (JPEG Quality 85%). High-resolution captures (2K/4K/Retina) are now scaled down before base64 encoding to reduce vision patch tiles by 40–60% per image, while preserving uncompressed full-resolution originals in the artifact store.
- **Web Content Sanitization & Whitespace Cleanup**: Refactored `read_page_text` to strip non-content noise (scripts, styles, noscript, inline SVGs, canvas, cookie/GDPR banners) and collapse redundant whitespace while strictly preserving semantic headers, navigation menus, and articles.
- **Adaptive Context Compaction Trigger**: Added an adaptive threshold (`ADAPTIVE_COMPACTION_TOKEN_CEILING = 40,000` at $\ge 20$ steps) so long-running agent workflows compact proactively before approaching provider context window limits.

## API Server Log De-noising & Frontend Polling Optimization (2026-10-21)

- **Server-Side API Log Filtering (`should_log_api_route`)**: Added high-frequency polling routes (`/api/background-jobs`, `/api/mods`, `/api/models`) to the route exclusion filter in `crates/mint-core/src/api_server.rs`. Completely eliminates repetitive terminal spam from routine UI polling while ensuring standard operational and administrative API calls (`/api/library`, `/api/system-info`, etc.) remain logged.
- **Eliminated Duplicate Request in `BackgroundTerminals`**: Replaced the parallel secondary `backgroundPlatform.list(workspacePath)` call with an in-memory client-side filter (`filterScopedJobs`). All background job scopes are now derived from a single fetch, cutting network requests in half.
- **Throttled Polling Frequencies & Visibility Pause**:
  - Relaxed `BackgroundTerminals` list polling interval from 1,000ms to 3,000ms.
  - Relaxed `ModsPanel` status polling interval from 1,000ms to 3,000ms, and added `document.hidden` guards with `visibilitychange` listeners to pause polling entirely when tabs/windows are inactive, saving CPU and bandwidth.
- **Model Input Warning Banner Refinement (`McpToolResults`)**: Refactored raw text warning notices into an accessible `.mcp-warning-banner` with Lucide `Info` icon and soft muted styling. Added `.mcp-results-compact` when only warnings are present to remove the oversized outer bordered box, maintaining strict design token parity (`var(--bg-surface-soft)`, `var(--border)`, `var(--text-muted)`, `var(--interactive-fg)`) across Desktop and Web without hardcoded colors.

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
