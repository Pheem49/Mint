# Release Notes - Mint Agent v1.13.0

This release is smaller than v1.12.0's feature push and leans toward
polish and cross-surface reliability instead: a proper day/night weather
icon set, offline-safe self-hosted fonts (plus a real font bug fixed along
the way), a blank-page bug fixed on the web dev server, the shared "cli"
conversation now live-syncing across the terminal/desktop/web instead of
staying silent until your next turn, a workspace-scoping attempt that
shipped, broke that same shared conversation, and got reverted the same
day once caught — and the web build's approval flow finally does something
real instead of silently auto-denying every tool call, with OS/browser
notifications for both finished replies and pending approvals.

---

## 🌤️ Weather Icons — Day/Night SVG Set

`WeatherCard` used flat `lucide` icons that didn't distinguish day from
night. Replaced with a detailed day/night SVG set, mapped from WMO weather
codes, so the previously-unused `isDay` flag now actually changes what
renders after dark instead of being dead data.

---

## 🔤 Self-Hosted Fonts (and a Real Chat Font Bug Fixed Along the Way)

The Settings font picker's font families now ship with the app via
`@fontsource` instead of being pulled from `fonts.googleapis.com` at
runtime — desktop and web both render correctly offline instead of
silently falling back to system fonts whenever the CDN is unreachable.

- **Found and fixed a real bug while wiring this up**: picking a Font
  Family in Theme & UI wrote to `document.body`'s inline style, but
  `#root` had its own explicit `font-family` rule grouped alongside
  `body`/`html` — nothing inherited from `body`'s override, so every chat
  message stayed on the hardcoded default no matter what was selected.
  Fixed by dropping `#root` from that rule and switching the chat body's
  own hardcoded font stack to inherit instead.

---

## 🩹 Fixed: Blank Page on Refresh at Deep Chat Routes (Web Dev Server)

Refreshing on a deep route like `/chat/<id>` under `npm run dev:web` left
a blank white page. `index-web.html`'s entry script used a relative src
(`./src-web/main.tsx`); Vite's dev server serves that file verbatim
(unlike a production build, which rewrites it), so on a deep route the
browser resolved the relative path against the current URL instead of the
site root, 404ing the entry script. Every other asset reference in the
file already used a root-absolute path matching this build's `base: '/'`
— this one was missed. Fixed the same way.

---

## 🔄 The "cli" Conversation Now Live-Syncs Across Surfaces

Web/desktop and the interactive `mint` REPL already shared one local
SQLite DB and the same `chat_id="cli"` rows, but nothing surfaced a
message sent from one surface on the others until your next turn there.

- **New lightweight DB-polling background task (`live_sync.rs`)**: queues
  a `[synced] ...` notice into the terminal's existing idle-tick notice
  display — the same path `/bg` job notices and linked-folder save notices
  already use — whenever a new row appears from another surface, without
  requiring the local API server to be running.
- **Closed a pre-existing writer/writer gap** by setting a `busy_timeout`
  on SQLite connections, so two surfaces writing at the same moment no
  longer race.

---

## ⚠️ Found and Reverted: a Workspace-Scoping Attempt That Broke the Shared "cli" Room

A same-day incident worth being honest about rather than quietly dropping
from the changelog: this release also shipped, and then reverted, an
attempt to scope the "cli" conversation by workspace.

- **The idea**: hash the active workspace path into the `cli` chat_id
  (`cli::<hash>`) so opening desktop/web against a different project
  wouldn't mix an unrelated project's history into the one shared
  terminal conversation.
- **What actually happened**: "workspace" in this app is a UI selection
  the user switches often (the sidebar's Workspace picker), not a stable
  per-process value like a terminal's `cwd`. Every switch produced a
  different hash, fragmenting the one conversation meant to stay shared
  across the CLI, desktop, and web into five-plus disconnected buckets
  within hours — desktop showed an empty "cli" room, web appeared to stop
  responding, and real chat history sat orphaned under the old unscoped
  `cli` id. Confirmed by direct inspection of the `chat_sessions` /
  `interaction_memories` tables in the local sqlite DB.
- **The fix**: `scoped_chat_id()` now always returns the chat_id
  unchanged — scoping is disabled, not deleted, so it can be revisited
  later with a genuinely stable identity source if it's ever worth doing
  again. Verified via `cargo check`/`cargo test` across all three crates,
  `npm run typecheck`, a live API server confirming history no longer
  depends on which workspace happens to be selected, and a real browser
  session confirming the "cli" room loads its full history again.

---

## 🔔📋 Notifications for AI Replies and Pending Approvals — and Web Approvals Made Real

Two related gaps closed together: a finished reply or a blocked approval
prompt gave zero signal when the app wasn't focused, and the web build had
no real approval flow at all — `/api/chat-stream`'s agent-mode branch
hardcoded auto-deny, so every tool call the agent tried silently failed
for web users with no way to approve it.

- **Web approvals are now interactive**: a new `PENDING_APPROVALS` map
  (token → one-shot channel, mirroring desktop's existing approval
  mechanism) lets the chat-stream endpoint send an `approval-requested`
  event down the same ndjson stream the reply/progress events already
  use, then block until a new `POST /api/submit-approval` resolves it —
  the same `ApprovalCard` UI desktop already had now actually renders and
  works on web. Tokens are `uuid` v4, not sequential, since this endpoint
  is LAN-reachable by default.
- **OS/browser notifications**: `window.api.notifyAiResponse(preview)`
  fires (via `tauri-plugin-notification` on desktop, the native browser
  `Notification` API on web) whenever a reply finishes or an approval
  becomes pending while the tab or window is hidden or unfocused, and
  clears again on refocus.
- **Verified end-to-end in a real browser session, not mocked**: a real
  `WriteFile` request got denied by workspace policy, the agent fell back
  to `run_shell`, a genuine `RunShell` approval card rendered, approving
  it completed the stream, and the notification fired with the correct
  description while the tab was unfocused.
