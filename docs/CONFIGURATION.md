# Configuration, safety and privacy

[← Back to Mint](../README.md)

## Configuration

Mint stores its local configuration in the platform config directory:

| Platform | Typical path |
| --- | --- |
| Linux | `~/.config/mint/mint-config.json` |
| macOS | `~/Library/Application Support/mint/mint-config.json` |
| Windows | `%APPDATA%\mint\mint-config.json` |

Create and inspect the configuration:

```bash
npm run cli -- config init
npm run cli -- config path
npm run cli -- config show
npm run cli -- config doctor
```

Configuration covers provider credentials, model preferences, browser context,
voice and TTS, proactive suggestions, headless tasks, updates, workflows, MCP
servers, and optional integrations such as Calendar, Gmail, Notion, Telegram,
Discord, Slack, LINE, WhatsApp, Google Search, and Brave Search.

### Ollama tool calling

Mint drives Ollama through native `/api/chat` tool calling when the model family
is known to support it (Llama 3.1+/4, Qwen2+/3/QwQ, Mistral/Mixtral, Gemma 4,
Granite 3–4, Command-R/A, Hermes 3, gpt-oss, DeepSeek V3, Phi-4-mini, and others
— see `OLLAMA_NATIVE_TOOL_MODEL_PREFIXES` in
`crates/mint-core/src/system/config.rs`). Anything else falls back to a less
reliable prompt-based JSON mode and warns once per run.

Check a specific model with `ollama show <model>` (look for `tools` under
**Capabilities**) or browse [`ollama.com/search?c=tools`](https://ollama.com/search?c=tools).
Force the JSON fallback regardless of model with `"forceJsonPromptMode": true` in
the config.

The optional browser smart-context helper can provide active-tab context from:

```text
http://127.0.0.1:3212/context
```

Chromium automation uses the local debugging endpoint:

```text
http://127.0.0.1:9222/json/list
```

## Webhook Integrations

LINE and WhatsApp webhook listeners bind to localhost by default. Read
[`docs/WEBHOOK_FORWARDING.md`](WEBHOOK_FORWARDING.md) before exposing them
through a TLS tunnel.

## Safety And Privacy

Mint keeps high-risk behavior behind explicit policy checks:

- Shell commands are evaluated before execution, then run inside an OS-level
  sandbox by default (bubblewrap on Linux, Seatbelt on macOS —
  `sandboxMode`). Subagents can additionally be isolated in a per-session
  Docker container (`sandboxBackend: "docker"`).
- Code edits and update installation require approval.
- Sensitive directories such as `.ssh`, `.gnupg`, and Mint's own config
  directory are protected by default.
- Sensitive filenames such as `.env` and private key files are blocked from
  routine workspace access.
- LINE and WhatsApp webhook services listen locally unless you intentionally
  forward them.
- Every messaging bridge (Telegram, Discord, Slack, LINE, WhatsApp, Signal,
  Email) locks itself to a single owner: the first sender it ever hears from
  is claimed as that owner, and every other sender is silently ignored from
  then on. To hand a bridge to a different sender, clear its stored owner id
  (e.g. `mint config set telegramOwnerChatId ""`) before they message it.
- The local API server (`mint api`, `mint web`, `mint gateway start
  --api-port`) is open by default, matching the assumption that it's only
  reached from localhost or your own desktop/web frontend. If you expose the
  port on a VPS, set `apiAuthToken` (`mint config set apiAuthToken
  "<secret>"`) to require every request to carry `Authorization: Bearer
  <token>` — and still prefer an SSH tunnel or Tailscale over opening the
  port publicly regardless. See [Running Mint 24/7 on a VPS](GATEWAY.md#running-mint-247-on-a-vps-headless-gateway).

Review the generated command or edit preview before approving an action.
