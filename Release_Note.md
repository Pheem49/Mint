# Release Notes - Mint Agent v1.14.0

## Smart Consecutive Tool Grouping & Modern Activity Feed UI (Desktop & Web)

Eliminated repetitive tool execution rows (e.g. `calculation >` repeated 6–7 times) by introducing consecutive tool grouping, rich category iconography, and accurate input target parsing:

- **Smart Consecutive Tool Grouping (`agentActivity.ts` & `AgentActivityTable.tsx`)**:
  - Automatically aggregates consecutive executions of the same tool into a single, clean collapsible group row by default (e.g. `Calculated 7 expressions` with `7 steps` badge).
  - Designed with an interactive accordion: clicking the group reveals the granular sub-items connected by fine branch lines (`├─`, `└─`).
  - Single tool executions remain unnested for instant visibility.
- **Resilient Group Status Rollup & Smart Error Recovery**:
  - Fixed false-positive red `Failed` badges on tool groups when searches or tool calls experienced intermediate non-fatal hiccups or trailing redundant calls.
  - Group status in `agentActivity.ts` now rolls up to `done` (`✓`) if at least one sub-step succeeded (`hasDone`), marking a group as `error` only if zero sub-steps succeeded.
  - Non-fatal tool errors occurring before an overall successful run (`RunCompleted` with `SUCCESS`) are marked as `retry` / recovered rather than fatal failures.
  - Granular sub-step badges remain fully transparent when expanding the accordion.
- **Cross-Platform Parameter Alias Resolution (CLI, Desktop & Web)**:
  - **Rust Backend (`crates/mint-core`)**: Added Serde aliases on `AgentInput` for `query` (`q`, `keyword`, `search`, `searchTerm`, `search_term`), `symbol` (`ticker`), `city` (`location`, `place`), `expression` (`expr`, `math`), and `command` (`cmd`). In `tools/web.rs`, added fallback checks against `prompt`, `command`, and `symbol` to eliminate `(empty query)` errors.
  - **CLI (`crates/mint-cli`)**: Added matching alias support in `generic_tool_label`, `explored_action_label`, and `ran_command_labels` ensuring 100% Platform Parity.
  - **Frontend (`agentActivity.ts`)**: Enhanced `describeTool` to parse all parameter aliases and prioritized `query` over generic placeholders.
- **Accurate Tool Target & Inline Result Extraction**:
  - Enhanced `describeTool` in `agentActivity.ts` to extract `expression` / `expr` for math queries, `city` / `location` for weather, `symbol` for stock tickers, and `url` / `selector` for browser automation.
  - Eliminated the fallback where target duplicated the action name (e.g. `calculation` repeating as target `calculation`).
  - Added concise inline result previews (e.g. `16 - 28 + 12 → 0`).
- **Modern Activity Feed Architecture (`AgentActivityTable.tsx`)**:
  - Replaced the rigid 4-column raw table (`Tool | Target | >`) with a modern Activity Feed with category icons (Lucide `Calculator`, `FileCode`, `Folder`, `Terminal`, `Search`, `Wrench`), status badges, and expandable output logs.
  - Synchronized CSS stylesheets across Desktop (`src/renderer/src/css/chat.css`) and Web (`src/renderer/src-web/css/chat.css`).

## Model Chip, Ghost Message Actions & Enhanced Reasoning Block (Desktop & Web UI)

Elevated the visual hierarchy, micro-interactions, and status indicators of conversation messages and reasoning chains:

- **Modern Model Chip (`.provider-model-chip`)**:
  - Replaced plain text provider badge with a sleek, pill-shaped chip displaying provider indicator dot, clean provider name, and model tag.
  - Dynamic indicator dot styling tailored to provider archetype (Anthropic amber, OpenAI green, Google/Gemini cyan, DeepSeek blue, Ollama purple, Groq orange, Mint emerald).
  - Built with theme-adaptive background (`var(--surface-bg)`), subtle border, and interactive hover illumination.
  - Implemented across completed chat turns (`ChatMessageItem.tsx`) and live streaming responses (`ChatPanel.tsx`).
- **Ghost Message Actions (`.msg-action-btn`)**:
  - Upgraded action buttons (Copy, Read Aloud / TTS, Edit) into polished 26x26px ghost buttons.
  - Smooth hover backgrounds, micro-border feedback, and prominent emerald accent states with pulsing glow for active playback and copy confirmation.
- **Thinking / Reasoning CoT Block Overhaul (`ThinkingBlock.tsx`)**:
  - **Quiet UI Aesthetic**: Replaced neon cyberpunk green borders and radioactive glowing effects with refined, quiet neutral borders (`var(--border, rgba(255, 255, 255, 0.08))`) inspired by Claude 3.7 and Cursor.
  - Replaced legacy question-mark icon with an authentic, neutral **Brain** icon (`<Brain size={14} />`) from Lucide.
  - Added a gentle, non-distracting live breathing pulse indicator (`@keyframes thinkingDotPulse`) without high-intensity colored halos.
  - **Secondary Role Typography & Visual Hierarchy**:
    - Downscaled reasoning body text size to `12.5px` (compact and distinct from the `15px` primary chat messages) with line-height `1.55`.
    - Dropped text color to muted slate (`var(--text-muted, #94a3b8)` with `opacity: 0.88`), communicating its role as internal background reasoning / secondary metadata.
    - Scoped all markdown children inside reasoning steps (`.chat-formatted-body`, `.chat-paragraph`, `.chat-heading`, `.chat-list-*`, `.chat-bold-highlight`, `.chat-inline-code`): clamped headings to `12.8px`, subdued bold highlights, neutralized list bullets and numbering from neon green to muted slate, and tightened vertical margins.
  - Added custom ultra-slim 3px scrollbar (`.thinking-block-content::-webkit-scrollbar`) with translucent thumb for deep reasoning chains.
- **Complete Desktop & Web Parity**:
  - Unified component logic in `src/renderer/shared` and synchronized stylesheet updates across `src/renderer/src/css/chat.css` and `src/renderer/src-web/css/chat.css`.

## Native Chain-of-Thought Reasoning Extraction & Full Provider Parity (CLI, Desktop & Web)

Resolved an issue where reasoning models (such as `deepseek-reasoner` / DeepSeek-R1) appeared with `"Model didn't send thinking steps this time"` when executing tools in Agent Mode:

- **Multi-Provider Reasoning & CoT Ingestion (`crates/mint-core/src/agent/chat.rs`)**:
  - **DeepSeek Reasoner**: Extracted `message["reasoning_content"]` emitted during native tool calling and conversation turns.
  - **OpenRouter & Compatible Endpoints**: Extracted `message["reasoning"]` and `message["thought"]` fields.
  - **Embedded `<think>` Tags**: Added `extract_think_tag` parser to detect and extract `<think>...</think>` blocks embedded in model text streams (such as Ollama or local QwQ/R1 deployments).
  - **Google Gemini 2.0 Flash Thinking**: Separated candidate parts with `thought: true` into the reasoning thought stream instead of blending into the final content string.
  - **Anthropic Claude 3.7 Sonnet**: Ingested native `thinking` content blocks.
  - **Ollama**: Extracted `message["thinking"]` and `message["reasoning_content"]`.
- **Orchestration Agent Loop Integration (`crates/mint-core/src/orchestration/mod.rs`)**:
  - Attached extracted `thought` tokens to `AgentDecision` across tool-calling and completion turns.
  - Guaranteed `AgentProgress::Thought` is emitted for every step containing reasoning traces, allowing the frontend's `ThinkingBlock` to render the live pulsing brain icon and step-by-step reasoning steps.
  - Added unit test coverage for each reasoning provider schema.


## Minimalist AI Workspace Code Block & Command Styling (Desktop & Web UI)

Mint's code block rendering and inline command displays have been redesigned to deliver a minimalist, high-clarity developer workspace aesthetic inspired by Claude and Cursor:

- **Zero-Dependency Syntax Highlighting Engine (`src/renderer/shared/utils/syntaxHighlight.tsx`)**:
  - Implemented a fast, lightweight, and resilient tokenization engine tailored for developer commands and code snippets.
  - **Bash / Shell / Terminal Commands**: Intelligent distinction between primary commands (`ffmpeg`, `git`, `docker`, `cargo`, `npm`, etc.), command-line flags and options (`-i`, `-vf`, `--help`), string parameters, variables (`$VAR`), property assignments (`key=value`), comments, and pipe operators (`|`, `&&`).
  - **Comprehensive Multi-Language Support**: Dedicated tokenizers for JavaScript, TypeScript, Python, Rust, JSON, SQL, HTML, CSS, and YAML with universal fallback handling.
  - **Low-Saturation Aesthetic**: High-contrast, easy-on-the-eyes palette (soft sky blue commands, calm emerald strings, warm amber flags, muted slate comments).
- **Redesigned Minimalist Header & Controls (`ChatCodeBlock.tsx`)**:
  - Monospace lowercase language badge (`bash`, `typescript`, `python`, etc.) with subtle metadata tracking code line count for multi-line snippets.
  - **Interactive Copy Action**: Added clear "Copy" label alongside the clipboard icon with fluid animated transition to green checkmark and "Copied!" confirmation.
  - **Ghost Download Button**: Clean, unobtrusive button for downloading the snippet with the appropriate file extension.
- **Line Numbers & Non-Selectable Gutter**:
  - Automatically displays a neat gutter with line numbers for multi-line snippets (`> 1` lines).
  - Designed with `user-select: none` so selecting code or copying to clipboard preserves clean code without stray line numbers.
  - Single-line terminal commands (e.g. `ffmpeg ...`) remain uncluttered with full padding and zero gutter distraction.
- **Refined Tech Pill Inline Code (`.chat-inline-code`)**:
  - Updated inline code pills across Desktop and Web with subtle dark translucent background (`rgba(255, 255, 255, 0.06)`), fine border, and crisp typography for clear readability in paragraphs and lists.
- **Full Platform Parity**:
  - Shared component `ChatCodeBlock` and identical CSS rules implemented across both Desktop (`src/renderer/src/index.css`) and Web (`src/renderer/src-web/index.css`).

## Dynamic Model Temperature Control, Per-Model Persistence & Interactive Wizard (CLI, Desktop & Web)

Mint now provides comprehensive model temperature management with **per-model persistence**, interactive multi-step selection wizard, and fine-grained, model-aware defaults tailored to prevent repetitive reasoning loops on reasoning models while preserving strict precision on coding backends:

- **Per-Model Persistence & Model-Aware Defaults (`crates/mint-core/src/system/config.rs`)**:
  - **Independent Per-Model Overrides**: Stores custom temperatures in `modelTemperatures: HashMap<String, f64>` so each model independently remembers its configured temperature (e.g. `deepseek-chat` at `0.65`, `claude-sonnet-5` at `0.15`, while unset models seamlessly use smart defaults).
  - Automatically sets temperature to **`0.6`** for Open-Weight Reasoning / CoT models (DeepSeek-R1/V3, Qwen-QwQ, and thinking/reasoner fine-tunes), directly mitigating the known agent looping/stagnation behavior caused by greedy decoding (`0.0`) or unconstrained drift (`1.0`).
  - Sets temperature to **`0.1`** for Codestral and pure code completion models to maximize deterministic completion accuracy.
  - Defaults to **`0.2`** for general coding and tool-calling models (Claude 3.5/3.7, GPT-4o, Gemini 2.5, Qwen 2.5 Coder, Llama 3.3, Ollama, etc.) for high accuracy, strict JSON schema adherence, and reliable tool execution.
  - Automatically omits the `temperature` parameter for OpenAI reasoning architectures (`o1`, `o3`), which reject explicit temperature values with HTTP 400 Bad Request.
  - Clamps all user values safely between `0.0` and `2.0`.
- **Interactive Multi-Step `/temperature` Selection Flow (`crates/mint-core/src/slash/mod.rs` & `slash-commands.json`)**:
  - **Interactive Wizard**: Pressing Enter on bare `/temperature` or `/temp` launches an interactive multi-step picker via `SlashResponse::NeedsChoice`:
    1. **Provider / Active Model Shortcut**: Quickly choose between the currently active model shortcut or any configured/standard provider.
    2. **Model Selection**: Lists provider models dynamically or from presets, tagged with current `[Custom: 0.XX]` or `[Auto: 0.XX]` status.
    3. **Temperature Presets**: Choose from curated presets (`Auto`, `0.0 Deterministic`, `0.1 Codestral`, `0.2 Standard Coding`, `0.4 Balanced`, `0.6 DeepSeek/Reasoning`, `0.8 Creative`, `1.0 Brainstorming`) or specify custom values.
  - **Fast Shorthands**:
    - `/temperature <0.0-2.0>` sets custom temperature for the active model.
    - `/temperature <model> <0.0-2.0>` sets temperature for a specific model directly.
    - `/temperature default` resets the active model to smart defaults.
    - `/temperature <model> default` resets that specific model.
    - `/temperature status` displays a formatted markdown status table listing active model effective temperature, reasoning mode handling, and all configured per-model overrides.
  - Available across all 3 interfaces with unified terminal interactive selection (CLI) and clickable chip selectors (Desktop & Web UI).
- **Interactive UI Slider with Per-Model Binding in Settings (`GeneralTab.tsx` & `config.ts`)**:
  - Added a dedicated **Model Temperature** control card under **AI Routing: Provider & Model** in Settings.
  - Dynamically binds to `config.modelTemperatures[activeModel]`, displaying dynamic status badges (`Custom for <model> (0.XX)` or `Auto (<rationale>)`).
  - Smooth slider (range `0.00` to `1.50`, step `0.05`) with live numerical display and an instant **`Reset to Auto`** action that removes the override for the active model while preserving other model configurations.
  - Informative contextual tooltip explaining the optimal sampling entropy for each model archetype.

## Remote MCP Server Connections via URL / SSE & Live Connection Testing (CLI, Desktop & Web)

Mint now natively supports connecting to remote Model Context Protocol (MCP) servers hosted over HTTP / HTTPS with Server-Sent Events (SSE) streaming and direct HTTP POST JSON-RPC transports.

- **Remote MCP Protocol Support (`crates/mint-core/src/integrations/mcp.rs`)**:
  - Extended `McpServer` configuration with `url: Option<String>`, `headers: Option<BTreeMap<String, String>>`, and `transport: Option<String>`.
  - Added `McpRemoteSession` with an SSE event listener thread and endpoint discovery (`endpoint` SSE events), dual-channel JSON-RPC dispatch (direct POST responses with SSE event fallback), and background keepalive.
  - Unified local stdio processes and remote network connections under the `McpSession` abstraction.
  - Implemented `test_remote_mcp_connection` for pre-flight connection verification and discovery of remote tools, prompts, and resources.
- **Interactive UI with Segmented Transport Toggle & Live Testing (`McpServersView.tsx`)**:
  - **Segmented Toggle**: Switch effortlessly between **`🌐 Remote Server (URL / SSE)`** and **`💻 Local Command (stdio)`** in the Add Server modal.
  - **Remote Endpoint & Authentication Setup**: Configure Remote Server URL (`https://...`), choose Authentication type (`None`, `Bearer Token`, or `Custom Headers JSON`), and provide optional custom server icons.
  - **Live Connection Testing**: Interactive **`Test Connection`** button triggers a non-blocking diagnostic test against the endpoint, displaying live loading states, success verification (reporting available tools, resources, and prompts count), or detailed error diagnostics before saving.
  - **Enterprise-Grade Security Notice & Risk Acknowledgement**: Custom warning banner with `<ShieldAlert />` icon and safety acknowledgement checkbox before adding external endpoints, ensuring users review risks prior to granting remote tool execution.
  - **Server Card & Details Parity**: Remote servers display a distinct `Remote` badge with Lucide vector icon on their card and reveal their full endpoint URL and custom headers in the server detail modal.
- **Full Platform Parity across CLI, Desktop & Web**:
  - **CLI (`crates/mint-cli`)**:
    - `mint mcp add <name> <url>` auto-detects remote URLs and accepts authentication headers via `--env "Authorization=Bearer <token>"`.
    - Interactive slash command `/mcp add <name> <url>` seamlessly adds remote servers with SSE transport.
    - `mint mcp list` displays `(url: <url>)` for remote servers.
  - **Desktop Tauri (`src-tauri` & `src/renderer/src`)**:
    - Registered `test_mcp_connection` Tauri command for native asynchronous ping and schema validation.
  - **Web UI & Server (`crates/mint-core/src/api_server/routes/cron_mcp.rs` & `src/renderer/src-web`)**:
    - Added `POST /api/mcp/test` REST route running on a dedicated blocking task to prevent event-loop starvation.
    - Updated Web `MintPlatformApi` with `testMcpConnection` bridging to the backend endpoint.

## Rewind System Overhaul: Themed Confirmation Modal, Targeted Step Checkpoints & Rescue Undo (CLI, Desktop & Web)

A comprehensive upgrade to Mint's file rewind and Git time machine capabilities:

- **Custom Themed Rewind Confirmation Modal (`RewindModal.tsx`)**:
  - Replaced raw browser `window.confirm()` and `alert()` with a custom, theme-aware review dialog (`.rewind-modal`).
  - Highlights safety guarantee, target step badge, short commit hash, action type, timestamp, and the exact files to be reverted (with line addition/deletion indicators and `[NEW FILE]` tags).
  - Supports keyboard shortcuts (`Escape` to dismiss, `Cmd/Ctrl + Enter` to confirm) and includes safety loading states.
- **Targeted Step Checkpoint Matching**:
  - Rewind buttons on file changes summary cards now dynamically bind to the exact pre-edit checkpoint for that specific interaction/turn, rather than defaulting to the latest session checkpoint.
  - Matches checkpoints against files touched in that turn or interaction creation timestamp, ensuring rolling back an earlier turn restores the workspace to the exact state before that turn began.
- **Rescue Snapshot Safety Net & Instant Undo (`/rewind undo`)**:
  - Before any rollback occurs, Mint creates a safety rescue snapshot in Git (`refs/mint/rescue/<timestamp>`).
  - **Floating Undo Banner**: After rewinding, an interactive floating notification appears in the chat panel with a prominent **`[ ↩ Undo Rewind ]`** button, allowing users to restore reverted files with a single click.
  - **Full Platform Parity**:
    - **CLI**: Run `/rewind undo` to recover from the latest rescue snapshot.
    - **Backend Slash Command**: `/rewind undo` supported via `cmd_rewind`.
    - **Desktop Tauri Command**: `undo_git_checkpoint` native invocation.
    - **Web REST API**: `POST /api/checkpoints/undo` endpoint.

## Live Preview Path Resolution & File Changes Deduplication (CLI, Desktop & Web)

- **Intelligent Path Resolution (`mint_core::files::resolve_readable_path`)**:
  - Solved `Failed to read file: Not Found` preview errors when viewing files with relative paths, tilde (`~`) prefixes, or notes stored in `~/.config/mint/notes/`.
  - Automatically resolves file targets across candidates: user home directory expansion, active workspace root, current working directory, and system config note locations.
  - Implemented with complete parity across all three interfaces: Web API endpoint (`/api/file/read`), Desktop Tauri command (`read_workspace_file`), and CLI (`mint preview` / `handle_preview`).
- **Distinct Operation Tracking for Creation & Modifications (`agentProgress.ts`)**:
  - Preserved distinct file change records for separate tool actions (e.g. initial file creation via `note_write` vs subsequent edits via `apply_patch`), ensuring the summary accurately reflects `1 created, 1 modified` with dedicated diff inspection for each step.
  - Both cards link to the resolved target on disk so clicking Preview on either the created or modified entry works seamlessly without path errors.
- **Preview Resiliency & Fallback Content**:
  - Pass initial created file content as in-memory fallback to `ArtifactPreviewPanel`, ensuring previews render instantly and gracefully even before disk sync or in transient environments.
- **Chat Scroll Preservation on Preview Toggle**:
  - Eliminated the issue where opening or closing the Live Preview panel caused the chat messages container to unmount and reset its scroll position (`scrollTop = 0`) to the top of history.
  - Stabilized the split view wrapper DOM hierarchy (`.chat-panel-split-wrapper` with `display: contents` in `no-preview` mode) so the conversation panel never unmounts, while anchoring the scroll position to the active message.
- **Draggable & Resizable Split Pane Divider**:
  - Added an interactive divider bar (`.preview-split-resizer`) between the Chat Panel and Live Preview panel.
  - Smooth 60fps mouse drag-to-resize using direct CSS variable updates (`--preview-width`), preventing costly re-renders of the chat conversation while dragging.
  - Double-click on the divider grip bar instantly snaps back to 50/50 balance.
  - Automatically persists the user's preferred split ratio in `localStorage` (`mint_preview_split_ratio`), preserving custom sizing across sessions.
  - Safety constraints prevent either pane from collapsing (Chat >= 340px, Preview >= 280px).

## File Changes, Live Preview & Diff Viewer Theme Parity (Desktop & Web UI)

- **Comprehensive Theme Alignment**:
  - Replaced hardcoded inline styles (`#10b981`, `#0b0f19`, `rgba(15, 23, 42, 0.6)`, `#a7f3d0`) in the File Changes summary and live Diff viewer with dynamic CSS theme variables (`var(--accent)`, `var(--surface-bg)`, `var(--input-bg)`, `var(--border)`, `var(--text-chat, var(--text-main))`, `var(--text-muted)`).
  - Ensured complete visual parity across default dark, light (`[data-theme="light"]`), midnight (`[data-theme="midnight"]`), and custom themes.
- **Live Preview Split View Panel (`ArtifactPreviewPanel.tsx`)**:
  - Replaced legacy fallback colors (`#232730`, `#111317`, `#f3f4f6`, `#181a20`, `#10b981`) across the entire Preview panel with native Mint theme tokens: `--panel-bg`, `--surface-bg`, `--border`, `--text-main`, and `--accent`.
  - Preview/Code tab toggles, device viewport selectors (Desktop/Tablet/Mobile), artifact badges, and markdown view now fully match the active system theme.
- **Diff Review Modal (`DiffReviewModal.tsx`)**:
  - Split and unified diff panes, header diff badge, and addition/deletion counters now track the active theme accent, error, and text variables.
- **Readable Diff Line Highlighting**:
  - Diff hunk lines now preserve the user's configured chat text color (`var(--text-chat, var(--text-main))`) while clearly highlighting additions and deletions with subtle background tints, accent/error border indicators, and separate unselectable sign markers (`+` in `var(--accent)`, `-` in `var(--status-error)`).
- **Rewind Action & Status Badges**:
  - Rewind checkpoint button, [NEW FILE] badge, and Preview button now cleanly utilize system theme tokens (`var(--status-error)`, `var(--hover-delete-bg)`, `var(--accent)`, `var(--radius-xs)`).
- **Platform Parity**:
  - Reusable `.file-changes-*` styling standardized across both Desktop (`src/renderer/src/css/chat.css`) and Web (`src/renderer/src-web/css/chat.css`).

## Claude Desktop Style Choice Cards UI Overhaul (Web & Desktop)

Redesigned the Action Approval and AskUser Question interface across both Web (`src/renderer/src-web`) and Desktop (`src/renderer/src`):

- **Claude Desktop "Choice Cards" Layout**:
  - Replaced legacy small horizontal buttons with vertical, interactive Choice Cards featuring dedicated hotkey badges (`[ 1 ]`, `[ 2 ]`, `[ 3 ]`), titles, and explanatory subtitles.
  - Distinct hover accents for Approve (emerald), Session Allow (sky blue), and Deny/Cancel (rose red).
  - Clean header with tool-specific badges (RunShell 🐚, File Edit 📝, MCP 🔌, Question 💬) and risk tiers (`DANGEROUS`, `APPROVAL REQUIRED`).
  - Monospace code box for shell commands and diffs with quick syntax review.
- **Global Keyboard Hotkey Triggers**:
  - Pressing `1`, `2`, `3` on the keyboard instantly selects or approves the corresponding choice card when not actively typing in an input field.
- **Enhanced AskUser Selection & Custom Answer**:
  - Full support for single-select choices, multi-select checkboxes, and custom write-in answers with `Enter`-to-submit.
- **Full Platform Parity**:
  - Unified across Desktop and Web via the shared `ApprovalCard.tsx` component and matching CSS tokens.

## Web UI Performance & Reliability Overhaul

This update resolves intermittent slow loading, white screens, and update error loops when accessing Mint via web browser:

- **Decoupled Build Output Targets (`out/web` vs `out/renderer`)**:
  - Web UI now builds cleanly to `out/web` while Desktop UI continues building to `out/renderer`.
  - Completely eliminates output clobbering where desktop builds wiped out `index-web.html` or chunk hashes.
- **Same-Origin Vite API Reverse Proxying**:
  - Added reverse proxy configuration for `/api` in Vite dev and preview modes routing to backend port 3000.
  - Eliminated CORS preflight `OPTIONS` requests, dramatically reducing API response times.
  - Resolves browser mixed-content blocks when accessing over HTTPS, SSH tunnels, or VS Code port forwarding.
- **Removed Render-Blocking Unused Live2D Script**:
  - Removed 150 KB synchronous `Live2DCubismCore.js` script tag from `index-web.html` `<head>`, as Live2D companion rendering is desktop-only.
- **Service Worker Dev Isolation & Smart Fallbacks**:
  - Dev mode now automatically purges leftover service workers from preview/production runs so Vite HMR is never intercepted or corrupted.
  - Added strict exclusions in `sw.js` for Vite internal endpoints.
- **Resilient Chunk Error Boundary & Dark Recovery Screen**:
  - Replaced the harsh white `#ffffff` error screen with an on-theme Mint dark card.
  - "Refresh & Update" button automatically clears stale `caches` and unregisters old service workers before reloading to guarantee a clean recovery loop.
- **Failsafe Timeout Guards**:
  - Added 5-second fetch timeouts to `authGetCurrentUser`, `getRuntimeStatus`, and `getSettings` with a 6-second failsafe in `AuthGate`, preventing indefinite hangs on "Loading Mint…".
- **Intelligent CLI Binary Discovery & Fast Dev Iteration**:
  - Rewrote [src/bin/index.js](file:///home/pheem49/vscode/Project/Mint-CLI/src/bin/index.js) to resolve binaries dynamically across local release (`target/release/mint`), fast local debug (`target/debug/mint`), user Cargo installation (`~/.cargo/bin/mint`), user local bin (`~/.local/bin/mint`), and global system paths (`/usr/local/bin`).
  - Running `cargo build -p mint-cli` (debug) is now immediately picked up by the CLI runner without requiring a full 5-10 minute `--release` build or reinstall.
  - Added `npm run install:cli` (`cargo install --path crates/mint-cli --locked`) to install the native binary permanently to `~/.cargo/bin`.

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
- **Dynamic Video Generation Model Fetching & Studio Parity (Google Veo)**:
  - Transitioned Google Veo video generation from hardcoded strings to dynamic API-discovered model discovery with static fallback presets (`veo-3.1-generate-preview`, `veo-3.1-fast-generate-preview`, `veo-3.1-lite-generate-preview`, `veo-2.0-generate-001`).
  - **In-Memory Cache & Network Resilience (`mint_core::media::video_model_fetcher`)**:
    - Live video models queried from Gemini API (`/v1beta/models`), filtered by `veo`/`video` keywords, with `models/` prefix stripping.
    - Cached process-wide for 1 hour (`CACHE_TTL = 3600s`) with a 6-second timeout guard, falling back instantly to canonical presets in `mint_core::media::video_models` on network or authentication errors.
  - **Full Platform Parity across CLI, Desktop, and Web**:
    - **CLI (`crates/mint-cli`)**: Added `/video-models` (and `/videomodels`) slash command, upgraded `/video-provider` with live discovery and interactive selection prompts (`SlashResponse::NeedsChoice`), and integrated live model fetching into `mint onboard` with transient terminal loading indicators (`Fetching available models...`).
    - **Desktop UI (`src/renderer/src`)**: Added Tauri IPC command `fetch_video_provider_models`, created React hook `useVideoProviderModels`, and updated the Settings General tab to render dynamic model dropdowns with bidirectional configuration persistence.
    - **Web UI (`src/renderer/src-web`)**: Added REST API endpoint `GET /api/video-models` and dedicated `GET /api/video-gen/providers`, web-runtime IPC shim in `src-web/tauri.ts`, and Web `useVideoProviderModels` hook.
    - **Veo Studio Panel (`VeoStudioPanel.tsx`)**: Upgraded studio model selection dropdown to dynamically load available live models while preserving static presets, synchronizing the active model seamlessly with settings and CLI commands via the centralized `modelManager`.

- **Application Version & Community Links in Settings Footer**:
  - Replaced the legacy "Quit Application" button in the Settings modal footer with a clean metadata block.
  - Displays application version (`Version: {APP_VERSION}`) dynamically sourced from `version.ts`.
  - Added external link to GitHub repository ([GitHub](https://github.com/Pheem49/Mint)) with external link indicator icon (`↗`).
  - Added external link to Mint website ([Website](https://mint.aemeth.xyz/)) with external link indicator icon (`↗`).
  - Implemented across both Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`) with responsive wrapping on compact viewports (< 620px).
  - Exported `APP_VERSION` from `package.json` into a centralized frontend module (`src/renderer/shared/version.ts`) ensuring a single source of truth without manual string duplication.
  - Added `"version"` reporting to the core API server `/api/status` endpoint.
