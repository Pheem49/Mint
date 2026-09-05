# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Mint is

A local-first AI assistant with **one Rust core, three surfaces**: a terminal CLI, a Tauri v2
desktop app (with a Live2D companion), and a web UI. The same chat/memory/agent/tools/safety
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
| Run the CLI | `cargo run -p mint-cli -- <args>` (alias: `npm run cli -- <args>`) |
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
- **`src-tauri`** — the `mint-desktop` binary (lib crate `mint_desktop_lib`). Tauri IPC
  commands, tray, global shortcuts, screen capture, proactive suggestions, headless task queue.
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
approvals: `PENDING_APPROVALS` in `lib.rs` for web, `ApprovalsState` in `src-tauri/src/lib.rs`
for desktop — keep the two resolution paths in sync.

**Bridges & gateway.** `integrations/channels.rs::start_channels` runs all messaging bridges;
`mint gateway start|install` runs them headless as a systemd service with a
`GET /api/gateway/health` endpoint.

## Release

`release.yml` fires on a pushed semver tag (`v1.13.1`), builds Linux/Windows/macOS desktop
bundles + standalone CLI binaries, and publishes them to the GitHub Release. Details in
`BUILD_AND_RELEASE.md`. The CLI is distributed via npm + build-from-source (not prebuilt
download). By default, commit only — don't push, tag, publish, or start a release unless asked.
