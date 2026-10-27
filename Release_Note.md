# Release Notes - Mint Agent v1.17.0

## Desktop & Web UI / UX Enhancements
- **Resilient Markdown Image Auto-Healing & Load Error Fallback**:
  - **Local Image Proxy & Disk-Based Cache (`/api/proxy-image`)**: Implemented a secure first-party local image proxy with LRU disk caching in `crates/mint-core/src/media/image_proxy.rs` (`~/.config/mint/cache/image-proxy`). External images blocked by client-side ad-blockers (e.g. uBlock Origin, Brave Shields blocking third-party social trackers like `pbs.twimg.com`) or cross-origin restrictions automatically fallback to the local proxy via `resolveApiProxyUrl`. Features rigorous SSRF protection (blocking localhost, link-local, and private IPv4/IPv6 networks), 7-day TTL, 25MB single-image safety limit, and 200MB cache capacity with automated LRU cleanup down to 150MB.
  - **Clean Storage Layout & Zero-Loss Avatar Fallback**: Reorganized `~/.config/mint/` directory roles into clear partitions: `Library/` for permanent user assets and chat media, `cache/image-proxy/` for temporary web cache (with automated cleanup of orphaned legacy cache from previous versions), and `profile/` for user avatars. Features path traversal protection and seamless Dual-Lookup avatar resolution in `resolve_avatar_file_path` (`api_server/routes/auth.rs`) that automatically falls back to legacy `Pictures/profile/` ensuring existing avatars are never lost upon upgrading.
  - **Auto-Heal Unclosed Markdown Images**: Added regex auto-repair in `readableAssistantText` and `autoHealMarkdownImages` (`markdown.tsx`) to detect when LLMs omit the closing parenthesis `)` on long image URLs at the end of a line (e.g. `![alt](https://...`), automatically restoring the closing `)` so the image parses and displays properly instead of leaking as raw text.
  - **Twitter/X Banner Dimension Auto-Normalization**: Added automatic dimension normalization (`/1500x500`) across both Backend search pipeline (`normalize_web_search_image_url` in `mint-core`) and Frontend media resolver (`normalizeTwitterImageUrl` in `markdown.tsx`), repairing naked `pbs.twimg.com/profile_banners/<id>/<timestamp>` URLs so they resolve to full-resolution images instead of throwing HTTP 404 errors.
  - **Graceful Image Load Error Fallback**: Replaced silent image hiding (`display: none`) on `onError` with a styled card fallback (`ChatMediaImage`) that informs the user with an "Image failed to load" indicator and provides an "Open link" button to view the source URL in a new tab.
- **Provider Brand Badges in Chat Message Footer**: Replaced the static colored dot in the message footer provider chip (`{provider} / {model}`, e.g. `deepseek / deepseek-flash`) with crisp 13px vector brand logos (DeepSeek whale, Gemini sparkle, Anthropic mark, OpenAI vortex, Groq, Ollama, OpenRouter, Mistral, Perplexity, Together, Qwen, Hugging Face, LM Studio) and full custom provider logo/emoji support across both Desktop and Web UI.
- **Compact Tabbed Custom Provider Logo Picker & Crop Integration**: Redesigned the Custom Provider Logo setting in General Tab with a modern 3-tab picker across Desktop and Web:
  - **AI Presets Tab**: One-click selection of 13 official vector SVG marks (DeepSeek, Ollama, OpenRouter, Groq, Mistral AI, Together AI, Perplexity, LM Studio, Hugging Face, OpenAI, Anthropic, Gemini, Qwen) bundled locally and stored as resilient SVG data URLs.
  - **Upload & Crop Tab**: Full support for SVG, PNG, WebP, and JPEG files with drag-and-drop dropzone, client-side auto-downscaling, direct native file input trigger, and integrated **Crop & Rotate popup modal** (`AvatarCropModal`) allowing users to zoom, pan, rotate, and crop custom provider badges.
  - **URL & Emoji Tab**: Supports direct image web URLs (`https://...`) and quick-pick emoji badges (🤖, 🧠, ⚡, 🐋, 🦙, etc.) converted into crisp vector SVG data URLs.
  - **Direct Index-based State Updates**: Resolved silent saving failures by updating custom providers directly by index instead of fragile real-time slug ID lookups.
- **Studio Dark Theme (Antigravity Studio Palette)**: Introduced the new "Studio Dark" theme preset and core color mode across Desktop and Web UI, modeled directly after modern dark IDEs like Google Antigravity and Cursor:
  - **Base Canvas**: `#101010` (RGB: 16, 16, 16) ultra-deep black for main chat area.
  - **Sidebar & Panels**: `#161616` (RGB: 22, 22, 22) muted charcoal for navigation bar and side drawer.
  - **Surfaces, Inputs & Message Bubbles**: `#1c1c1c` (RGB: 28, 28, 28) for the composer input container and user chat bubbles with high legibility.
  - **Active Items & Focus Highlights**: `#323232` (RGB: 50, 50, 50) for selected sidebar projects/sessions and hover interactions.
  - **Typography**: `#e1e1e1` primary text with `#8e8e8e` secondary/muted labels and icons.
  - **Accent**: `#10b981` mint green highlights for focal points, badges, and status indicators.
- **Charcoal Onyx 60/30/10 Theme Architecture**: Refined dark themes (Dark Mint, Midnight, Dark Monochrome) across Desktop and Web to adopt the deep Charcoal Onyx palette (`#17171a` / `#222227` / `#121215`): 60% deep black base canvas (`#0a0a0c`, `#000000`), 30% deep charcoal surfaces (`#121215` – `#17171a`) for textboxes, cards, dialogs, and secondary buttons, and 10% mint green (`#10b981`) accent highlights for focused elements, active indicators, and primary CTA buttons.
- **Sleek Gray Buttons with Mint Hover Affordance**: Secondary and action buttons (`.btn-secondary`, `.management-action-btn`, `#chat-tool-btn`, `#screen-capture-btn`) feature neutral gray backgrounds with white text and a subtle mint-glow border upon hover, preventing visual fatigue while maintaining brand coherence.
- **Removed Standalone Dark Slate Gray Preset**: Streamlined the theme catalog by removing the separate `Dark Slate Gray` preset in favor of embedding the sleek slate-gray surface layer directly into all dark themes.
- **Configurable Agent Step Limit & `/steps`**: Added `/steps` slash command and graphical step budget controls in Settings across Desktop, Web, and CLI (Full-Screen TUI), allowing users to adjust maximum tool execution steps per turn (10–200, default 40).
- **Trajectory Export & SWE-bench Parity**: Added one-click "Export Trajectory" capability in the Run Summary Dashboard across Desktop and Web UI, downloading canonical SWE-bench / Agent Protocol v1.0 execution transcripts (`{runId}-trajectory.json`) containing complete step sequences, thoughts, observations, token metrics, and duration.
- **Diagnostic Self-Correction Visuals**: Enhanced the activity drawer to detect and render `[Self-Correction Guidance]` blocks with dedicated high-contrast callout styling, making the agent's self-healing loop clearly visible to developers.
- **Resilient Activity Drawer Rendering**: Historical "Working through task" activity drawer and file change logs now remain fully accessible and expandable across completed, failed, and interrupted turns, allowing users to inspect executed tools and diagnostics after errors or cancellations.
- **Run Summary Badge Accuracy**: Updated telemetry outcome classification to render casual chat and non-verified turns (`PARTIAL` / `COMPLETED`) as green `Completed` badges instead of erroneously showing red `Failure` chips.
- **Titlebar Visibility**: Preserved Desktop titlebar, window controls (minimize/maximize/close), drag region, and application menus while Settings modal is open.
- **Settings Hierarchy**: Reordered navigation tabs to position General directly above Notifications in the Workspace section across Desktop and Web.
- **Streamlined Composer**: Removed static AI gateway disclaimer text and lowered the chat input closer to the bottom edge.
- **External Link Interceptor**: Added global link interception for Desktop to open image previews, search source citations, and chat URLs directly in the system default browser.
- **Activity & Preview Cards**: Added inline viewed image previews with click-to-zoom modals, and enabled in-panel image viewing inside the Tools Surface without navigating away.
- **TUI & Terminal Polish**:
  - **Compact TUI Web Search Sources & Ctrl+O Integration**: Streamlined web search citations in Full-Screen TUI transcript to display at most 5 primary sources (`MAX_TUI_WEB_SOURCES`), appending an interactive `… [+N more sources] · Ctrl+O details` hint whenever additional sources exist, preventing screen clutter while allowing instant inspection of all results and snippets via the tool viewer modal (`Ctrl+O`).
  - **Unified Command Badges**: Unified command status badges (`[Command]`, `[Check]`, exit codes) with mint-green accents and suppressed duplicate command output blocks in chat.

## Performance & Frontend Optimization
- **Re-render Isolation**: Decoupled message draft input state from dashboard coordinators, preventing full-page re-renders while typing.
- **Component Splitting**: Isolated streaming elapsed timers into dedicated leaf components and lazy-loaded secondary modals and rich cards.
- **Zero-Allocation Transport**: Replaced 32 MiB buffer allocations with in-place UTF-8 byte estimators and lightweight fingerprinting for attachments.
- **Adaptive Polling**: Switched conversation changes, background jobs, and mods polling from fixed intervals to adaptive active/idle cadences, pausing on inactive tabs.
- **GPU Compositing Relief**: Removed backdrop blur filters from scrollable chat cards to ensure smooth virtualized scrolling on Linux WebKitGTK and mobile web.

## Core Engine & Web API Server
- **Step Budget Warning & Graceful Wrap-Up Synthesis**: Injected proactive step budget warnings at `max_steps - 5` into agent prompt context and tool observations. Replaced hard turn failures at step limits with a Graceful Wrap-Up Synthesis pass (`tools: None`), compelling the LLM to consolidate and deliver a comprehensive final answer rather than terminating prematurely.
- **Fault-Tolerant Tool Call Parsing**: Hardened tool-call input decoding against provider quirks: strips markdown fences and DeepSeek DSML/thinking tokens, recovers from trailing commas, prunes hallucinated null fields, unpacks double-wrapped arguments, and accepts command aliases (`cmd`, `bash`, `sh`, `exec`, `script`, `code`) and raw shell strings.
- **Structured Self-Correction Diagnostics Engine**: Built intelligent diagnostic guidance for tool failures (`diagnostics.rs`). Automatically identifies indentation/whitespace mismatch drifts on `apply_patch` failures, calculates sliding-window similarity to pinpoint exact drifted line numbers and code snippets, guides ambiguous hunks with line-numbered occurrences, and extracts structured compiler/runtime error culprits for Rust (`cargo`), Python (`Traceback`), and TypeScript (`tsc`) with actionable next-tool recommendations.
- **Standardized Trajectory Schema v1.0**: Established an industry-standard, versioned Trajectory Schema (`StandardTrajectoryRun` v1.0) with typed steps, thoughts, tool inputs, observations, execution statuses, and step-level token usage (`StepTokens`). Supports full roundtrip serialization, deterministic replayability, on-the-fly reconstruction from historical SQLite interaction memories (`export_trajectory_json`), and export via `/trajectory` in CLI TUI, Desktop, and Web UI.
- **Line-Aware Head-Tail Truncation**: Integrated industry-standard dual-bound Head-Tail Truncation harness (`head_tail_truncate`) preserving initial command execution context (first 35 lines) and trailing compiler errors, stack traces, and exit summaries (last 75 lines) while omitting verbose intermediate logs with structured diagnostic markers. Cuts token overhead by up to 70% and prevents loss of root-cause error information.
- **Repetition & Stuck-Loop Guard**: Implemented an automated agent loop governor (`RepetitionGuard`) that calculates canonical action fingerprints, warns on consecutive duplicate executions, intercepts stuck loops (3+ consecutive identical calls or oscillating cycles A->B->A->B), and delivers structured self-correction guidance without killing the turn. Automatically resets upon workspace-modifying operations.
- **Context Compaction Natural Role Alternation**: Converted synthetic compaction summaries from internal tool-call representations to compliant alternating dialogue (`Assistant` summary + `User` continuation prompt). Resolves post-compaction agent loop crashes (`HTTP 400 Bad Request`) on reasoning-enabled models like DeepSeek (missing `reasoning_content`) and Gemini (missing `thoughtSignature`).
- **OpenAI & DeepSeek Serializer Safeguard**: Added automatic `reasoning_content` fallback when serializing historical tool-calling assistant messages in OpenAI-compatible payloads, ensuring strict API protocol compliance during thinking mode.
- **Non-Blocking SQLite & I/O**: Offloaded SQLite persistence queries and system subprocesses to background worker threads (`spawn_blocking`).
- **Low-Latency Networking**: Enabled `TCP_NODELAY` on incoming HTTP sockets and implemented single-pass HTTP header framing to prevent redundant body re-scanning.
- **Workspace Parity**: Standardized agent root resolution between Web API and Desktop to eliminate involuntary working directory overwrites.
- **Log Suppression**: Filtered routine high-frequency polling endpoints from terminal logs to keep server output clean and operational.

## Subagents, Swarm Orchestration & Plan Mode
- **Parallel Swarms**: Engineered multi-agent swarm runner (`dispatch_agents`) supporting concurrent read-only tasks (1–8 workers) and serialized write lanes.
- **Plan Mode Reconnaissance**: Mandated read-only parallel subagent exploration for multi-file discovery before plan finalization.
- **Transcript Isolation**: Segregated subagent micro-actions and child compactions from the main timeline into a live docked status strip and DAG view.
- **Deterministic Compaction Fallback**: Added structural hard-truncation fallback when LLM summarization fails or hits token limits, preventing agent loop crashes.
- **Context Cache Optimization**: Introduced rolling vision image eviction in LLM context (retaining latest 2 steps) to cut token usage by up to 90%.

## Agent Tools, Browser Automation & Sandbox
- **Safe Temp Directory Access**: Allowed reading and viewing `/tmp` and OS temporary paths across agent tools with persistent Linux sandbox mounts.
- **Browser Automation**: Integrated isolated headless Chromium automation engine supporting ordered navigation, clicks, fills, screenshots, and durable artifacts.
- **Evidence-Based Reliability**: Replaced unconditional duplicate-command skips with typed execution states and evidence-based completion checks.

## Mint Companion & Voice Input
- **Live2D Desktop Companion**: Redesigned translucent glass interface with full-screen character view, OS-wide cursor gaze tracking, and window dragging.
- **Synchronized Speech**: Centralized text-to-speech audio preparation with mouth sync and cross-process playback leasing.
- **Voice Input Improvements**: Microphone dictation now appends directly to the active draft without auto-submitting.
