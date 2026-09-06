# Release Notes - Mint Agent v1.14.0

## Agent Activity Retry & Self-Correction UX Refinement

- **Soft Recovered / Retry State for Intermediate Tool Errors**:
  - When an intermediate tool step encounters an error (e.g. missing arguments or transient failures) but the Agent subsequently retries, self-corrects, or finishes the task, the activity table now marks the step as `'retry'` with an amber badge (`[Retried]`) instead of a harsh red `Failed` label.
  - The tool/action name (e.g., `web_search`) is preserved in the "Tool" column rather than being wiped out by the word "Failed".
  - Terminal red `[Failed]` badges are reserved strictly for unrecovered errors where the agent halted without subsequent success.
  - Added descriptive fallback targets (e.g. `(empty query)`, `(empty path)`) when a tool was invoked without required arguments so users immediately understand why a step required a retry.
  - Implemented across both Desktop (`src/renderer/src`) and Web (`src/renderer/src-web`) interfaces with consistent theme styling.

## Settings First-Load Reliability & Route Resilience

- **Eliminated Circular Dependency Cycle in Settings Tabs**:
  - Moved `CustomProviderConfig`, `CustomProviderModel`, and `CustomProviderHeader` to `shared/types.ts` as the canonical source of truth.
  - Updated all settings tab components (`GeneralTab`, `AgentsTab`, `PluginsTab`, `ThemeTab`, `AutomationTab`, `AudioTab`) to import config, models, and types directly from `@shared/constants` and `@shared/types` instead of `@/components/SettingsWindow`.
  - Fixes uninitialized `undefined` exports during initial module evaluation in Vite ESM, which caused first-load crashes (`ChunkErrorBoundary`) in Web and Desktop.

- **Hashbang Route Normalization (`#!/settings` & `#/settings`)**:
  - `getCurrentRoute()` in both `src-web/App.tsx` and `src/App.tsx` now normalizes both `#/` and `#!/` prefixes cleanly to prevent route mismatch on direct navigation or hashbang links.

- **Resilient Lazy Chunk Loading & Preload**:
  - Implemented `lazyWithRetry` with automatic backoff retry to prevent transient chunk fetch failures from unmounting the app.
  - Added background idle preloading for the Settings bundle so opening Settings is instant and reliable.
  - Added a localized `ModalErrorBoundary` inside the settings modal container to prevent any modal rendering exception from bubbling up and unmounting the main dashboard and chat.

## UI Stylesheet Cleanup & Theme Token Consistency

- **Removed the legacy `mint-*` layout CSS** (~330 lines per copy) from `src/renderer/src/index.css` and `src/renderer/src-web/index.css`:
  - `.mint-app`, `.mint-sidebar`, `.mint-toolbar`, `.mint-composer`, `.mint-message`, `.mint-model`, `.mint-picture(s)`, `.mint-empty`, `.mint-workspace` and their `@media` block were dead — no component renders those classes anymore.
  - Fixed a real styling conflict: the leftover `.mint-attachment` / `.mint-attachment button` rules were overriding the current composer-attachment styles in `chat.css` (transparent remove button, stray slate background/padding). The attachment strip now renders exactly as `chat.css` defines it, including the red circular remove button.
  - Removed the duplicate `.chat-inline-code` definition (two conflicting blocks in the same file) and the unused `.chat-ordered-list` / `.chat-unordered-list` rules.

- **Hard-coded colors replaced with theme tokens** so chat tables, code blocks, and error banners now follow the active theme (dark/light/midnight) instead of assuming a dark background:
  - `.mint-error` now derives from `--status-error` / `--text-main` via `color-mix`.
  - `.chat-table*` and `.chat-code-*` rules use `--border`, `--border-light`, `--shadow-sm`, and `color-mix` tints of `--text-main` instead of `rgba(255,255,255,…)` values that were invisible on the light theme.
  - Markdown table renderer (`src/renderer/shared/utils/markdown.tsx`) inline styles (container border/shadow, header tint, zebra rows, row borders) — these override the CSS, so they are now token-based too, fixing themed tables on Desktop and Web at once.
  - `body`/`html` base text color now uses `var(--text-main)` instead of a fixed dark-theme hex.

## Web Search Image Hotlink Protection & Direct URL Resolution

- **Unwrapped Web Search Image Proxy URLs**:
  - Automatically unwraps Brave Search internal proxy URLs (`imgs.search.brave.com`) back to direct source image URLs in both backend and frontend, eliminating 403 Forbidden errors.
  - Fixes images failing to display in Web and Desktop message bubbles while appearing as raw Markdown links in the CLI.
  - Automatically decodes existing image URLs stored in past conversation history upon rendering.

- **Referrer Policy Hardening**:
  - Added `referrerPolicy="no-referrer"` to image tags across Markdown bubbles, Image Search tiles, and Search Source drawer thumbnails to bypass third-party image hotlink protection.

## CLI UX & Ergonomics Improvements

- **Direct One-Shot Prompt Execution (`mint [PROMPT]...`)**:
  - Running `mint "explain this function"` or `mint explain this function` now directly executes the code agent on that task without needing `mint agent "..."` or entering the interactive TUI.
  - Returns exit code `0` on successful completion or `130` on cancellation.

- **Top-Level Global Flags (`-m`, `-C`, `--fast`, `--plan`, `--image`)**:
  - Added `-C / --cwd <DIR>` to set the workspace directory upfront (mirroring `git -C`).
  - Added `-m / --model <MODEL>` for temporary in-memory model overrides per session or task (e.g. `mint -m claude-3-7-sonnet "..."` or `mint -m gemini-2.5-flash`), with automatic provider detection.
  - Added `--fast` and `--plan` flags at the root CLI level.
  - Added `--image <PATH>` to attach image files directly to one-shot prompts.

- **Graceful Interruption & Ctrl+C Handling**:
  - Fixed an issue where `Ctrl+C` was swallowed while the queueing box held raw terminal mode. `Ctrl+C` and `Esc` now cleanly interrupt agent turns in both normal and fast modes.
  - Pressing `Ctrl+C` on an empty prompt now displays the yellow `{WARN}Press Ctrl+C again or Ctrl+D to exit{RESET}` notice below the input box (matching `Ctrl+D`'s notification behavior) instead of overriding the prompt placeholder. Pressing either key again cleanly closes the session.

- **Robust Piped Input, Non-TTY Fallbacks & Scriptability**:
  - Fixed TTY detection across `confirm.rs`, `picker.rs`, `approval_prompts.rs`, and `agent.rs` by checking `!io::stdin().is_tty()` alongside `stdout`.
  - When `stdin` is piped (e.g. `echo "y" | mint run ...` or in CI/CD scripts), prompts automatically fall back to line-oriented reading (`read_line`) instead of attempting raw-mode key polling or hanging on terminal escape sequences.
  - Interactive arrow-key selection (`prompt_interactive_select`) safely returns `None` immediately when input is not a terminal.
  - `mint run <COMMAND>` now prompts for confirmation via `confirm()` when `--approve` is omitted, allowing interactive confirmation in terminals or automated pipeline approvals via `printf "y\n" | mint run ...`.

- **Fast Mode Esc & Ctrl+C Cancellation**:
  - Unified turn cancellation in `agent.rs` so that `--fast` mode can be interrupted immediately with either `Esc` or `Ctrl+C`, without leaving terminal cursor or raw mode in an inconsistent state.

- **Streaming Stability & Localization Audit**:
  - Replaced `lines().flatten()` with `.map_while(Result::ok)` in the `mint auto` log watcher, preventing infinite loop risks on broken file streams or invalid UTF-8 bytes.
  - Fixed UTF-8 character boundary panics in tool output truncation (`hooks.rs`) and agent instruction summaries (`agent.rs`). Slicing strings with non-ASCII multi-byte characters (e.g. Thai `\u{0e38}`, Japanese, emojis) now safely snaps backward to the nearest valid character boundary.
  - Audited all user-facing CLI prompts across `crates/mint-cli` to ensure 100% English language consistency (e.g. `$skill` prompts).

## Architecture & Platform Parity (CLI, Desktop & Web)

- **Core Slash Command Engine Unification (`mint_core::slash::execute`)**:
  - Connected CLI interactive mode (`interactive/slash_commands.rs`) directly to `mint_core::slash::execute`.
  - Guarantees 100% feature and behavioral parity across CLI, Desktop, and Web: commands like `/models`, `/fast`, `/autoskill`, `/autorecall`, `/autofacts`, `/factrecall`, `/multi-agent`, `/release-notes`, `/clear`, `/init`, `/remember`, `/memory`, `/link`, `/plugin`, `/code`, `/generate-image`, `/cd`, `/stats` now share identical backend logic, options, persistence, and Markdown rendering across all platforms.
  - Interactive choice requests (`SlashResponse::NeedsChoice`) seamlessly invoke the CLI's interactive arrow-key selector (`prompt_interactive_select`) with live selection re-dispatch.
  - Preserved CLI-specific specialized commands (`/plan`, `/palette`, `/bg`, `/jobs`, `/shells`, `/exit`, `/image`, `/paste`, `/avatar`, and interactive TUI wizards for bare `/mcp`, `/cron`, `/subagent`).

- **Comprehensive `@` Mention Parity with GUI**:
  - Upgraded the CLI's `@` mention and autocompletion system from only listing MCP servers to a unified multi-category completion popup matching the Desktop & Web interface:
    - Configured MCP servers (`[Plugin]`): `@<server>` with tool details (ordered at the very top).
    - Builtin contexts (`[Context]`): `@workspace`, `@file`, `@docs`, `@memory` with descriptive badges.
    - Workspace files and directories (`[File]`, `[Folder]`): Automatically scans the active workspace directory, skipping build artifacts (`target`, `.git`, `node_modules`).
  - Supports `@` mention autocomplete anywhere in the input line (not only at the start of the line), correctly replacing the active token at the cursor.

- **Subcommand Modularization (`main.rs` Refactoring)**:
  - De-monolithized the 3,190-line `main.rs` down to ~400 lines by decomposing subcommand handlers into dedicated modules under `crates/mint-cli/src/commands/`:
    - `commands/config.rs`: `Status`, `Config`, `Providers`, `Update`, `Onboard`, `Setup`.
    - `commands/system.rs`: `Run`, `Open`, `OpenApp`, `Preview`, `ReadFile`, `ReadFolder`, `Safety`, `Files`.
    - `commands/agent.rs`: `Agent`, `Rewind`, `Auto`, `Web`, `Api`, `Gateway`, `Chat`, `Imagine`, `Veo`, `Video`, `Avatar`.
    - `commands/integrations.rs`: `Mcp`, `Link`, `Hooks`, `Gmail`, `Plugins`.
    - `commands/knowledge.rs`: `Learn`, `Symbols`, `SemanticCode`, `Knowledge`, `Code`, `Skills`.
    - `commands/tasks.rs`: `Memory`, `Task`, `Cron`.
    - `commands/mod.rs`: Unified `Command` enum and clean `dispatch()` router.
- **Config Safety & Test Environment Isolation**:
  - Added automatic test environment isolation (`is_test_environment()` in `crates/mint-core`) so test runners never read or write user configuration files (`~/.config/mint/mint-config.json`), preventing accidental config wiping during unit tests.
  - Added automatic backup preservation (`mint-config.json.bak`) before every configuration write operation in `save_config_to`.
  - Added `MINT_CONFIG_PATH` environment variable override support for testing and custom config paths.

