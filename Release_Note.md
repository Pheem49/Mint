# Release Notes - Mint Agent v1.18.0

## Context Compaction & Observation Compression Harness (CLI, Desktop, and Web)
- **File-Backed Output Scratchpad (Cursor / Claude Code Pattern)**: Automatically offloads bulky tool execution logs (> 3,000 characters) to `.mint/scratch/tool_outputs/<timestamp>_<action>_<hash>.log`, presenting the model with clean head/tail previews and an actionable pointer path to prevent token bloat while keeping full raw outputs on-demand.
- **Progressive Observation Ghosting (Three-Tier Aging)**: Implemented deterministic turn-level aging for historic `ToolResult` messages without LLM invocation cost — retaining recent 3 turns in full (Hot Zone), truncating turns 4–10 to concise summaries (Warm Zone), and collapsing older turns (> 10 turns back) into 1-line tombstones (Cold Zone).
- **Semantic Tool Superseding**: Automatically detects file modifications (`apply_code_edits`, `write_file`, `patch_file`, `delete_file`) and replaces earlier `read_file` results targeting those modified paths with compact superseding notices, eliminating stale code hallucinations and saving thousands of prompt tokens.
- **Trajectory Buffer Compaction**: Compacts observation previews for older trajectory items in `rebuild_observation`, keeping the task history bounded and responsive across 50–100+ step autonomous runs.
- **Structured Milestone Compaction**: Upgraded `compact_native_messages` with structured markdown sections (Core Intent, Files Modified & Validated, Key Decisions & Findings, Pending Actions) ensuring critical state and verified paths are never lost during macro compaction.

## Deep Module Architecture & God Files Refactoring (`mint-core`)
- **Chat Provider Seam Decomposition (`agent/chat/`)**: Decomposed the 4,651-line `chat.rs` monolith into a deep, modular structure under `crates/mint-core/src/agent/chat/`. Each provider now owns an isolated seam with its own payload construction, thought signature handling, and response decoding:
  - `gemini.rs` (518 lines): Google Gemini 1.5/2.0 native function calling, thinkingConfig, schema sanitization, and multimodal parts.
  - `anthropic.rs` (318 lines): Anthropic Messages API client, prompt caching breakpoints, and tool response parsers.
  - `openai.rs` (582 lines): OpenAI, OpenRouter, and DeepSeek client with reasoning tokens and audio attachments.
  - `ollama.rs` (321 lines): Ollama native messages and streaming parsers.
  - `custom.rs` (149 lines): Custom HTTP endpoints and Hugging Face adapters.
  - `streaming.rs` (466 lines): Server-Sent Events (SSE) accumulator and progressive thought/chunk streams.
  - `tests.rs` (1,403 lines): Isolated 58-test unit verification suite.
  - `mod.rs` (856 lines): High-level provider-agnostic coordinator, fallback dispatchers (`send_chat`, `stream_chat`), and public types.
- **Orchestration Seam Deconstruction (`orchestration/`)**: Reduced the 6,653-line orchestration god file down to 2,587 lines (a 61% reduction) by isolating discrete responsibilities into cohesive modules:
  - `types.rs` (646 lines): Centralized execution types including `TurnLease`, `TurnStartListener`, `OrchestrationError`, `AskUserOption`, `AgentApproval`, `AgentProgress`, `ToolExecutionRecord`, and `AgentResult`.
  - `decision_parsing.rs` (736 lines): Deep parsing for LLM output, tool call preparation, schema sanitization, and retry tracking.
  - `tools/dispatch.rs` (899 lines): Tool execution router, parallel read-only batch execution, code review gatekeeper, and result appending.
  - `tests.rs`, `mcp_outcome_tests.rs`, `full_skill_result_tests.rs` (2,011 lines): Dedicated unit and regression suites.
- **100% Backward Compatibility**: Maintained exact API and struct compatibility across all public and crate interfaces for CLI (`crates/mint-cli`), Desktop (`src/renderer/src`), and Web (`src/renderer/src-web`). Fully verified with all 58 chat tests, 180 orchestration tests, and 241 CLI tests passing.

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
