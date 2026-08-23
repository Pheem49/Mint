# Connecting n8n (`/n8n`)

`/n8n` is a shortcut that lets Mint's agent trigger [n8n](https://n8n.io)
workflows. **n8n is not bundled with Mint** — it's a separate open-source
project you run yourself, wired in through an MCP server. If you just cloned
Mint and haven't set this up, `/n8n` will show a status panel saying it
can't reach n8n; that's expected until you follow the steps below.

> **Note:** n8n redesigned its MCP support at some point after v1.88. The
> old "MCP Server Trigger" node (one SSE endpoint per workflow) is gone.
> Newer n8n ships a single **instance-wide MCP HTTP endpoint** instead —
> confirmed here by inspecting a running `docker.n8n.io/n8nio/n8n:latest`
> container (v2.34.6). If your n8n is older, look for an "MCP Server
> Trigger" node instead of the Settings toggle described below.

## 1. Run n8n

n8n has an official Docker image. Anywhere outside the Mint repo (a sibling
folder works well), create a `docker-compose.yml`:

```yaml
services:
  n8n:
    image: docker.n8n.io/n8nio/n8n:latest
    container_name: mint-n8n
    restart: unless-stopped
    ports:
      - "5678:5678"
    environment:
      - N8N_SECURE_COOKIE=false
    volumes:
      - n8n_data:/home/node/.n8n

volumes:
  n8n_data:
```

Then:

```bash
docker compose up -d
```

Open `http://localhost:5678` and create the owner account (first run only).

## 2. Enable MCP and grab an API key

1. In n8n, open **Settings → MCP Server** ("Access your n8n instance
   through MCP clients").
2. Turn it on.
3. Generate/copy the API key shown there. It authenticates every MCP
   request as a Bearer token — treat it like a password.

Workflows become callable as MCP tools automatically once they have a
supported trigger node (Schedule, Webhook, Form, or Chat Trigger for
production use; Manual Trigger too if you enable it) — no separate
"expose to MCP" step per workflow.

## 3. Register it with Mint

Mint's MCP client only speaks stdio, so bridge the instance's streamable-HTTP
MCP endpoint with [`mcp-remote`](https://www.npmjs.com/package/mcp-remote),
passing the API key as a Bearer header:

```bash
mint mcp add n8n npx \
  --args -y \
  --args mcp-remote \
  --args http://localhost:5678/mcp-server/http \
  --args --header \
  --args "Authorization:${AUTH_HEADER}" \
  --env AUTH_HEADER="Bearer <your n8n MCP API key>"

/mcp allow n8n *
```

(`--header` needs `Authorization:${AUTH_HEADER}` with no space around the
colon, and the actual `Bearer <key>` value goes in the `AUTH_HEADER` env var
— that keeps the key out of the process argument list.)

## 4. Use it

```
/n8n                      # opens n8n in your browser (once it's running)
/n8n send the weekly report workflow
```

`/n8n <task>` only works once step 3 is done — until then it'll show the
companion-services status panel instead of running the task.
