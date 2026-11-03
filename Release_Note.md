# Release Notes - Mint Agent v1.18.0

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
