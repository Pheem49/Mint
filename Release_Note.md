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

---

## 🛡️ Agent-Initiated Plan Mode

Plan mode used to be something only the user could switch on. The agent can now
propose switching into it itself, mid-task — subject to the same kind of
approval gate as any other risky action.

- **New `enter_plan_mode` action** (`crates/mint-core/src/prompts/agent.rs`,
  `prompts/tool_catalog.rs`) — offered to the model only while plan mode is
  off, symmetric to the existing `exit_plan_mode` (offered only while it's on),
  so the two can never both appear at once. The system prompt tells the agent
  to call it before its first mutating action on anything that looks like a
  multi-file refactor, a migration, deletions, or another hard-to-reverse
  change.
- **New `AgentApproval::EnterPlanMode`** (`orchestration.rs`) gates the switch
  behind the same approval flow as every other risky action — declining or
  leaving feedback keeps plan mode off and the agent proceeds directly.
  Unattended contexts (cron jobs) auto-approve it like everything else there.
- **CLI approval card matches every other picker-style approval** (`agent.rs`):
  both `EnterPlanMode` and `ExitPlanMode` now render through the same
  arrow-key `run_option_picker` used for `AskUser`/file-write approvals
  (↑/↓ + Enter, type to answer freely, Esc to decline) instead of the old
  plain `[y/N]` text prompt.
- **Status bar reflects plan mode**: the interactive composer's bottom-line
  label switches from `[Agent]` to `[Plan]` whenever `session.plan_mode` is on
  (`interactive/input_box.rs`).

---

## ⏰🌏 Scheduled Tasks — Fixes & Timezone Support

Two real bugs found and fixed in the cron system shipped above, plus proper
timezone handling and a friendlier way to create a job.

- **Fixed: orphaned empty conversations in the chat sidebar.** `CronStore::add`
  pre-creates the job's conversation up front (so it's titled in the sidebar
  immediately) via `MemoryStore::open_default()` — which, unlike
  `CronStore`'s own store, has no test-scoped path override. Every
  `cargo test` run was silently leaving real, permanent, empty conversation
  rows in the developer's *actual* chat database. Fixed with a `cfg!(test)`
  guard on both sides (`cron/store.rs`), matching the existing pattern already
  used in `memory.rs`.
- **Fixed: deleting/replacing a task could still leave an empty conversation
  behind.** New `MemoryStore::delete_chat_session_if_empty` (`memory.rs`) is
  now called from `CronStore::remove` — cleans up the placeholder only if the
  job never actually ran (zero interactions), leaving real report history
  untouched for jobs that did run.
- **Timezone-aware scheduling, backend and frontend:**
  - New `mint_core::cron::localize_schedule()` (`cron/schedule.rs`, built on
    `chrono-tz`) converts a wall-clock time in an explicit IANA timezone into
    the UTC cron expression the scheduler actually evaluates against —
    correctly handling daily/weekly/monthly/one-time shapes and DST (verified
    against real `America/New_York` EST/EDT transitions in tests).
  - `mint cron add --name/--schedule/--task --timezone <IANA zone>` and
    `/cron add <name> | <schedule> | <task> | [timezone]` (a new optional 4th
    field) now do this conversion automatically instead of requiring the
    schedule to already be written in UTC.
  - The Desktop/Web "New Scheduled Task" form (`ScheduledTasksView.tsx`) does
    the same conversion client-side via `luxon`, plus a new **Timezone**
    dropdown (defaults to the device's zone, not locked to it) — previously
    the time picker silently assumed UTC, so picking "08:00" could fire at a
    different hour than intended depending on the browser's real timezone.
- **New interactive wizard**: running `mint cron add` or `/cron add` with no
  arguments now walks through name → repeat type (daily/weekly/monthly/
  one-time/custom) → time → weekday/day-of-month/date → timezone
  (auto-detected via `iana-time-zone`, editable) → task → workspace, reusing
  the onboarding flow's own `prompt_choice`/`prompt_input` helpers
  (`cron_wizard.rs`). The flag-based and pipe-delimited forms still work
  unchanged for scripting.
- **`/cron add` is now its own entry** in the CLI's slash-command suggestion
  list, not just documented under the parent `/cron` entry.

---

## 🔧 `mint setup` — Full Tool Coverage

`mint setup`'s tool-enable wizard had drifted out of sync with the agent's
actual tool catalog as new tools were added over time — 21 real tools had no
way to be toggled off through it at all. `crates/mint-cli/src/setup.rs` now
lists all 49 tools from `base_allowed_actions()`, in the same order, verified
by direct comparison: `image_search`, `weather`, `stock`, `calculation`,
`mcp_list_tools`, the full video/subtitle/audio editing set (`video_trim`,
`video_remove_silence`, `video_resize`, `video_merge`, `video_export`,
`video_extract_audio`, `speech_transcribe`, `subtitle_generate`,
`subtitle_translate`, `subtitle_burn`, `timeline_reorder`,
`effect_zoom_on_speaker`, `audio_duck_music`, `make_shorts`), and
`generate_image`/`generate_video`.

---

## 🔒 Messaging Bridges — Owner Allowlist (Security Fix)

Found and fixed a real gap: none of Telegram, Discord, Slack, LINE, or
WhatsApp verified *who* was messaging the bot. Any stranger who could reach
one — DM a public Telegram bot, share a Discord server or Slack workspace
with it, message a LINE/WhatsApp account — could trigger `answer_channel`'s
agent loop, which auto-approves every action (`write_file`, `apply_patch`,
`run_shell`, …) since there's no human present on a bridge to click approve.

- **New `authorize_sender()`** (`channels.rs`): the first sender any bridge
  ever hears from is claimed as its owner (persisted to
  `config.extra["<platform>OwnerChatId"/"OwnerUserId"/"OwnerPhone"]`);
  everyone else is silently ignored from then on. Zero setup required. Wired
  into all five loops (Telegram `from.id`, Discord `author.id`, Slack
  `event.user`, LINE `source.userId`, WhatsApp `message.from`).
- Decision logic split into a pure, directly-testable `sender_authorization()`
  core so tests never touch the real on-disk config (the same class of bug
  fixed in cron's tests above).
- `README.md`'s Safety And Privacy section documents the behavior and how to
  reset an owner (`mint config set telegramOwnerChatId ""`).

---

## 📣 README Repositioning — Reach Is The Headline

Messaging-bridge reach was buried as feature #6 of 9; it's Mint's most
distinctive capability (a local agent reachable from a chat app, not just a
terminal or desktop window), so `README.md` now leads with it: the top
tagline, intro paragraph, feature-list ordering (now #1), and Highlights list
all foreground it, done only after the owner-allowlist fix above closed the
security gap that would have made a louder headline risky.

---

## 🎧 Multimodal — OpenAI Audio Input

Audio attachments were Gemini-only; OpenAI's real API supports them too via
`input_audio` content parts, so `crates/mint-core/src/chat.rs` now builds that
payload shape for it (`openai_audio_part`, wired into both the single-turn and
native multi-turn message paths).

Scoped honestly rather than claimed as blanket "provider-agnostic multimodal":
video stays Gemini-only, since OpenAI/Anthropic have no native video
ingestion at all in their chat APIs — a genuine provider capability gap, not
an integration gap. Audio also stays unsupported for `local_openai`,
`openrouter`, `deepseek`, and `huggingface` — proxies/other services on
similar wire formats that aren't confirmed to speak the same `input_audio`
schema as the real OpenAI API.

---

## 🎯 Agent Honesty — Don't Report Success On A Real Failure

The existing "did you verify your changes" gate only checked that the `verify`
tool had been *called* after the last edit — not whether it actually passed.
An agent could run `verify`, see real test failures in the output, and still
call `finish` claiming success, and nothing would catch it. That matters more
for Mint than for an interactively-watched coding assistant: a task run from
a scheduled job or a messaging bridge has nobody watching live to notice the
lie until they check back later.

- **Fixed a real bug along the way**: the in-context "your command failed"
  nudge that fires right after `run_shell`/`verify` only scanned the *first*
  `"exit: "` line in the result before stopping — a multi-command `verify`
  call where an earlier command passed but a later one failed never
  triggered it. New `shell_result_failed()` (`orchestration.rs`) scans every
  line instead.
- **New hard gate at `finish` time**: `last_verify_failed` now tracks whether
  the most recent `verify` call actually passed, separately from whether it
  merely ran. `unacknowledged_verify_failure()` rejects `finish` outright if
  the last verify failed and the agent's `finish.verification` field says
  nothing about it — forcing the agent to either fix the problem, re-verify
  successfully, or explicitly explain the failure (e.g. "pre-existing,
  unrelated to this change") rather than silently claiming success over it.
- Mirrors the existing `unverified_modification` gate's shape and escape
  hatch (a non-empty, non-placeholder `verification` field satisfies it) so
  the two gates behave consistently.
- **Known limit, stated plainly**: this stops silent success claims, not a
  sufficiently motivated *false* explanation typed into the verification
  field — a text-based gate can't fully replace actually re-running the
  check itself, which is out of scope here.
