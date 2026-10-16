# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Mint is

A local-first AI assistant with **one Rust core, three surfaces**: a terminal CLI, a Tauri v2
desktop app and a web UI, plus an optional separate Mint Companion Live2D app. The same chat/memory/agent/tools/safety
logic backs all three, plus messaging bridges (Telegram, Discord, Slack, LINE, WhatsApp,
Signal, Gmail) that share the CLI's conversation.

## Build / test / lint

Rust workspace, `edition = "2024"`. `npm` wraps the Rust build; the npm package
(`@pheem49/mint`) ships `src/bin/index.js`, a shim that spawns `target/release/mint`, and a
`postinstall` that runs `cargo build -p mint-cli --release`.

| Task | Command |
| --- | --- |
| Run the test suite | `cargo test -p mint-core -p mint-cli` (alias: `npm test`) |
| One test | `cargo test -p mint-core scoped_chat_id` / `cargo test -p mint-core --test memory_persistence` |
| Integration tests | `crates/mint-core/tests/*.rs` (`--test mcp_stdio`, `--test task_lifecycle`, …) |
| Format check (CI-enforced) | `cargo fmt --all -- --check` |
| Lint (CI-enforced) | `cargo clippy --workspace --all-targets --no-deps` |
| TS typecheck | `npm run typecheck` |
| Build web UI (CI-enforced) | `npm run build:web` |
| Release link check (CI-enforced) | `cargo build --release -p mint-core -p mint-cli` |
| Run the CLI | `npm run cli -- <args>` (cached binary); use `npm run cli:dev -- <args>` for `cargo run` development mode |
| Desktop dev | `npm run dev` (`tauri dev`) |
| Web dev server (port 9000) | `npm run dev:web`; backend: `cargo run -p mint-cli -- web` |

`ci.yml` runs the CI-enforced rows above, in that order, on PRs and pushes to `main`/`master`.
It does **not** run on the `Rust` feature branch, so keep `fmt`/`clippy` clean there by hand.

This machine has limited RAM — **run heavy `cargo`/`tauri`/build commands one at a time**, not
in parallel. The `dev`/`test` profiles are deliberately trimmed to `debug = "line-tables-only"`
(deps get none) in the root `Cargo.toml` to keep linker memory down; don't "restore" full
debuginfo.

## Workspace layout

- **`crates/mint-core`** — all domain logic. `lib.rs` groups modules into subdirs
  (`agent/ media/ integrations/ search/ system/ browser/ cron/ orchestration/ prompts/ slash/`)
  then **re-exports every member flat**, so `mint_core::chat::X`, `mint_core::config::X`, etc.
  resolve regardless of which subdir the file physically lives in. Follow the existing
  `pub use group::member;` pattern when adding a module.
- **`crates/mint-cli`** — the `mint` binary. `main.rs` is the clap command tree; interactive
  chat + slash handling under `src/interactive/`.
- **`apps/mint-companion`** — optional Tauri/React character app; it connects to running Mint processes through `mint-companion-protocol`, without linking `mint-core`. For Companion transport, lifecycle, packaging, or GUI changes, read `docs/MINT_COMPANION.md`.
- **`crates/mint-companion-protocol`** — shared local wire types; changes also invalidate CLI/Desktop launcher build caches.
- **`src-tauri`** — the `mint-desktop` binary (lib crate `mint_desktop_lib`). Tauri IPC
  commands, tray, global shortcuts, screen capture, proactive suggestions, headless task queue.
  `src/lib.rs` exposes the entrypoint and existing public state types. `src/app/` owns
  startup/shutdown, command registration, tray and shortcuts. `src/commands/` groups
  domain IPC adapters; `src/native/` owns platform IO and native window/terminal state;
  `src/background/` owns Desktop background loops. Keep command names, payloads and
  event names stable when moving code, and keep domain behavior in `mint-core`.
- **`src/renderer`** — React 19 + TS + Vite frontend:
  - `src/` → desktop UI (Vite alias `@`, `vite.config.ts`, base `./`)
  - `src-web/` → web UI (Vite alias `@`, `vite.config.web.ts`, base `/`, SPA fallback to
    `index-web.html`)
  - `shared/` → components/types/css used by both (alias `@shared`)
  - `tauri.ts` in each is the platform shim: desktop calls real IPC, web calls the HTTP API.

## Architecture that spans files

**One entrypoint, three callers.** CLI, desktop IPC, and the web HTTP API
(`mint_core::api_server`, started by `mint web` / `mint api`, routes in
`api_server/routes/`) all funnel into `mint_core::orchestration`:
`orchestrate_chat`, `orchestrate_chat_stream`, `orchestrate_agent_loop` (+ `_with_fallback`
provider variants). `orchestration/mod.rs` (~3k lines) is the real agent driver; tool
implementations are in `orchestration/tools/`. `agent/agent_loop.rs` is just the JSON-action
parser and error types.

**Platform Parity Rule** (`.agents/AGENTS.md`): any new feature, behavior change, or
slash/UI option **must** be mirrored across CLI (`crates/mint-cli`), desktop UI
(`src/renderer/src`), and web UI (`src/renderer/src-web`). Verify all three before calling a
task done.

**Slash commands.** The catalog lives in `slash-commands.json` at the repo root — **edit
only there**, never hard-code a command list elsewhere. Shared behavior is in
`mint_core::slash` (`catalog.rs`, `models.rs`, `render.rs`); per-surface dispatch is
CLI `interactive/slash_commands.rs` and web `api_server/routes/slash.rs`, each with its own
fallback for surface-specific commands.

**Media model choices.** `mint_core::model_catalog` (`media/model_catalog.rs`) owns
image/video/Gemini Live credential selection, discovery and preset merging for CLI/TUI,
Desktop IPC and HTTP. A nonblank key override wins; otherwise use the saved provider
key. Discovered models come first, then missing presets. The image/video async helpers
delegate to this catalog; low-level fetchers retain provider IO, parsing, timeout and
cache. Keep adapters free of their own credential lookup or fallback lists. HTTP can
serve presets with default configuration if loading saved configuration fails.

**Learned skills.** `agent/skill_inventory.rs::list_available_skills` owns database,
global and selected-workspace discovery, location metadata and name deduplication.
Workspace entries override global entries, which override database entries; root rules
and `skills/` override their `.agents/` counterparts in pickers. Agent context retains
all mandatory rule files by canonical source path while optional skills use this precedence.
Agent context and skill reads,
Full-Screen TUI pickers, CLI `/learn`, Desktop IPC and HTTP all consume this inventory.
HTTP callers pass `workspacePath` for the workspace currently selected in the UI.

**Image persistence.** `media/image_workflow.rs::generate_and_save_images` owns generation,
data validation and saving to Pictures. It returns persisted entries with the actual
provider/model/prompt; generation and save failures propagate. CLI/TUI agent tools,
Desktop IPC and HTTP use this workflow. Adapters retain display formatting, HTTP picture
URLs and optional CLI output copies.

**Memory.** SQLite at `~/.config/mint/mint-knowledge.sqlite`, opened by `MemoryStore`
(`agent/memory.rs`). One conversation is shared across CLI + bridges, keyed by
`CHAT_CLI_ID` / `DEFAULT_CONVERSATION_ID` — it must **never** be keyed by anything a user can
change through the UI (workspace-scoping once broke this). Includes a facts table with FTS5
and semantic recall. `live_sync.rs` reconciles messages that arrive on different surfaces
(`note_own_interaction` raises the watermark so a surface doesn't re-ingest its own send).
Full design: `docs/MEMORY_ARCHITECTURE.md` — it's all in-repo, no external memory service.

**Config & safety.** `MintConfig` (`system/config.rs`) ← `mint-config.json` (git-ignored) +
`.env`. `agent/safety.rs` holds capability/permission tiers and shell-command
classification; risky actions and file writes require explicit user approval. Pending
approvals use `orchestration/approvals.rs::ApprovalRegistry` for token ownership, delivery,
cancellable waits and answer resolution. Web owns `PENDING_APPROVALS`; Desktop owns
`ApprovalsState` and emits through `src-tauri/src/commands/approvals.rs` for chat and voice. Explicitly
blank answers decline. Keep platform adapters on this shared lifecycle.

**Bridges & gateway.** `integrations/channels.rs::start_channels` starts one owned loop per
bridge; cancellation releases that ownership. `integrations/webhooks.rs` owns LINE and
WhatsApp listeners, raw-body signature verification, owner checks and acknowledgement
messages. The API serving future registers its bound address before starting bridges,
so matching callbacks share its listener while other webhook ports bind separately.
Shared callbacks check each connection's destination against the configured webhook host,
including when the API binds a wildcard address.
Provider signatures authenticate callbacks; ordinary API routes still require the API
token. Desktop starts these through its embedded API, with bridge/cron fallback on bind
failure. `mint gateway start|install` runs them headless with `GET /api/gateway/health`.

## Release

`release.yml` fires on a pushed semver tag (`v1.13.1`), builds Linux/Windows/macOS desktop
bundles + standalone CLI binaries, and publishes them to the GitHub Release. Details in
`BUILD_AND_RELEASE.md`. The CLI is distributed via npm + build-from-source (not prebuilt
download). By default, commit only — don't push, tag, publish, or start a release unless asked.
