# Webhook Forwarding

Most of Mint's messaging bridges reach out to their provider (Telegram long-polling,
Discord Gateway, Slack Socket Mode, Signal, Email/IMAP). **LINE and WhatsApp are the
exceptions** — those providers *push* events to a public HTTPS URL you register with them.

Mint's LINE and WhatsApp listeners bind to **loopback (`127.0.0.1`) by default**, so the
raw HTTP listener is never directly exposed to the internet. To connect them you put a
TLS-terminating tunnel or reverse proxy in front of the local port and register the
tunnel's HTTPS URL with the provider. This document covers that step.

> The listeners speak plain HTTP and do no TLS themselves. Whatever sits in front of them
> (Cloudflare Tunnel, ngrok, nginx, Caddy, …) is your responsibility. LINE and WhatsApp
> both *require* an HTTPS callback URL — an `http://` URL will be rejected in their consoles.

---

## Ports and config keys

| Bridge | Default bind | Purpose | Enable + credential keys | Bind override |
| --- | --- | --- | --- | --- |
| **LINE** | `127.0.0.1:3000` | Receives `POST` webhook events | `enableLineBridge`, `lineChannelAccessToken`, `lineChannelSecret` | `lineWebhookHost`, `lineWebhookPort` |
| **WhatsApp Cloud** | `127.0.0.1:3001` | `GET` verify handshake + `POST` deliveries | `enableWhatsappBridge`, `whatsappCloudAccessToken`, `whatsappPhoneNumberId`, `whatsappVerifyToken`, `whatsappAppSecret` | `whatsappWebhookHost`, `whatsappWebhookPort` |

Set any key with `mint config set <key> <value>` (or through Settings on desktop/web).
The bridges are started by `start_channels()` — running Mint via `mint`, `mint web`,
or `mint gateway start` all bring them up.

By convention the registered paths are:

- LINE: `https://<your-tunnel>/callback`
- WhatsApp: `https://<your-tunnel>/` (root)

The LINE listener only checks the HTTP method, not the path, so any path under the tunnel
works — but pick one and keep it consistent with what you enter in the provider console.

**Keep the default `127.0.0.1` bind.** Only change `*WebhookHost` to `0.0.0.0` if the
tunnel agent runs on a different host than Mint and you have another network control (a
private network, firewall rules) in front of it. A tunnel like Cloudflare Tunnel or ngrok
running on the same machine connects to `127.0.0.1` and needs no bind change.

---

## Signature verification — keep it on

Both listeners verify the provider's request signature when the corresponding secret is
configured, and this is the only thing stopping a third party who discovers your tunnel
URL from injecting messages.

- **LINE** — set `lineChannelSecret`. Each `POST` is checked with
  HMAC-SHA256 (base64) over the raw body against the `X-Line-Signature` header; a mismatch
  gets `401` and the event is dropped. If `lineChannelSecret` is empty, **all** requests
  are accepted — don't run it that way on a public tunnel.
- **WhatsApp** — set `whatsappAppSecret`. Each `POST` is checked with
  HMAC-SHA256 (hex) over the raw body against `X-Hub-Signature-256` (`sha256=` prefix
  stripped); a mismatch gets `401`. The `GET` verification handshake separately requires
  `hub.verify_token` to equal your configured `whatsappVerifyToken` (which must also match
  what you type into Meta's subscription form), otherwise it returns `403`.

Owner locking applies on top of signature checks: the first sender a bridge ever hears
from is claimed as its owner and every other sender is ignored. To hand a bridge to a
different person, clear the stored owner id first (e.g.
`mint config set whatsappOwnerPhone ""`).

---

## Cloudflare Tunnel

No account needed for a throwaway URL (`try.cloudflare.com`); use a named tunnel for a
stable hostname.

### LINE

```bash
cloudflared tunnel --url http://127.0.0.1:3000
```

`cloudflared` prints a `https://<random>.trycloudflare.com` URL. In the
[LINE Developers Console](https://developers.line.biz/) → your channel → **Messaging API**:

1. Set **Webhook URL** to `https://<random>.trycloudflare.com/callback`.
2. Click **Verify** — Mint answers `200 OK`.
3. Turn **Use webhook** on. Disable "Auto-reply messages" / "Greeting messages" if you
   don't want LINE's canned replies alongside Mint's.

Then in Mint:

```bash
mint config set lineChannelAccessToken "<long-lived channel access token>"
mint config set lineChannelSecret "<channel secret>"
mint config set enableLineBridge true
```

### WhatsApp Cloud

```bash
cloudflared tunnel --url http://127.0.0.1:3001
```

In the [Meta App Dashboard](https://developers.facebook.com/apps/) → **WhatsApp** →
**Configuration** → **Webhook**:

1. **Callback URL**: `https://<random>.trycloudflare.com/`
2. **Verify token**: any string you choose — it must match `whatsappVerifyToken` below.
3. Click **Verify and save** — Mint echoes `hub.challenge` back.
4. Subscribe to the **messages** field.

Then in Mint:

```bash
mint config set whatsappCloudAccessToken "<access token>"
mint config set whatsappPhoneNumberId "<phone number id>"
mint config set whatsappVerifyToken "<the same verify token>"
mint config set whatsappAppSecret "<app secret from App settings → Basic>"
mint config set enableWhatsappBridge true
```

---

## ngrok

```bash
ngrok http 3000     # LINE
ngrok http 3001     # WhatsApp (second terminal / second agent)
```

Use the `https://<subdomain>.ngrok-free.app` forwarding URL from ngrok's output in place
of the Cloudflare hostname above (LINE → append `/callback`, WhatsApp → root). On the free
plan the hostname changes every restart, so you'll re-enter the webhook URL in the
provider console each time; a reserved domain (paid) or Cloudflare named tunnel avoids
that.

---

## nginx / Caddy (own domain + cert)

If you already terminate TLS on the box, just proxy the two paths to loopback. Caddy:

```caddyfile
mint.example.com {
    handle /callback* {
        reverse_proxy 127.0.0.1:3000   # LINE
    }
    handle {
        reverse_proxy 127.0.0.1:3001   # WhatsApp
    }
}
```

nginx equivalent:

```nginx
location /callback { proxy_pass http://127.0.0.1:3000; }
location /         { proxy_pass http://127.0.0.1:3001; }
```

Neither provider sends large bodies, so no special `client_max_body_size` /
buffering tuning is needed.

---

## Running on a VPS

`mint gateway start` runs the same `start_channels()` path headless, so LINE and WhatsApp
work there identically. See [Running Mint 24/7 on a VPS](../README.md#running-mint-247-on-a-vps-headless-gateway).
Keep the tunnel/proxy in front of `3000`/`3001`; do **not** publish those ports directly,
and keep `lineChannelSecret` / `whatsappAppSecret` set so unsigned requests are rejected.

The separate local API server (`--api-port`) is a different concern — gate that with
`apiAuthToken`, an SSH tunnel, or Tailscale as described in the README's Safety section.

---

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| Provider "Verify" fails immediately | Tunnel not running, wrong port, or Mint not started (bridge only binds when its `enable*` key is `true` and credentials are set) |
| LINE verify OK, no messages arrive | "Use webhook" toggle still off, or another sender claimed owner first — check `mint config` for the owner key |
| WhatsApp `GET` returns 403 | `hub.verify_token` ≠ `whatsappVerifyToken` |
| Requests reach Mint but get `401` | Signature secret set in Mint doesn't match the provider's (`lineChannelSecret` / `whatsappAppSecret`), or a proxy is altering the raw body |
| Works locally, not through tunnel | Proxy rewriting/trimming the body; disable body buffering/transformation so the bytes the signature was computed over arrive intact |

Check bridge state any time with `GET /api/gateway/health` (when the API server is
running) — it reports each bridge's enabled flag and last success/error.
