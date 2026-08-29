# Release Notes - Mint Agent v1.13.1

A reliability-and-polish release: scheduled tasks no longer lose a run when
the app is closed mid-execution, the five management pages (Scheduled Tasks,
Skills, MCP Servers, Plugins, Linked Folders) were rebuilt as one consistent
list-style UI, typing a `~/…` path when linking a folder finally works, and
the web UI's "Browse…" button now opens a real folder picker.

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
