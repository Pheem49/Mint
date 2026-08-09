# Release Notes - Mint Agent v1.12.0

We are excited to release **Mint Agent v1.12.0**! This version focuses on closing
the gap with modern self-hosted agent products: the agent can now **teach itself
new skills**, **run tasks on a schedule** without any manual trigger, and **keep
running notes in folders you care about** as you chat — each shipped end-to-end
across the Rust core, CLI, interactive chat, and both Desktop/Web GUIs.

---

## 🧠 Autonomous Skill Acquisition (Auto Skill Writing)

Mint's agent can now write its own reusable skills after solving a hard problem,
instead of only being taught skills manually via `mint learn`.

- **Heuristic-gated reflection call (`crates/mint-core/src/orchestration.rs`)**:
  `looks_skill_worthy()` cheaply pre-filters on step count (≥3) and whether a
  substantive action ran (`apply_patch`, `write_file`, `run_shell`,
  `browser_*`, `dispatch_subagent`) before spending a second LLM call. Only then
  does `spawn_auto_skill_write` / `auto_write_skill` — modeled on the existing
  `spawn_auto_memory_update` reflection pattern — ask the model whether the task
  was genuinely reusable and, if so, write a `.agents/skills/<slug>/SKILL.md` file
  with YAML frontmatter, picked up automatically by the existing
  `learned_skills_context()` skill-discovery system next session.
- **Opt-in config toggle**: `auto_skill_writing` (default **off** — it costs an
  extra LLM call and writes files). Togglable via `mint config set
  autoSkillWriting true`.
- **CLI slash command (`/autoskill`)**: `crates/mint-cli/src/interactive.rs` —
  `/autoskill on|off` for direct toggling, or `/autoskill` alone opens an
  arrow-key interactive selector (`prompt_interactive_select`, ↑/↓ + Enter) that
  pre-selects the current state.
- **Desktop & Web Settings toggle**: New "Self-improvement" section in
  `AutomationTab.tsx` (both `src/renderer/src` and `src/renderer/src-web`) with a
  standard toggle switch and explanation of when it fires.

---

## ⏰ Cron Scheduling — Recurring Agent Tasks

A full scheduling subsystem so agent tasks can run automatically on a cron
expression — no OS-level daemon required, no manual trigger needed.

- **New `crates/mint-core/src/cron/` module**: `store.rs` (`CronJob`/`CronStore`,
  a JSON-file-backed store mirroring `TaskStore`'s shape), `schedule.rs` (wraps
  the `cron` crate, normalizes 5-, 6-, and 7-field expressions so plain
  `min hour dom month dow` cron syntax "just works"), `scheduler.rs` (a
  self-healing tick loop modeled directly on `channels.rs`'s
  `start_channels()`/`restarting_loop` pattern).
- **No daemon needed — rides along on whatever's already open**: the scheduler
  auto-starts at the exact same points `start_channels()` does (interactive CLI,
  `mint api`/`mint web`, and the desktop app), so cron jobs fire as long as one of
  those is running — matching how messaging bridges already behave, with zero new
  process-management code.
- **Unattended execution with a safety net**: cron runs auto-approve every agent
  action for convenience, except any explicit "always deny" permission rule you've
  already set (`MintConfig::permission_decision`) — that's still honored.
- **CLI (`mint cron`)**: `add --name --schedule --task --workspace`, `list`,
  `show <id>`, `remove <id>`, `enable`/`disable <id>`, and `run-now <id>` to fire a
  job immediately without waiting on its schedule.
- **Interactive slash command (`/cron`)**: `list | add <name> | <sched> | <task> |
  remove <id> | enable <id> | disable <id>`.
- **Desktop & Web GUI — new "Scheduled Tasks" page**: `ScheduledTasksView.tsx`
  (`src/renderer/shared/components/`), reachable from the sidebar's "More" menu
  next to Skills/MCP/Plugins. Full CRUD with an enable/disable toggle switch and
  last-run status per job, backed by new Tauri commands
  (`list_cron_jobs`/`add_cron_job`/`remove_cron_job`/`set_cron_job_enabled`) and
  REST endpoints (`GET/POST /api/cron`, `DELETE /api/cron/:id`,
  `POST /api/cron/:id/enable|disable`) for the web build.

---

## 📁 Linked Folders — Auto Note-Taking

Link a named folder (e.g. "Food", "YouTube") to Mint, and chat that touches on its
topic gets a short note written into it automatically — inspired by the same
capability in Hermes Agent.

- **New `crates/mint-core/src/linked_folders.rs`**: `LinkedFolder`/
  `LinkedFolderDraft` stored under `config.extra["linkedFolders"]`, mirroring the
  MCP-servers storage pattern (`add_linked_folder`/`remove_linked_folder`/
  `list_linked_folders`) — the only existing precedent in the codebase for a
  by-name CRUD list with real CLI verbs.
- **Reflection hook on every chat surface**: `spawn_linked_folder_note` is wired
  into all five places `spawn_auto_memory_update` already lives (all four
  `orchestrate_chat*` functions plus `orchestrate_agent_loop`'s finish block) —
  necessary because interactive CLI, desktop, and web chat all default through the
  agent loop, not the plain-chat functions. A cheap case-insensitive keyword
  pre-filter against each folder's name/description skips the LLM call entirely
  unless there's a plausible topic match.
- **Notes land in a dedicated subfolder**: writes go to
  `<folder>/mint-notes/<YYYY-MM-DD>.md`, appended under a `## HH:MM` heading per
  entry, so your real files never get mixed with Mint's notes.
- **Safety**: both linking a folder and writing a note re-check
  `assert_path_capability` (the same guard used for the agent's file-write tools),
  so linking or silently writing into a blocked path (e.g. `~/.ssh`) is rejected.
- **CLI (`mint link`)**: `add <name> <path> [--description ...]`, `list`,
  `remove <name>`.
- **Interactive slash command (`/link`)**: `list | add <name> | <path> |
  <description> | remove <name>`.
- **CLI save indicator**: a background notices queue
  (`mint_core::LINKED_FOLDER_NOTICES`, mirroring the `/bg` job-notices pattern)
  surfaces a `Saved note to <folder> (<path>)` line in the interactive prompt's
  idle tick whenever a note gets written, so it's never a silent operation.
- **Desktop & Web GUI — new "Linked Folders" page**: `LinkedFoldersView.tsx`,
  reachable from the sidebar's "More" menu next to Scheduled Tasks. Add/remove
  folders with a name, path, and optional description; the Desktop build adds a
  **Browse...** button next to the path field that opens a native folder picker
  (reusing the existing `select_workspace_directory` Tauri command) — omitted on
  Web, since browsers cannot expose a real filesystem path from a folder picker.
