# Connecting SurfSense (`/notebook`)

`/notebook` is a shortcut that lets Mint's agent query a self-hosted
[SurfSense](https://github.com/MODSetter/SurfSense) instance — an
open-source NotebookLM alternative. **It is not bundled with Mint** — it's a
separate project you clone and run yourself, wired in through its own MCP
server. If you just cloned Mint and haven't set this up, `/notebook` will
show a status panel saying it can't reach SurfSense; that's expected until
you follow the steps below.

## 1. Clone and run SurfSense

```bash
git clone --depth 1 https://github.com/MODSetter/SurfSense.git
cd SurfSense/docker
cp .env.example .env
# generate a real secret and put it in SECRET_KEY=
openssl rand -base64 32
docker compose up -d
```

This is a heavier stack than n8n (Postgres+pgvector, Redis, SearXNG, Caddy,
the backend/frontend, Celery worker/beat, zero-cache — about nine
containers), so the first `docker compose up -d` can take a while while
images pull. Once it's up, open `http://localhost:3929` (SurfSense's default
port — `/notebook` checks this exact port) and:

1. Create the first account (local auth — separate from your Mint account;
   see the note on shared login below).
2. Add your LLM provider's API key in Settings (or point it at a local
   Ollama server, same as Mint and Mint Search).
3. Create a workspace and generate a SurfSense API key.

## 2. Register the MCP server with Mint

SurfSense ships its own MCP server in `surfsense_mcp/` (Python, run with
[`uv`](https://docs.astral.sh/uv/)):

```bash
mint mcp add surfsense uv \
  --args --directory \
  --args /path/to/your/SurfSense/surfsense_mcp \
  --args run \
  --args mcp_server \
  --env SURFSENSE_API_KEY=<your key>

/mcp allow surfsense *
```

The server **must** be registered under the name `surfsense` — that's the
key `/notebook` checks for. Check `surfsense_mcp/README.md` in your clone
for the exact invocation if the package layout has changed since this doc
was written.

## 3. Use it

```
/notebook                      # opens SurfSense in your browser
/notebook find every note about the Q3 launch
```

`/notebook <task>` only works once step 2 is done — until then it'll show
the companion-services status panel instead of running the task.

## Login is separate from Mint

SurfSense has its own account system (Postgres-backed NextAuth), independent
from Mint's shared identity store (`~/.config/mint/mint-user.sqlite`) used
by Mint agent and Mint Search. There's no single sign-on between them —
SurfSense's user records are tied by foreign key to its own workspace data,
so pointing it at Mint's auth database isn't a safe drop-in swap. Sign into
SurfSense with its own account; the `SURFSENSE_API_KEY` from step 1 is what
actually connects it to Mint, not your login session.

## If SurfSense is already running elsewhere

`/notebook`'s reachability check is hardcoded to `127.0.0.1:3929` — if
you've set `LISTEN_HTTP_PORT` to something else, run it on a different host,
or use SurfSense Cloud, the status panel will say it's "not running" even
though it's up. As long as you register the `surfsense` MCP server per step
2, `/notebook <task>` still works regardless — only the browser-opening
convenience (`/notebook` with no args) depends on the default port.
