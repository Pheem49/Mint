# Release Notes - Mint Agent v1.17.0

## Desktop & Web UI / UX
- **Resilient Media & Image Proxy**: Added local image proxy (`/api/proxy-image`) with disk LRU caching and SSRF protection to bypass ad-blocker false positives; auto-heals unclosed markdown image links (`![alt](url...`), normalizes Twitter/X banner URLs, and displays a graceful fallback card on load errors.
- **Provider Brand Badges & Logo Picker**: Vector SVG brand logos in chat message footers across 13+ providers; redesigned custom provider logo picker with preset SVGs, emoji selection, and an upload modal with pan/crop/rotate controls.
- **Theme Refresh (Studio Dark & Charcoal Onyx)**: Introduced IDE-inspired "Studio Dark" palette (`#101010`), refined dark themes with balanced Charcoal Onyx styling, and added sleek neutral gray buttons with subtle mint hover glow.
- **Configurable Step Limits (`/steps`)**: Control max tool execution steps per turn (10–200, default 40) via `/steps` slash command or Settings across Desktop, Web, and Full-Screen TUI.
- **Trajectory Export & Diagnostics**: One-click SWE-bench/Agent Protocol v1.0 trajectory export (`.json`); dedicated high-contrast callout styling for self-correction guidance; persistent activity drawer accessible across completed and interrupted turns.
- **Full-Screen TUI Onboarding Wizard (`mint onboard` & `/onboard`)**:
  - Overhauled to native Ratatui alternate-screen wizard with responsive edge-to-edge layout and Mint TUI styling.
  - **Step 1 (Core AI)**: Unified Gemini setup with masked key input (`Ctrl+H`), live model discovery, and integrated model picker.
  - **Step 2 (Services)**: Grouped checklist with arrow navigation, spacebar toggles, select-all (`a`), and invert (`i`).
  - **Step 3 (Configurations)**: Paged per-service cards featuring bundled API credentials and real-time model pickers (Anthropic, OpenAI, DeepSeek, Ollama, etc.).
  - **Step 4 (Summary & Launch)**: Visual configuration overview with direct launch (`Save & Launch Mint`) or exit options.
  - Added `/onboard` slash command and first-run detection across CLI, Desktop, and Web.
- **Terminal Polish**: Compact web search citations (top 5 with `Ctrl+O view web_search` smart jump, matched tool-card ANSI color styling, and structured results formatting), automatic source citation reconstruction on session resume (`/resume`), unified command badges, external browser link handling, and non-blocking desktop titlebar during settings.

## Performance & Frontend Optimization
- **Re-render Isolation**: Decoupled message input state to eliminate full-page re-renders while typing.
- **Component Splitting**: Isolated streaming elapsed timers into leaf components and lazy-loaded secondary modals.
- **Zero-Allocation Buffers**: Replaced 32 MiB buffers with lightweight UTF-8 estimators and compact attachment fingerprinting.
- **Adaptive Polling**: Switched background jobs and status polling to idle-aware intervals that pause on inactive tabs.
- **GPU Compositing Relief**: Removed backdrop blur filters on scrollable chat containers for smooth virtualized scrolling.

## Core Engine & Web API Server
- **Step Budget Warning & Wrap-Up Synthesis**: Proactive warning at `max_steps - 5` and graceful synthesis pass (`tools: None`) ensuring a complete final answer instead of abrupt termination.
- **Fault-Tolerant Tool Call Parsing**: Recovers from Markdown fences, DeepSeek thinking tokens, trailing commas, hallucinated nulls, and shell aliases (`bash`, `sh`, `exec`).
- **Structured Self-Correction Diagnostics**: Diagnostic engine (`diagnostics.rs`) detecting patch indentation drift, line numbering offsets, and compiler errors (Rust, Python, TypeScript) with actionable recovery suggestions.
- **Standardized Trajectory Schema v1.0**: Versioned, typed trajectory runs (`StandardTrajectoryRun`) with step tokens, thoughts, and replay support via `/trajectory`.
- **Head-Tail Truncation**: Dual-bound log clipping (first 35 lines context, last 75 lines error output), cutting token overhead by up to 70% while preserving root-cause errors.
- **Repetition & Loop Guard**: Detects duplicate tool executions and oscillating loops (A→B→A→B), injecting self-correction guidance without aborting the turn.
- **Context Compaction Role Alternation**: Alternating `Assistant`/`User` format for summaries, fixing HTTP 400 crashes on reasoning models (DeepSeek, Gemini).
- **Engine Reliability**: Non-blocking SQLite persistence via `spawn_blocking`, low-latency TCP sockets (`TCP_NODELAY`), standardized workspace path resolution, and terminal API noise suppression for high-frequency Library thumbnails (`/thumbnail`), asset content (`/content`), and MCP artifacts.

## Subagents, Swarm Orchestration & Plan Mode
- **Confirmation-First Planning**: Clarified intent confirmation so conversational acknowledgments ("OK", "understood") don't prematurely trigger code modifications.
- **Parallel Swarms & Reconnaissance**: Concurrent read-only worker pools (1–8 agents) for multi-file project exploration before plan confirmation.
- **Transcript Isolation**: Nested subagent micro-actions and child compactions into a dedicated docked status strip and DAG view.
- **Context Cache Optimization**: Rolling vision window retaining the latest 2 image steps, saving up to 90% in token overhead.

## Agent Tools, Browser Automation & Sandbox
- **Safe Temp Directory Access**: Persistent Linux sandbox mounts for `/tmp` and OS temporary paths across agent tools.
- **Headless Browser Automation**: Isolated Chromium automation engine for navigation, clicks, inputs, screenshots, and durable artifacts.
- **State-Based Execution**: Evidence-backed completion verification replacing unconditional duplicate-command skips.

## Mint Companion & Voice Input
- **Live2D Desktop Companion**: Translucent interface with full-screen character view, OS cursor gaze tracking, and window dragging.
- **Synchronized TTS Speech**: Centralized speech synthesis with real-time lip-sync and audio leasing.
- **Non-Destructive Voice Input**: Microphone dictation appends directly to the active draft without auto-submitting.
