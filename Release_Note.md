# Release Notes - Mint Agent v1.13.1

A reliability-and-polish release: scheduled tasks no longer lose a run when
the app is closed mid-execution, the five management pages (Scheduled Tasks,
Skills, MCP Servers, Plugins, Linked Folders) were rebuilt as one consistent
list-style UI, typing a `~/…` path when linking a folder finally works, the
web UI's "Browse…" button now opens a real folder picker, Settings' General
and Plugins tabs were cleaned up (and image generation gained per-provider
model pickers), the half-finished Custom Workflows feature was removed, and
a new `/init` command writes an AGENTS.md for the project.

This build also adds a local-first cross-session memory (a structured facts
table plus full-text recall of older messages), corrects the per-provider
context-window sizes and makes the local ones configurable, raises the
context-compaction trigger so long tasks keep more verbatim history, brings
the chat composer's up/down history, persistent per-conversation drafts, and
edit-and-resend to Web and Desktop, finishes the Settings component restyle
(segmented pills / bordered toggle cards) across the remaining tabs, and
gives the CLI's interactive prompt readline-style editing keys plus prompt
history that survives a restart.

---

## ⏰ Fixed: Scheduled Tasks Lost a Run When Mint Closed Mid-Execution

The cron scheduler advanced a job's `next_run` to the following occurrence
*before* running it. If the host process exited during that run — the desktop
app closed, the machine restarted — the occurrence was gone for good:
`next_run` had already moved past it and nothing recorded a failure. Opening
the app briefly to check on a task burned one run per open, which is why a
daily job could sit at "last run 2 days ago" while looking healthy.

- **`next_run` now advances only after the run finishes** (`finish_run`), so
  an interrupted run still has `next_run` in the past on the next scheduler
  pass and simply runs again.
- **A `running_since` claim** stamped before each run lets a second Mint
  process (a desktop app plus `mint gateway`, say) skip a job another
  scheduler is already running; a claim older than an hour is treated as a
  dead process and the occurrence is retried rather than left stuck.
- The Scheduled Tasks detail view surfaces `Running since` while a run is in
  flight.

---

## 🧹 Management Pages Rebuilt as One Consistent UI

Scheduled Tasks, Skills, MCP Servers, Plugins, and Linked Folders each had
their own take on the same "list of things with a detail modal" pattern —
tall gradient cards tiled 2–3 across, emoji empty states, multi-sentence
explainer subtitles, and bordered pill badges that read as generic template
chrome. They now share one design:

- **Compact single-column rows** — scan by name, open the detail modal for
  everything else. MCP Servers and Plugins keep their icon rows but stop
  tiling into a grid.
- **Plain-text status and labels** instead of pill-with-dot badges; trimmed
  page subtitles; no decorative emoji in empty states.
- **Wider detail modal** (540px → 720px) across every management page.
- **Real markdown in the detail modal**: a scheduled task's "Last response"
  and a skill's body now render through the same renderer chat uses —
  tables, stock cards, headings — instead of being dumped as raw text.

---

## 🗂️ Fixed: `~/…` and Relative Paths Rejected When Linking a Folder

`add_linked_folder` checked `path.is_dir()` on the raw string *before* the
`~/` and relative-path expansion that happens inside the safety check — so
typing `~/notes/food` (exactly what the field's placeholder suggests) always
failed with "path does not exist or is not a directory". It now resolves the
path first, then checks it's a real directory, and reports the resolved path
in the error. Fixes typed input on Desktop and Web, and `mint link add` /
`/link add` in chat. The desktop "Browse…" button was unaffected either way.

---

## 📂 "Browse…" Folder Picker Now Works on the Web UI

A browser can't hand back a real filesystem path, so the web build's
"Browse…" button previously did nothing. But `mint web` runs on the same
machine as the browser pointed at it, so:

- **New `POST /api/select-folder`** opens the host's own native directory
  dialog (`zenity`/`kdialog` on Linux, `osascript` on macOS) and returns the
  chosen path.
- **Gated to loopback callers** — a `mint web` instance reachable from other
  machines can't be made to pop a dialog on the server host; remote web
  sessions get no button rather than a dead one.
- Desktop keeps its existing native Tauri picker.

---

## ⚙️ Settings: General/Plugins Tab Cleanup + Per-Provider Image Models

The two heaviest settings tabs were still carrying pre-redesign markup — ~90
inline `style={{…}}` blocks between them, and hardcoded colors that broke the
light and midnight themes (a Veo model `<select>` was pinned to white text on
a translucent-white background).

- **GeneralTab & PluginsTab** now use the same section / row / card
  primitives as the other tabs. The accordion chrome, plugin cards, badges,
  and icon buttons all moved to CSS classes; a provider picker with many
  options now spans the full row and wraps naturally instead of shrinking
  into a lopsided box.
- **Image Generation** gained a card per provider (NanoBanana, DALL·E,
  Stability, Ideogram, Replicate, BFL) — mirroring the chat "Provider &
  Model" section — each with a **model dropdown** plus its API-key field or a
  shared-key note. Five new default-model config fields back it; the Rust
  side already had matching fields, and each image call reads its provider's
  configured model.

---

## 🗑️ Removed: Custom Workflows / "If This Then Mint"

The workflow engine was desktop-only, Unix-only (trigger detection shelled
out to `ps -A`), and had a single trigger type — "is process X running" —
that fired a proactive suggestion rather than running anything. Not worth the
maintenance surface.

Gone: the 15-second monitor thread, `workflows.json` read/write, the
Settings > Automation "Custom Workflows" section, the `enableCustomWorkflows`
flag, the sidebar "Workflow (Beta)" entry, the Workflow Builder panel (and a
dead standalone window), and the related Tauri commands. The `/n8n` slash
command and n8n MCP integration are unrelated and untouched.

---

## 📝 New: `/init` Writes an AGENTS.md for the Project

`/init` (CLI, web, desktop) mirrors Claude Code's `/init`: it asks the code
agent to scan the codebase and write — or extend — an `AGENTS.md` at the
workspace root, the file every Mint surface already loads as workspace
rules. It captures the build/test/lint commands, the high-level
architecture, project conventions, and gotchas, folding in any existing
rules file rather than replacing it.

---

## 🧠 New: Local-First Cross-Session Memory

Mint now keeps a structured memory in `mint-knowledge.sqlite` and pulls the
relevant parts into every turn — no external service.

- **`facts` table** — typed `user` / `preference` / `project` entries with a
  dedup index and supersede/retract semantics, injected live on each turn on
  top of the existing recent-conversation window.
- **`interaction_fts`** — an FTS5 index over past interactions; each turn
  also injects the few older messages most relevant to what you just typed
  (BM25 ranked, injection-safe `MATCH` builder).
- **Slash commands** (shared engine + CLI): `/remember [here] <text>`,
  `/memory facts`, `/memory forget <id>`, `/autorecall [on|off]`. Gated by
  `config.memory_recall`.
- The long-dead `interaction_memories.keywords` column was dropped.

---

## 📏 Fixed: Context-Window Sizes + Configurable Local `num_ctx`

`context_window_tokens()` was reporting 1M for Anthropic and OpenAI — far
above what the default models actually offer, so history was allowed to grow
past the real limit before compaction stepped in.

- **anthropic** — 200K for every model (1M is a tier-gated beta).
- **openai** — keyed off the model string (gpt-4.1 ≈ 1M, gpt-5 400K,
  o-series 200K, else 128K).
- **deepseek** — explicit 128K.
- **ollama / openrouter / local_openai** — read a configurable `*_num_ctx`
  (defaults 8192 / 128K / 32768). Mint sends `num_ctx` on every Ollama
  request so the window is enforced, not guessed.
- **Context-compaction trigger raised 0.4 → 0.75** of the window. Compaction
  fires as soon as a step crosses the threshold and shrinks history before
  the next request, so 0.75 keeps far more verbatim context while still
  staying under the real limit. 0.4 was compacting long tasks much earlier
  than necessary.

---

## ⌨️ Chat Composer: History, Persistent Drafts, Edit & Resend (Web + Desktop)

Three input conveniences the CLI already had now work in the Web and Desktop
apps:

- **Up / Down history** — with the caret on the first/last line and no
  suggestion menu open, ↑/↓ step through your previously sent messages; the
  in-progress draft is stashed and restored on the way back, and a small
  `History n/N` badge shows while browsing.
- **Persistent drafts** — unsent composer text is saved per conversation
  (`localStorage`, debounced) and restored when you switch back or reload,
  cleared on send.
- **Edit & resend** — an edit button on each of your messages drops its text
  back into the composer (caret at end, focused) to tweak and send again as
  a new message. Nothing is deleted.

---

## ⌨️ Rebuilt: the `/` `$` `@` Suggestion Menu (Web + Desktop)

The command menu was reworked into one self-contained component with a
single derived-visibility model (it had grown a tangle of open/reopen
state that trapped keyboard focus in a few cases).

- **Picking a command completes it into the composer and stops** — like
  the CLI's Tab. A separate Enter runs it; ↑/↓ bring the menu back for
  subcommands.
- **Slash commands are grouped by category** (System / Models / Workspace /
  Tools), and each row shows the command's argument template.
- **Empty state** instead of a vanishing menu: "No command matches `/xyz`",
  or the active command's usage once you're past its name.
- Hover no longer fights arrow-key navigation; the header shows a stable
  count.

---

## ⚙️ Settings: Component Restyle Finished

The pills/toggles pass from the settings redesign now covers the last tabs:

- **Automation** — "Browser Engine" select → segmented pills.
- **Multi-Agent** — "Enable Multi-Agent Collaboration" wrapped in a bordered
  toggle card (it was a borderless floating row); the agent-form "Scope"
  select → segmented pills, with the workspace option disabled when no
  workspace is open.
- **General** — the Web-Search (Brave / Google / SearXNG) and Video-Gen
  (Veo) provider cards no longer use a radio dressed as an on/off switch;
  each shows an "Active" badge or a "Set active" button.

Model-picker dropdowns with long or dynamic lists are deliberately left as
native `<select>`.

---

## ⌨️ CLI: Readline-Style Editing + Persistent Prompt History

The interactive prompt's input box only had character-wise Left/Right and
Backspace, and its Up/Down recall started empty on every launch.

- **Editing keys** every shell/REPL provides: Home / Ctrl+A, End / Ctrl+E,
  Ctrl+←/→ and Alt+B/F (move by word), Ctrl+W (delete word), Ctrl+U / Ctrl+K
  (delete to line start / end), and Delete (forward-delete).
- **Prompt history now persists** to `~/.config/mint/prompt-history.json` —
  loaded on start, each submitted line appended (immediate duplicates
  skipped), capped at the last 100.
