# Release Notes - Mint Agent v1.18.0

## Headless Browser Automation & Anti-Loop Engine (CLI, Desktop, and Web)
- **Headless Chromium via Shell Playbook**: Replaced agent-facing `browser_*` tools and the headed Mint Browser UI (`mint auto`, `BrowserShell.tsx`, `BrowserSuggestionsPopup.tsx`) with high-reliability headless Chromium shell commands (`google-chrome --headless=new --dump-dom`, `--screenshot`) managed through `run_shell`.
- **System Default Browser Previews**: HTML artifact previews open directly in the user's default system browser across CLI (Full-Screen TUI / Classic), Desktop, and Web, eliminating custom embedded browser windows.
- **Anti-Loop Repetition Guard & SHA-256 Image Dedup**:
  - Multi-layered repetition guard in `mint-core` tracks execution outputs over a rolling 8-execution window, intercepting stuck loops ($\ge 3$ identical results) with actionable `InterceptStuckLoop` guidance.
  - Image artifact viewer computes SHA-256 hashes on `view_image` calls, suppressing repeated analysis of identical screenshots and steering the agent toward forward progress.
- **Deterministic Headless Workflow Runner (`mint browser run`)**: Retained the 14-action deterministic browser workflow engine for scripted multi-step automation and testing, strictly headless with per-run temporary user profiles and artifacts stored under `.mint/browser-runs/`.
- **Diagnostic Slash Command (`/browser` & `/browser status`)**: Streamlined `/browser` across CLI TUI, Desktop, and Web to report detected Chromium binary paths, version info, and sandbox readiness.
- **Information Sufficiency & Follow-Up Search Gate**: Prevented over-agentic crawling and runaway token consumption by enforcing an Information Sufficiency rule on web searches (stopping and synthesizing immediately once target links/answers are found), requiring interactive user confirmation via `ask_user` before conducting follow-up searches on incomplete topics, and restricting headless Chrome strictly to explicit user requests or unblocked direct page reading.
- **Clean Architecture Sweep**: Fully removed `mint auto` CLI command, headed Tauri browser capabilities, port 9222 takeover logic, and legacy browser-mode toggles across Desktop and Web settings.
