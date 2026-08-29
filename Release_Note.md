# Release Notes - Mint Agent v1.13.1

A reliability-and-polish release: scheduled tasks no longer lose a run when
the app is closed mid-execution, the five management pages (Scheduled Tasks,
Skills, MCP Servers, Plugins, Linked Folders) were rebuilt as one consistent
list-style UI, typing a `~/…` path when linking a folder finally works, the
web UI's "Browse…" button now opens a real folder picker, Settings' General
and Plugins tabs were cleaned up (and image generation gained per-provider
model pickers), the half-finished Custom Workflows feature was removed, and
a new `/init` command writes an AGENTS.md for the project.

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
