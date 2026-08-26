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
  --args "Authorization: Bearer <your n8n MCP API key>"

/mcp allow n8n *
```

Put the key inline in the `Authorization` header as shown. Don't try to use a
shell placeholder like `Authorization:${AUTH_HEADER}` — your shell expands it
when you paste the command, and Mint passes MCP server arguments through
verbatim with no environment expansion, so the only place the key reliably
lands is right here (or in `--env`, if the MCP server itself reads one, like
the SurfSense example below). The key is stored in Mint's config alongside
the server entry — treat that config file like the password it contains.

## 4. Use it

```
/n8n                      # opens n8n in your browser (once it's running)
/n8n send the weekly report workflow
```

`/n8n <task>` only works once step 3 is done — until then it'll show the
companion-services status panel instead of running the task.

## If n8n is already running elsewhere

`/n8n`'s browser-opening shortcut checks a hardcoded `127.0.0.1:5678` — if
you run n8n on a different port, on another host, or as n8n Cloud, running
`/n8n` with no arguments will show the companion-services status panel saying
it's "not running" even though it's up. As long as you point the `mcp-remote`
URL in step 3 at the right host and port and registered the `n8n` MCP server,
`/n8n <task>` still works regardless — only the browser-opening convenience
(`/n8n` with no args) depends on the default address.
