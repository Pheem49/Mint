# LINE and WhatsApp Cloud Webhook Forwarding

Mint listens on localhost intentionally. Expose only the required listener through a TLS tunnel and
keep the configured signature secrets private.

## LINE

Mint listens at `http://127.0.0.1:3000/callback`.

```bash
cloudflared tunnel --url http://127.0.0.1:3000
```

Append `/callback` to the HTTPS URL printed by Cloudflare Tunnel and register it as the webhook URL
in LINE Developers Console. Configure these Mint settings:

- `lineChannelAccessToken`
- `lineChannelSecret`
- `enableLineBridge`

Mint validates `x-line-signature` before processing messages.

## WhatsApp Cloud API

Mint listens at `http://127.0.0.1:3001/`.

```bash
cloudflared tunnel --url http://127.0.0.1:3001
```

Register the HTTPS URL in Meta Webhooks. Configure these Mint settings:

- `whatsappCloudAccessToken`
- `whatsappPhoneNumberId`
- `whatsappVerifyToken`
- `whatsappAppSecret`
- `enableWhatsappBridge`

Use the same verify token when Meta validates the subscription. Mint verifies
`x-hub-signature-256` for incoming messages when `whatsappAppSecret` is configured.

## Alternative Tunnel

The equivalent ngrok commands are:

```bash
ngrok http 3000
ngrok http 3001
```

Do not bind Mint directly to `0.0.0.0`; the localhost listener plus TLS tunnel keeps the exposed
surface narrow.
