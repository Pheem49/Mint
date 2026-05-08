
# Mint Release Notes

## v1.4.0 - The "Intelligence & Collaboration" Update

Mint 1.4.0 is the most significant release to date — Mint is now **project-aware**, **persona-driven**, and **context-sensitive**. With persistent memory, real-time streaming, and a full suite of system tools, Mint has evolved from a simple chat interface into a true autonomous assistant.

### ✨ New Features

* **⚡ Streaming Responses (Gemini):**
    * Real-time interaction! Messages now appear word-by-word as they are generated, rather than waiting for the full response.
    * Uses Gemini SDK's `sendMessageStream()` with progressive JSON buffer extraction.

* **🧠 Long-Term Memory & Caching:**
    * Mint now remembers you across sessions via a local SQLite store.
    * **Personalization:** Tracks your language preferences, active projects, and common interaction patterns to inject relevant context into every prompt.
    * **Response Caching:** Instant answers for repeated queries! Saves API quota and reduces latency by caching common AI responses.

* **🤖 Multi-Agent Orchestrator:**
    * **Persona Switching:** Switch between specialized agents like **Coder**, **Researcher**, **Creative**, **Manager**, and **Reviewer** using the `/agent <type>` command.
    * **Review Mode:** Use `/review` to trigger a second-pass critique of any AI response by the specialized Reviewer agent.

* **📂 Workspace Management:**
    * Register your project directories with `/workspace add <name> [path] [instructions]`.
    * Mint automatically detects when you are working inside a registered workspace and applies project-specific instructions to the AI context.

* **📊 System Monitor & Notifications:**
    * **Stats:** Use `/stats` to view real-time CPU load, Memory usage, and Disk space.
    * **OS Notifications:** Receive system alerts when autonomous tasks finish or when Mint proposes a bash command, keeping you informed even when the terminal is in the background.

* **🎵 Spotify Plugin (Complete Edition):**
    * Full control via `playerctl` (no OAuth required):
    * **Playback:** `play`, `pause`, `stop`, `next`, `previous`.
    * **Now Playing:** View current track, artist, and album status.
    * **Volume & Shuffle:** Fine-grained control over audio levels and playback modes.
    * **Search:** Quick browser-based search integration.

### 🛠️ Improvements & Refactoring

* **Smarter API Key Detection:** Automatically detects and skips placeholder API keys (e.g., "your_key_here") to prevent unauthorized errors.
* **Unified System Prompt:** Refactored `buildSystemPrompt()` to centralize MCP tools, plugin descriptions, and workspace context across all AI providers.
* **Smart Routing Priority:** Code tasks now prioritize your configured `aiProvider` while gracefully falling back to the best available model.

### 🧪 Testing & Stability

* **Robust Test Suite (69 tests passed):**
    * Full coverage for `config_manager`, `memory_store`, `workspace_manager`, `agent_orchestrator`, `system_monitor`, and `spotify` plugins.
    * Implemented strict test isolation with temporary databases and configurations to protect production data.

---

## v1.3.0 - The "Agent & Plugin Power-Up" Update

Mint 1.3.0 introduced the foundation for multi-agent capabilities and a more flexible plugin architecture.

### ✨ New Features
* **Agent Framework:** Initial support for switching between different AI specializations.
* **Plugin System 2.0:** Dynamic loading/unloading of plugins without restarting the CLI.
* **Contextual Help:** Introduced `/help` command with situation-aware suggestions.

### 🛠️ Improvements & Bug Fixes
* **Performance Boost:** Optimized CLI responsiveness and reduced memory footprint.
* **Better Error Handling:** Improved user-facing error messages with actionable fixes.

---

## v1.2.4 - The "Smart Path & Dynamic Version" Update

Focused on path resolution intelligence and consistent versioning.

### ✨ New Features
* **Smart Path Resolution:** Automatic Home-directory correction and common folder searching.
* **Dynamic Versioning:** CLI versioning is now synchronized directly with `package.json`.

---

## v1.2.3 - The "Smart TUI" Update

Focused on the terminal user interface (TUI) polish and system awareness.

### ✨ New Features
* **System Awareness:** Added reporting for OS, Kernel, and Architecture.
* **Enhanced TUI Layout:** Bubble-Lite message style and smart text wrapping for Thai/English.
* **Mouse Support:** Added scroll wheel support for chat history.

### 🛠️ Improvements & Bug Fixes
* **Terminal Cleanup:** Fixed "garbage" character artifacts on exit.
* **Slash Command Aliases:** Added `/model` as an alias for `/models`.
