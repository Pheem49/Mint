# Headless gateway and VPS deployment

[← Back to Mint](../README.md)

## Running Mint 24/7 on a VPS (Headless Gateway)

By default, messaging bridges and cron only run while something's actually
attached — the interactive terminal, the desktop app, or `mint web`/`mint
api`. **Gateway mode** is a real headless mode built for unattended
deployment: no TUI, no desktop window, just the bridges and the cron
scheduler running in the background, installable as a systemd service that
survives reboots.

### How it works

- `mint gateway start` calls the exact same `start_channels()`/
  `start_cron_scheduler()` the interactive app uses — it just never launches
  the terminal UI, so it needs no TTY and can run under systemd with no
  login session attached.
- Every bridge loop auto-restarts on error *or* panic (5s backoff), so a bad
  payload from one platform can't silently and permanently kill that bridge.
- All bridges (Telegram, Discord, Slack, LINE, WhatsApp, Signal, Email) share
  one continuous memory thread with the terminal CLI, not a siloed one per
  platform.
- `GET /api/gateway/health` reports each bridge's enabled state, last
  success, last error, and consecutive-failure count as JSON — check it
  remotely instead of SSHing in to read `journalctl`.

### Quick start on a fresh VPS

```bash
# 1. Install Mint (Linux, needs Node/npm + Rust — the installer offers to set both up)
curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash

# 2. Configure a provider + the bridge(s) you want (Telegram, Signal, Email, ...)
mint onboard

# 3. Test in the foreground first — fix any config errors here before installing as a service
mint gateway start
# Ctrl+C once you see your bridge(s) come up "Active" and a test message gets a reply

# 4. Install as a systemd service and start it now
mint gateway install --now --api-port 3000 --memory-max 512M

# 5. Per-user units (the default) only run while you're logged in —
#    this keeps it running after you log out / reboot with no session at all
sudo loginctl enable-linger "$(whoami)"
```

### Gateway commands

| Command | Purpose |
| --- | --- |
| `mint gateway start` | Run bridges + cron in the foreground, headless (no TUI) |
| `mint gateway start --api-port <N>` | Same, plus the local API/WebUI on port `<N>` |
| `mint gateway install` | Write + enable a per-user systemd unit (`~/.config/systemd/user/`, no root) |
| `mint gateway install --system` | Same, but system-wide (`/etc/systemd/system/`, needs `sudo`) |
| `mint gateway install --now` | Also start the service immediately after installing it |
| `mint gateway install --memory-max <size>` | Cap the service's memory (systemd size syntax, e.g. `512M`, `1G`) — unset by default |

Once installed, `mint gateway install` has already written and enabled the
unit — day-to-day you drive it with plain `systemctl` (drop `--user` and use
`sudo` for a `--system` unit):

| Task | Command |
| --- | --- |
| Start it now | `systemctl --user start mint.service` |
| Stop it now | `systemctl --user stop mint.service` |
| Restart it | `systemctl --user restart mint.service` |
| Is it running? | `systemctl --user status mint.service` |
| Follow logs | `journalctl --user -u mint.service -f` |
| Start on boot | `systemctl --user enable mint.service` (`install` already did this) |
| Don't start on boot | `systemctl --user disable mint.service` |
| Uninstall | `systemctl --user disable --now mint.service`, then delete `~/.config/systemd/user/mint.service` |

`start`/`stop` control it right now; `enable`/`disable` control whether it
comes up on boot — the two are independent. Per-user units also need
`sudo loginctl enable-linger "$(whoami)"` once to keep running with no login
session (see the VPS quick start above).

### Checking on it remotely

Don't expose the API/WebUI port to the public internet — reach it over an
SSH tunnel or [Tailscale](https://tailscale.com/) instead:

```bash
ssh -L 3000:localhost:3000 you@your-vps
curl http://localhost:3000/api/gateway/health
```

For an extra layer beyond the tunnel itself, set a shared secret so every API
request needs it:

```bash
mint config set apiAuthToken "$(openssl rand -hex 32)"
```

Once set, every request (except the browser's CORS preflight) needs
`Authorization: Bearer <token>` or gets `401 Unauthorized`. Leave it unset to
keep the previous open-on-localhost behavior (desktop app / `mint web` don't
need to change anything).

### New bridges built for this: Signal and Email

- **Signal** has no official bot API, so Mint talks to a self-hosted
  [`signal-cli-rest-api`](https://github.com/bbernhard/signal-cli-rest-api)
  instance instead (you link the number yourself first). Config:
  `enableSignalBridge`, `signalApiUrl`, `signalNumber`.
- **Email** reuses the same Gmail OAuth connection as the `gmail` plugin —
  set `gmailClientId`/`gmailClientSecret`, run `mint gmail auth` once to get
  a refresh token, then enable it. Both are offered directly in `mint
  onboard` under "Messaging Bridges".

> [!NOTE]
> LINE and WhatsApp are webhook-based (the provider pushes to you), which
> means they need a real public HTTPS URL — a reverse proxy (Caddy/nginx) +
> TLS cert in front of the VPS. Telegram, Discord, Slack, Signal, and Email
> all connect *outbound* instead, so they need nothing public at all. See
> [`docs/WEBHOOK_FORWARDING.md`](WEBHOOK_FORWARDING.md) before exposing
> a webhook listener.
