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

---

## 🎙️ Native Voice Input — Desktop Mic Replaces Browser Speech Recognition

The desktop app's push-to-talk mic button ran on the browser's
`window.SpeechRecognition` API — Chrome/Edge-only, and effectively broken
inside Tauri's WebKitGTK webview on Linux. It's now a native Rust recorder
that works regardless of webview engine.

- **New `crates/mint-core/src/mic_transcribe.rs`**: `cpal`-based mic capture
  running on a dedicated OS thread (`cpal::Stream` isn't `Send` on every
  backend, notably ALSA, so it can't live on a tokio task) with a
  `start_recording()`/`stop_recording()` handle pair; recording is encoded to
  an in-memory WAV via `hound` at the device's native sample rate — no forced
  16kHz resampling, since a WAV header self-describes its own rate.
- **Reuses whichever provider is already configured for chat** — Gemini or
  OpenAI's existing multimodal audio support in `chat.rs`, not a separate
  Whisper API key. If the configured provider doesn't accept audio
  (Anthropic, Ollama, HuggingFace, local/custom endpoints), the mic button
  shows a specific "switch provider" error instead of silently falling back
  or faking a transcript. Deliberately calls `send_chat`, not
  `send_chat_with_fallback`, so an unsupported-attachment error can't be
  silently retried on a different provider.
- **Not the same as the existing Whisper-based transcription** —
  `crates/mint-core/src/speech.rs`'s OpenAI Whisper API → local `whisper` CLI
  → placeholder fallback chain (used for subtitle generation) is untouched
  and still exists for that separate use case.
- **New Tauri commands** (`start_mic_recording`, `stop_mic_recording_and_transcribe`)
  and a new frontend hook, `useNativeVoiceInput.ts`, replacing
  `useSpeechToText` in the desktop build's `ChatPanel.tsx` only — the `mint
  web` browser build keeps the old browser-based mic button unchanged, since
  its server can be reached from a different device than the browser
  (native recording would capture the wrong machine's mic).
- **New Linux build prerequisite**: ALSA development headers
  (`libasound2-dev` / `alsa-lib-devel` / `alsa-lib`) to build `cpal`,
  documented in the README's Linux Dependencies section.
- **Verified end-to-end** with real captured speech through the actual
  record → encode → transcribe pipeline (not just a compile check) — a
  spoken test phrase came back transcribed correctly via Gemini's audio
  input.

---

## 🎞️ Video Filmstrip & Waveform — Cheap Visual Context Instead of Frame-Dumping

Inspired by the open-source `browser-use/video-use` project's approach:
give the agent cheap visual context on a video instead of shipping the
whole file. Previously, `video_data_uri` sent a video's entire raw bytes as
base64 straight into Gemini's `inlineData` (the only provider that accepts
video at all) with no size limiting whatsoever.

- **Two new agent actions** in `crates/mint-core/src/video_edit.rs`:
  `video_filmstrip` (a grid image of frames sampled evenly across the
  timeline, composited with the `image` crate — no new dependency) and
  `video_waveform` (an audio amplitude image via ffmpeg's built-in
  `showwavespic` filter) — a couple of small PNGs instead of the whole file,
  and it works on any vision-capable provider, not just Gemini.
- **Found and fixed a real vision-attachment gap while building this**: an
  image only ever reached the model as something it could actually *see* on
  a task's very first turn (attached from the frontend before the loop
  starts). A `step_images` mechanism already existed to attach a **mid-task**
  tool result as real vision on the next turn, but only `browser_screenshot`
  used it — the existing `view_image` tool returned a JSON-wrapped data URI
  that failed the attach check, so its image was dumped as unreadable base64
  text instead of actually being seen. Generalized the check in
  `orchestration.rs` to cover `browser_screenshot | video_filmstrip |
  video_waveform | view_image`, and simplified `view_image` to return the
  bare data URI so it benefits too.
- Both new actions registered in `prompts/agent.rs`'s allowed-actions list
  (including plan mode's read-only allowlist, alongside `view_image` and
  `browser_screenshot` — generating an inspection image doesn't modify
  project files) and `prompts/tool_catalog.rs`'s native tool-calling schemas.
- **Verified end-to-end** against a real synthetic test video: the agent
  correctly read a moving on-screen counter from the generated filmstrip and
  correctly located a silent audio gap from the generated waveform — genuine
  visual reasoning grounded in the actual pixels, not placeholder text.

---

## 🔌 Companion Service Shortcuts — `/n8n` and `/notebook`

Two self-hosted open-source projects — [n8n](https://n8n.io) (workflow
automation) and [SurfSense](https://github.com/MODSetter/SurfSense) (a
self-hosted NotebookLM alternative) — can now be driven straight from Mint's
agent, each wired in as its own MCP server. Neither ships with Mint; both
are separate projects you clone and run yourself, connected the same way any
third-party MCP server is.

- **New slash commands `/n8n [task]` and `/notebook [task]`**
  (`crates/mint-cli/src/interactive/slash_commands.rs`): with no task, each
  checks whether its service is actually reachable (a 300ms TCP probe via a
  new `is_reachable()` helper) and opens it in the browser
  (`crate::actions::open_system_handler`); with a task, it forwards to the
  agent loop tagged `[n8n]`/`[notebook]` — the same `ForwardToAgent` pattern
  `/code` already uses — so the model reaches for that service's MCP tools
  specifically.
- **Guarded against the obvious silent-failure case**: both commands check
  `mint mcp list` for a server registered under the exact name `n8n` /
  `surfsense` before forwarding a task, so a task never gets silently sent
  to an agent with no matching tools to call.
- **New inline status panel instead of walls of warning text**
  (`render_companion_status_panel`): reuses the same "flash an inline
  `ratatui` `Terminal`, draw once, `clear()` to finalize into scrollback"
  pattern `picker.rs` already uses for `/mcp` and `/models`, just without
  the interactive redraw loop since there's nothing to select — a colored
  `●`/`○` dot per service plus its MCP-connection status and a setup-doc
  pointer, shown whenever either command can't proceed.
- **New setup docs** (`docs/N8N_INTEGRATION.md`,
  `docs/SURFSENSE_INTEGRATION.md`): full clone-and-run steps for each
  project, how to register it with `mint mcp add`, and — for n8n
  specifically — a correction that its MCP support moved from a per-workflow
  "MCP Server Trigger" node (older versions) to a single instance-wide
  `/mcp-server/http` endpoint authenticated by a dedicated MCP API key
  (distinct from n8n's general Public API key, which shares the same JWT
  shape and is easy to grab by mistake), found by inspecting a running n8n
  v2.34.6 container directly since public docs hadn't caught up yet.
- **Fixed a real `mint mcp add --args` parsing bug found while wiring this
  up** (`crates/mint-cli/src/main.rs`): `args`/`env` were declared with
  `num_args = 0..` (unbounded) plus `allow_hyphen_values`, which made clap
  unable to distinguish a fresh `--args` occurrence from a hyphen-prefixed
  value continuing the previous one — passing a value like `--header` (needed
  to bridge n8n's MCP endpoint through `mcp-remote`) silently swallowed every
  following `--args`/value as literal text instead of starting a new
  occurrence. Pinned both to `num_args = 1` (one value per occurrence,
  still repeatable) to remove the ambiguity entirely.

---

## 🖥️ Headless Gateway Mode — Run Mint 24/7 on a VPS

- **`mint gateway start [--api-port <N>]`**: headless mode — bridges + cron,
  no TUI, no TTY required. Clean shutdown on Ctrl+C and SIGTERM.
- **`mint gateway install [--system] [--now] [--memory-max <size>]`**:
  registers a systemd unit (per-user by default, `--system` for root-level).
  Hardened out of the box (`TasksMax`, crash-loop limits, `NoNewPrivileges`);
  `--memory-max` is opt-in, not a guessed default.

## 📡✉️ Signal & Email Bridges, Unified Cross-Channel Memory

- **Signal**: talks to a self-hosted `signal-cli-rest-api` instance (no
  official bot API exists). **Email**: reuses the existing Gmail OAuth
  connection (`mint gmail auth`) instead of separate IMAP/SMTP setup. Both
  configurable via `mint onboard`.
- **All seven bridges now share one memory thread** with the terminal CLI
  instead of one per platform — pick up a Telegram conversation from the
  terminal and vice versa.

## 🩺 Bridge Reliability — Panic Isolation & Health Monitoring

- A panic inside any bridge loop used to kill it silently and permanently.
  `restarting_loop` now catches panics too, not just errors, and retries
  with backoff like normal.
- **New `GET /api/gateway/health`**: each bridge's enabled state, last
  success/error, and failure count as JSON — check status remotely without
  SSHing in.

## 🔐 Opt-in API Authentication + a Real Bug Fixed Along the Way

- **`apiAuthToken`** config value gates the whole local API server: set it
  and every request needs `Authorization: Bearer <token>` or gets `401`.
  Unset by default — no change for existing desktop/`mint web` users.
- **Bonus fix**: `mint api` and `mint gateway start --api-port` were both
  starting every bridge *twice* (duplicate Telegram polling, duplicate
  Discord sessions) — `start_api_server` already starts them internally.
  Found while wiring up the auth check above; now fixed.

---

## 💬 `ask_user` — Descriptions, Multi-Select, and a Claude-Code-Style Picker

The agent's `ask_user` tool could already offer up to 3 plain-text options
plus an always-available free-text fallback. This round brings it closer to
feature parity with Claude Code's own `AskUserQuestion` tool — richer option
data, a matching visual style in both the CLI and desktop/web GUI, and
explicit guidance on when the agent should actually reach for it.

- **Per-option `description`, `multiSelect`, and a short `header` tag**
  (`crates/mint-core/src/orchestration/mod.rs`): new `AskUserOption { label,
  description }` struct on `AgentApproval::AskUser`, plus `header:
  Option<String>` and `multi_select: bool` (wire name `multiSelect`).
  Decoding uses an untagged `AskUserOptionInput` enum (`Plain(String) |
  Detailed{label, description}`) so a model that still emits the old bare
  `["a","b"]` array shape keeps working — no breaking change for in-flight
  models. `ApprovalOutcome` itself is untouched (still just `Approved |
  Denied | Intercepted(String)`, shared by every other approval type); a
  multi-select answer is pre-joined by the CLI/GUI into one canonical string
  (`"A, B"`, or `"A, B — <free text>"` if the user also typed something)
  before it ever becomes `Intercepted`.
- **Native tool schema and legacy prompt both updated**
  (`prompts/tool_catalog.rs`, `prompts/agent.rs`) to document the new
  `header`/`multiSelect`/`{label, description}` shape for both the
  native-tool-calling and prose-JSON code paths.
- **CLI picker restyled to match Claude Code's own look**
  (`crates/mint-cli/src/agent/approval_prompts.rs`): a highlighted header
  chip, the question wrapped in a left-bordered quote block
  (`textwrap`-powered, sized to the real terminal width), bold option
  labels with dimmed descriptions underneath, and — new — an explicit,
  navigable **"Chat about this"** row at the bottom of the list instead of
  free text only being reachable via a hidden "type any key" gesture. Ships
  for single-select (`run_option_picker`), multi-select checkboxes
  (`run_multi_option_picker`, new), and the non-raw-mode numbered fallback
  alike.
- **Desktop & Web GUI** (`ApprovalCard.tsx`): numbered, described option
  buttons; a header chip; multi-select toggle buttons that build the same
  canonical joined answer the CLI does, submitted alongside (not gated
  behind) an always-visible free-text box.
- **New usage guidance on the tool itself**: both prompt paths now tell the
  model to call `ask_user` *only* when genuinely blocked on a decision that
  is the user's to make — not resolvable from the request, the code, or a
  sensible default — and explicitly not to use it for permission-to-proceed
  or confirmation, which the existing approval/plan-review flow already
  handles.

---

## 🐳 Docker Sandbox for Subagents

`dispatch_subagent`-spawned subagents can now run their shell commands inside
an isolated Docker container instead of the shared bwrap/sandbox-exec
host-level sandbox — the biggest sandboxing gap the agent had, since a
subagent is exactly the piece of the system most likely to run less-trusted,
model-generated shell commands.

- **New `crates/mint-core/src/system/docker_sandbox.rs`**: one container per
  subagent *session*, not per command — `start_session` starts a single
  detached container keyed by `sub_chat_id`, and every subsequent
  `run_shell` call from that subagent `docker exec`s into it instead of
  paying container-startup latency per command. Modeled directly on
  `integrations::mcp`'s `SESSIONS` registry pattern rather than
  `bg_shell`'s job registry, which has no reuse/lifecycle semantics.
- **Ref-counted teardown**: `run_parallel_subagent_batch` can dispatch the
  same subagent name twice concurrently (it doesn't dedupe names), so two
  callers can share one container. `stop_session` now decrements a
  reference count and only actually stops/removes the container once every
  caller that started it has also stopped it — without this, whichever
  concurrent dispatch finished first would tear the shared container down
  out from under the other one still using it.
- **Filesystem policy stricter than bwrap by design**: only
  `allowedWritePaths`/`allowedReadPaths` get bind-mounted (rw/ro
  respectively) rather than exposing the whole host filesystem read-only
  the way bwrap's `--ro-bind / /` does — a container ships its own
  `/usr`/`/bin`, so there's no need to also expose the host's.
- **New config**: `sandboxBackend` (`"os"` default | `"docker"`) and
  `dockerSandboxImage` (default `debian:bookworm-slim`), settable via `mint
  config set`; a subagent definition's own `sandbox: docker` frontmatter
  field overrides the global setting per subagent. `mint config doctor`
  reports a new `dockerSandbox` block (backend/image/availability).
- **Verified against a real Docker daemon**, not just compile-checked: unit
  tests actually start a container, `docker exec` into it, confirm
  `/.dockerenv` is visible, and confirm `docker ps -a` shows nothing left
  behind after teardown — including a dedicated regression test for the
  ref-counting fix and an error-path test proving teardown still runs when
  the subagent's run fails.

---

## 📱 Web UI, Installable as a PWA

The Web UI can now be installed to a phone's home screen like a native app —
no app store, no separate mobile codebase — the lowest-effort step toward
mobile access ahead of a heavier Tauri-mobile investment.

- **New web app manifest** (`manifest.webmanifest`) and a generated icon set
  (192×192, 512×512, and a padded 512×512 maskable variant) so Android/iOS
  "Add to Home Screen" gets a proper name, theme color, and icon instead of
  a generic browser bookmark.
- **A narrowly-scoped service worker** (`public/sw.js`): stale-while-revalidate
  for static assets (safe since Vite already content-hashes built
  filenames, so a new deploy's JS/CSS never collides with a stale cache
  entry), a network-first app-shell fallback for navigations when the
  network drops mid-use — and `/api/*` is **never** intercepted, since
  caching or replaying agent/chat responses would be actively wrong, not
  just stale.
- **Production-only registration**: `registerServiceWorker()` is gated on
  `import.meta.env.PROD` so it never registers under Vite's dev server,
  where it would otherwise fight with HMR.
- **Desktop (Tauri) build is untouched** — the manifest/service worker only
  ever get referenced from `index-web.html`/`src-web`, not the desktop
  `index.html`/`src`.
- **Verified in an actual browser**: driven headlessly through Chrome
  DevTools Protocol against a real production build (`vite preview`) —
  confirmed the service worker registers, the manifest resolves as valid
  JSON with the right name/icons, and the page loads with zero console
  errors or warnings.

---

## 🔗 Cross-Reference Links for Linked Folders

Notes written into a linked folder can now reference each other, closer to
the Obsidian-style web of notes Linked Folders was originally inspired by
instead of a flat, unlinked pile of daily files.

- **The note-writing reflection call now sees recent existing entries**
  (`crates/mint-core/src/search/linked_folders.rs`, up to 15 per candidate
  folder) and can wiki-link a new note to ones it names as genuinely
  related — `id` format is `"YYYY-MM-DD#HH:MM"`, deliberately matching the
  real `## HH:MM` heading each entry is already written under, so
  `[[2026-08-10#09:00]]` resolves if the folder is ever opened as an actual
  Obsidian vault, not just inside Mint.
- **Hallucination-safe**: any id the model returns that wasn't in the
  candidate list it was actually shown gets silently dropped rather than
  written into the note as a permanently broken link.

---

## 🧬 Self-Evolving Skills

Auto skill writing now behaves closer to "creates skills from experience,
improves them during use" instead of a one-shot write-and-forget.

- **On by default**: `auto_skill_writing` now defaults to `true` (was
  `false`) — `/autoskill off` or the Settings toggle still turns it off, but
  the self-improving behavior it exists for no longer requires a user to
  discover and enable it first.
- **Genuine refinement instead of a blind overwrite**
  (`orchestration/memory_skill.rs`): when the reflection call's chosen slug
  matches an existing workspace skill, `auto_write_skill` now shows the
  model that skill's full current `SKILL.md` content and asks for a real
  merge — keep what's still correct, incorporate what's new — rather than
  risking a from-scratch rewrite that silently discards a skill's earlier,
  hard-won content.
- **`revisions: N` frontmatter, computed in code, not by the model**: every
  write stamps a revision count — read the previous file's own count, add
  one — deterministic rather than trusted to an LLM's arithmetic across
  calls, so there's visible, verifiable evidence a skill has actually
  evolved over repeated invocations.
