# Release Notes - Mint Agent v1.14.0

## Agent Harness Engineering Upgrade (9-Pillar System)

This release introduces a major architectural overhaul transforming Mint Agent into an enterprise-grade Autonomous Agent Harness. Grounded in systematic harness engineering principles, this upgrade equips Mint with deep workspace awareness, precise code navigation, safe autonomous verification loops, git checkpointing, structured knowledge authoring, run-level observability, and benchmarking across CLI, Desktop, and Web.

### 1. Workspace & Monorepo Architecture Detection (Pillar 1)
- **Deep Architecture Scanner (`mint_core::system::project_detector`)**:
  - Automatically identifies monorepo structures (Cargo workspaces, pnpm-workspaces, Turborepo, Lerna, Nx) and primary project ecosystems (Rust, Node/TypeScript, Python, Go, Java).
  - Scans workspace roots and sub-crates/packages, extracting build scripts, package managers, and test runner configurations.
  - Automatically injects an architectural briefing directly into the Agent's system prompt before the first turn, eliminating exploratory "what kind of project is this?" steps.
- **Custom Rules Auto-Ingestion**:
  - Automatically scans and injects project rules from `CLAUDE.md`, `.cursorrules`, `.github/copilot-instructions.md`, `.agents/rules/`, and `.agents/AGENTS.md` into the agent's context.

### 2. Code Intelligence & AST Symbol Navigation (Pillar 2)
- **AST / Syntax-Aware Symbol Extraction (`mint_core::search::symbols`)**:
  - Added native symbol navigation without requiring heavyweight external Language Server Protocol (LSP) daemons.
  - **`find_definition` Tool**: Locates definitions of functions, structs, classes, enums, interfaces, types, and traits across Rust, TypeScript, JavaScript, Python, and Go codebases.
  - **`find_references` Tool**: Scans call-sites, usages, and implementations across workspace files.
  - Eliminates blind full-text grep hallucinations when refactoring or tracing complex type hierarchies.

### 3. Safe Automated Tooling & Zero-Prompt Pre-Approval Policy (Pillar 3)
- **Dedicated Validation Tools (`mint_core::orchestration::tools::safe_tools`)**:
  - **`run_tests`**: Automatically discovers and invokes test runners (`cargo test`, `npm test`, `pytest`, `go test`) or executes targeted test suites.
  - **`run_typecheck`**: Invokes project type-checkers (`tsc`, `cargo check`, `pyright`, `mypy`) to catch compiler-level errors early.
  - **`run_linter`**: Runs code linters (`clippy`, `eslint`, `flake8`/`ruff`, `golangci-lint`) to enforce style and correctness.
- **Zero-Prompt Pre-Approval Policy**:
  - Classified validation and test-running tools as non-destructive safe operations, allowing autonomous execution in the background without prompting the user for repetitive command approvals.

### 4. Task Planning Checklist & State Machine (Pillar 4)
- **Structured Plan State Machine (`mint_core::orchestration::tools::planning`)**:
  - Introduces `plan_task` and `update_plan_step` tools enabling agents to establish structured, multi-step implementation workflows.
  - Supports dynamic step states: `Pending`, `InProgress`, `Completed`, `Failed`, and `Skipped`.
- **Live Terminal & UI Checklist Cards**:
  - **CLI**: Live ANSI progress cards with status glyphs (`[ ]`, `[-]`, `[x]`, `[!]`, `[~]`) and dynamic step updates in `mint-cli`.
  - **Desktop & Web**: Dedicated `PlanChecklistWidget` displaying interactive progress, step state transitions, and step timing.

### 5. Active Verification Loop & Balanced Log Engineering (Pillar 5)
- **Automatic Post-Modification Verification Hook**:
  - Integrated post-edit verification passes in the orchestration loop to trigger targeted checks before declaring tasks completed.
- **Balanced Log Truncation Engine (`Head + Tail Windowing`)**:
  - Implemented 3 KB Head + 12 KB Tail balanced buffer windowing for command and test outputs.
  - Preserves both invocation parameters at the start and critical failure stack traces/compiler errors at the end, joining them with a `[... N bytes truncated ...]` marker to avoid context blowup while retaining actionable diagnostic data.

### 6. Git Safety Harness & Rollback Checkpoints (Pillar 6)
- **Automatic Checkpoint & Rollback Engine (`mint_core::git::checkpoint`)**:
  - **`git_checkpoint`**: Snapshots working tree state and stash references before risky multi-file edits.
  - **`git_rollback`**: Safely reverts working directory changes back to the pre-task snapshot when tasks fail or the agent goes astray.
  - **`git_restore_file`**: Precision rollbacks of individual modified files (`git checkout -- <file>`).
  - **`git_create_branch`**: Automatically spins up isolated task branches (`mint/<task-id>-<slug>`).
  - **`git_commit`**: Generates context-aware commit messages based on diff analysis and commits verified changes.

### 7. Autonomous Knowledge Engine & Documentation (Pillar 8)
- **Tiered Documentation Search & Authoring (`mint_core::system::knowledge_engine`)**:
  - Recursively indexes repository knowledge repositories across `docs/`, `architecture/`, `decisions/` (ADRs), `api/`, and `troubleshooting/`.
  - **`search_docs`**: High-speed keyword and semantic search across local project documentation before falling back to external sources.
  - **`create_project_doc`**: Enables the agent to author, persist, and update Architecture Decision Records (ADRs), API guides, and troubleshooting runbooks directly in the workspace.

### 8. Run Observability Dashboard & Telemetry (Pillar 9)
- **Run Telemetry & Metrics Collection (`mint_core::orchestration`)**:
  - Tracks total execution duration, step counts, token usage, tool breakdown counts, and per-tool execution latency via `RunTelemetrySummary` and `ToolExecutionRecord`.
  - Emits `AgentProgress::RunCompleted` event upon task finalization.
- **Cross-Platform Telemetry Dashboards**:
  - **CLI**: Rich ANSI terminal summary card rendered upon agent turn completion.
  - **Desktop UI & Web UI**: Themed `RunSummaryDashboard` component featuring execution statistics, status indicators, and an expandable tool-call latency drawer with platform parity across `src/renderer/src` and `src/renderer/src-web`.

### 9. Benchmark Evaluation System (Pillar 10)
- **Evaluation Suite Engine (`mint_core::eval`) & CLI Subcommand (`mint eval`)**:
  - Added `mint eval --suite <path> [--concurrency <N>] [--output <path>]` command for automated benchmarking of agent models and harnesses.
  - Evaluates benchmark cases against prompt instructions, file modifications, and unit test pass criteria.
  - Includes benchmark suite templates under `benchmarks/mint_eval.json`.

### 10. Design System & Theming Refactoring (`agent-observability.css`)
- **Semantic CSS Token Integration**:
  - Extracted all ad-hoc styles from `RunSummaryDashboard.tsx` and `PlanChecklistWidget.tsx` into a shared stylesheet `src/renderer/shared/css/agent-observability.css`.
  - Strictly bound all components to Mint design tokens:
    - Surfaces & borders: `var(--surface-bg)`, `var(--surface-bg-alt)`, `var(--border)`, `var(--border-light)`.
    - Typography: `var(--text-main)`, `var(--text-muted)`.
    - Accents & status: `var(--accent)`, `var(--accent-hover)`, `var(--status-speaking)` (success), `var(--status-error)` (failure), `var(--status-listening)` (warning).
  - Guarantees seamless aesthetics and readability across Dark, Light, and Midnight themes on both Desktop and Web interfaces.


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

- **Dynamic Model Fetching (Hybrid Approach)**:
  - Replaced hardcoded model presets with live API discovery across Gemini (`/v1beta/models`), Anthropic (`/v1/models`), OpenAI (`/v1/models`), OpenRouter (`/api/v1/models`), DeepSeek (`/v1/models`), and local OpenAI-compatible endpoints (e.g. LM Studio).
  - **In-Memory Cache & Network Resilience**: Live model lists are cached process-wide for 1 hour (`CACHE_TTL`). Configured a 6-second HTTP request timeout in `mint_core::slash::model_fetcher` to promptly and silently fall back to static presets on network/DNS errors, authentication failures, or offline environments without hanging.
  - **Cross-Platform Parity across CLI, Desktop, and Web**:
    - **CLI (`crates/mint-cli`)**: Integrated async live model fetching into `/models <provider>` selection and the interactive onboarding wizard (`mint onboard`), complete with transient terminal loading feedback (`Fetching available models...`).
    - **Desktop UI (`src/renderer/src`)**: Added Tauri command `fetch_provider_models` and React hook `useProviderModels` powering the Settings General tab for hosted providers and LM Studio.
    - **Web UI (`src/renderer/src-web`)**: Added API endpoint `GET /api/models` on the core API server, web-runtime shim in `src-web/tauri.ts`, and Web UI `useProviderModels` hook ensuring identical dynamic model lists.

- **Dynamic Image Model Fetching (Hybrid Approach)**:
  - Replaced hardcoded image model presets with live API discovery across Google Gemini / NanoBanana (`/v1beta/models`), OpenAI DALL·E (`/v1/models`), and Replicate (`/v1/collections/text-to-image`), with immediate static fallback for Stability AI, Ideogram, and Black Forest Labs (FLUX).
  - **In-Memory Cache & Network Resilience**: Live image model lists are cached process-wide for 1 hour (`CACHE_TTL`), with a 6-second HTTP request timeout in `mint_core::media::image_model_fetcher` falling back gracefully to static presets on offline or auth errors.
  - **Cross-Platform Parity across CLI, Desktop, and Web**:
    - **CLI (`crates/mint-cli`)**: Added `/image-models` slash command, enhanced `/image-provider` with live model fetching, and integrated live model discovery into `mint onboard` Step 4 with transient loading indicators.
    - **Desktop UI (`src/renderer/src`)**: Added Tauri command `fetch_image_provider_models`, React hook `useImageProviderModels`, and updated Settings General Tab & Image Studio Panel (`ImageStudioPanel.tsx`) with dynamic dropdown selection.
    - **Web UI (`src/renderer/src-web`)**: Added API endpoint `GET /api/image-models` on `mint_core` API server, web-runtime shim in `src-web/tauri.ts`, and Web UI `useImageProviderModels` hook ensuring identical dynamic image model discovery.


