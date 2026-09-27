---
name: machine-session-recon
description: Inspect the current Linux desktop session and answer 'what am I using right now?' — read the session via systemd-logind (not utmp), detect X11 vs Wayland and the login-vs-desktop stack, enumerate which other sessions are switchable to, then group running processes into the apps actually in use. Trigger when the user asks 'ช่วยดู session ของเครื่อง / ฉันใช้อะไรอยู่', 'am I on x11 or wayland?', 'x11 หรือ wayland', 'what session am I on', or 'check what's using my machine'.
revisions: 2
---

# Machine Session Recon

Answer "what session am I on, and what am I actually using?" with real evidence from the running system. This is **read-only** reconnaissance: it inspects who's logged in, which display stack is live, and which processes make up the apps the user has open. Report it as a scannable summary, not a raw dump.

## Core gotcha: `who` is empty on systemd-logind systems

On a modern systemd desktop, **`who -a` shows only `system boot` and `run-level N` — no user line.** This is *normal*: systemd-logind does not write utmp, so `who`/`w` have nothing to read. Do **not** conclude "no one is logged in." Get the session from **`loginctl`** instead:

```bash
loginctl list-sessions                 # session IDs, seat, user, state
loginctl show-session <id>             # Type, Class, Active, IdleHint, Remote, TTY, Display
loginctl user-status <user>            # user's sessions + linger + processes
```

State this explicitly in the report — users (and agents) who only ran `who` get confused by the empty output.

## Command ladder (all read-only)

```bash
# 1. The session itself
loginctl list-sessions
loginctl show-session <id>              # or: loginctl session-status <id>
loginctl user-status "$USER"

# 2. Type/class: x11 vs wayland, which display
loginctl show-session <id> -p Type -p Class -p Active -p Remote -p TTY
env | grep -E 'XDG_SESSION_TYPE|DISPLAY|WAYLAND_DISPLAY|XDG_CURRENT_DESKTOP'

# 3. Who/what logged us in, and how long
who -a                                 # usually only boot + run-level (see gotcha)
uptime                                 # boot time + uptime

# 4. What's actually running (top consumers)
ps -eo pid,pcpu,pmem,etime,comm --sort=-%mem | head -20
ps -eo pid,pcpu,pmem,etime,args --sort=-%mem | head -20   # full command line

# 5. Drill into a suspected app
pgrep -af '<name>'                     # shows PID + full args

# 6. Memory/swap headroom (optional, if 'is my machine loaded?' is the real question)
free -h; swapon --show
```

Prefer `ps -o comm` for the short name and `ps -o args=` when you need to see *which build* is running (e.g. `target/debug/...` vs `target/release/...`).

## Identify the display stack

Read the environment, not guesses. For a plain "am I on X11 or Wayland?" question, this is the whole answer — report it first, then the detail:

- **`XDG_SESSION_TYPE=x11`** and a non-empty `DISPLAY=:N` with empty `WAYLAND_DISPLAY` → X11 session.
- **`XDG_SESSION_TYPE=wayland`** and non-empty `WAYLAND_DISPLAY` → Wayland session.
- `XDG_CURRENT_DESKTOP` / `DESKTOP_SESSION` names the desktop (GNOME, KDE, COSMIC, …).
- Cross-check with `loginctl show-session <id> -p Type` — `Type=x11` is the authoritative field; the env vars should agree.
- Confirm with running processes: `Xorg` + `gnome-shell` (and **no** `Xwayland`/`cosmic-comp`) = X11; `Xwayland` present = a Wayland session hosting X clients.

**Login manager ≠ desktop.** It's common to log in through one greeter (e.g. `cosmic-greeter`) and land in a different desktop (e.g. GNOME on X11). Check the session leader process / `loginctl session-status` for the login service, and report both — "logged in via X, desktop is Y" is a real, useful distinction (e.g. Pop!_OS 24.04 ships COSMIC + GNOME).

**X11-vs-Wayland on the *same* desktop is a session-entry choice, not a bug.** A user can be on "GNOME on Xorg" (`/usr/share/xsessions/gnome-xorg.desktop`) without ever meaning to be. When they ask "why am I on X11?", the answer is usually "the login screen picked the Xorg entry."

## Enumerate what's switchable (the natural follow-up)

When the user's question is *"am I on X11 or Wayland?"*, the unspoken next question is almost always *"…can I switch?"* Answer both in one pass by listing the installed session entries:

```bash
ls /usr/share/xsessions/            # X11 session entries (.desktop)
ls /usr/share/wayland-sessions/     # Wayland session entries (.desktop)
```

Report which X11 and Wayland entries both exist (e.g. `gnome.desktop` / `gnome-xorg.desktop` under xsessions, `gnome.desktop` / `cosmic.desktop` under wayland-sessions), so the user can see the exact entry to pick.

Switch procedure (give it to the user, don't do it):

1. **Log out** — not just lock the screen. Switching sessions only happens at login.
2. At the login screen, click the ⚙️ / session-selector (usually near the password field or a corner) and pick the target entry — e.g. **"GNOME"** (Wayland) rather than **"GNOME on Xorg"**, or **COSMIC** for a Wayland compositor.
3. Log in, then re-check with `echo $XDG_SESSION_TYPE` — it should now read `wayland`.

**NVIDIA caveat worth stating.** Many users deliberately stay on X11 on NVIDIA machines because screen sharing (OBS/Discord/Zoom), global hotkeys, and some remote-desktop tools behave more reliably there. If the user has none of those dependencies, Wayland generally gives smoother HiDPI and touchpad gestures. Mention the trade-off rather than pushing a switch.

## Group processes into "apps the user is using"

`ps` rows are processes, not apps. Collapse them by recognizable process families so the report reads as "here's what you have open":

| Looks like | It's |
|---|---|
| `<name>-ide` + `language_server` + `node_repl` + `chrome-devtools` | An Electron/VS Code-family **IDE** (e.g. Antigravity) |
| `firefox-bin` + many `Isolated Web Co` / `Web Content` / `WebExtensions` / `Privileged Cont` | **Firefox** (one process per tab/content) |
| `codex` / `<cli>` + `crashpad_handler` / `bwrap` sandbox | A **CLI tool** running (note if it's a `target/debug/...` build) |
| `gnome-shell`, `gnome-session-binary`, `gsd-*`, `gjs`, `gnome-keyring-daemon`, `evolution-*`, `goa-daemon` | The **GNOME desktop stack** (session-lived) |
| `pipewire`, `wireplumber`, `dbus-broker`, `Xorg`, `xinit` | **Audio/bus/display plumbing** (session-lived) |

Report age from the `etime` column (e.g. Firefox ~1h51m, IDE ~23m) — it tells the user which app they opened recently vs which has been running all session.

Flag things you can't confirm instead of guessing: e.g. an ambiguous helper like `gk` (2 processes) — say "can't confirm which app owns this; I can dig further" rather than asserting an owner.

## What to call out

- **Session type**: X11 vs Wayland, `DISPLAY`/`WAYLAND_DISPLAY`, and where they'd switch (log back in via the Wayland session entry if they want Wayland).
- **Login manager vs desktop** when they differ (cosmic-greeter + GNOME).
- **Debug vs release builds** of a running CLI: `target/debug/<bin>` burning CPU is worth switching to `target/release/<bin>` (or a built release) to reduce load — a concrete, actionable note on a RAM/CPU-constrained machine.
- **Concurrent memory pressure**: if 3–4 big apps (browser + IDE + CLI + desktop shell) are each holding several % of RAM, say so — it matches the "run heavy commands one at a time" constraint many dev boxes have. Cross-reference `hardware-upgrade-check` if the user pivots to "can I add RAM?" and `cargo-clean-decision` if it becomes a disk question.
- **Linger**: if `loginctl user-status` shows `Linger=yes`, note that user services (e.g. a `mint gateway` systemd --user unit) keep running after logout.

## Output shape

1. **Verdict line up front** — for a pure X11/Wayland question, lead with the one word (**X11** or **Wayland**), then "one session, GNOME on X11, N main apps running" for the fuller recon.
2. **Session table** — session ID/scope, user (UID), seat/TTY, **Type/Class**, state + `IdleHint`, Remote, desktop, login service, Linger, boot time + uptime, run-level.
3. **The `who -a` note** — explain why it shows no user (systemd-logind, no utmp), so the empty output doesn't look like a failure.
4. **Top processes table** — PID, %CPU, %MEM, `etime` (age), process name — sorted by mem.
5. **"Apps you're using" table** — collapsed process families → app → evidence (process names) → session age. Include the desktop stack and audio/plumbing as background rows.
6. **Own observations** — X11 vs Wayland, login-vs-desktop mismatch, debug-build CPU cost, concurrent RAM use, Linger, anything unconfirmed (say so).
7. **State clearly that nothing was changed** — all reads (`loginctl`, `who`, `uptime`, `ps`, `pgrep`), no files or settings touched.
8. **Offer A/B/C next steps** in the user's language — e.g. (A) full `ps -o args=` + identify the ambiguous helper, (B) real RAM/swap headroom (`free -h`, `swapon --show`), (C) inspect the app's own config/state dir. When the topic is the display stack, offer: which sessions are installed / how to make Wayland the default (config edit — ask approval first) / list X11 clients (`xlsclients`).

## Gotchas

- **`who -a` empty ≠ nobody logged in.** systemd-logind doesn't write utmp; use `loginctl`. This is the #1 confusion in this task.
- **`loginctl` is the source of truth** for session Type/Class/Active/Remote/IdleHint — `who`/`w`/`last` can all be stale or empty on logind systems. `loginctl show-session <id> -p Type` is the authoritative x11-vs-wayland field.
- **IdleHint=no means they're actively using it** — worth stating, since "active session" and "actively used" aren't the same thing.
- **Type: x11 with a Wayland display empty** is a deliberate session choice, not a bug — if the user expected Wayland, they picked (or the greeter defaulted to) the X11 session entry. Point them at the session file, don't call it broken.
- **A greeter that doesn't match the desktop is normal.** Don't report it as broken; report it as "logged in via X, running Y."
- **X11 and Wayland entries of the same desktop are easily confused at the login screen** — "GNOME" vs "GNOME on Xorg". Name the exact `.desktop` file the user is on when relevant.
- **Switching sessions requires a full logout**, not a lock-screen. Say so, or the user will lock and wonder why nothing changed.
- **Don't push the Wayland switch.** On NVIDIA, X11 can be the deliberate choice for screen-share / hotkey / remote-desktop reliability. State the trade-off and let the user decide.
- **Processes are not apps.** Collapse families (`firefox-bin` + `Isolated Web Co*` = one Firefox) before reporting, or the user reads 30 rows of noise.
- **Don't guess app ownership.** If a helper is ambiguous (`gk`), say it's unconfirmed and offer to dig; don't invent an owner.
- **Read-only means read-only.** No settings changes, no killing processes, no config edits — inspection only, and say so.
- **Answer in the user's language.** Thai trigger ("ช่วยดู session ของเครื่อง", "ฉันใช้อะไรอยู่", "x11 หรือ wayland") → reply in Thai.
- **Tool-dedup caveat**: some CLI shell tools dedupe "duplicate" commands and may skip a near-identical follow-up — vary the command or add distinguishing flags rather than re-issuing the same one, or expect a "Skipped duplicate shell command" response.