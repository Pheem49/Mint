# Release Notes - Mint Agent v1.17.0

## Desktop & Web UI / UX Enhancements
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
- **TUI & Terminal Polish**: Unified command status badges (`[Command]`, `[Check]`, exit codes) with mint-green accents and suppressed duplicate command output blocks in chat.

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
