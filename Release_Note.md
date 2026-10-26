# Release Notes - Mint Agent v1.17.0

## Desktop & Web UI / UX Enhancements
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
