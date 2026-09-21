---
name: tool-existence-audit
description: Determine whether a CLI tool, shell alias, or gateway command is actually installed on the machine (vs. only referenced by stale docs/skills), and decide whether to deprecate or revive it. Trigger when the user asks 'is X still installed?', 'do I still use X?', 'did I remove Y?', or a documented command fails.
revisions: 1
---

# Tool Existence Audit

Answer "do I still use / still have X?" with real evidence from the machine, not memory. The usual finding is **the command is gone but documentation (a skill, README, or config) still pretends it works** — so an agent that trusts the doc will fail on first use. Separate *the real command* from *the documentation that mentions it* before answering.

## Why this is not a one-liner

`command -v <name>` returning empty is **not** proof the tool is gone, because non-interactive shells do not source aliases. A tool defined only as a `alias claude-9arm='...'` is invisible to `command -v` yet fully usable in an interactive shell. You must check the rc files directly. And the reverse trap: a skill/doc mentioning the command is not evidence the command exists.

## Workflow

### 1. Check the command on PATH

```bash
command -v <name>        # emptiness = not on PATH (but see step 2 for aliases)
<name> --version         # only if found
```

### 2. Check for an alias (the #1 false negative)

Non-interactive `command -v` does not see shell aliases, so grep the rc files directly:

```bash
grep -nE 'alias|<name>' ~/.bashrc ~/.bash_aliases ~/.profile \
    ~/.zshrc ~/.zprofile ~/.config/fish/config.fish 2>/dev/null
```

Empty output = no alias. A hit = the command is a shell alias (still usable interactively, invisible to scripts).

### 3. Check config files and directories

```bash
ls -d ~/.<name>* ~/.config/<name> 2>/dev/null
find ~ -maxdepth 3 -iname "*<name>*" 2>/dev/null
```

### 4. Check package managers

```bash
npm ls -g --depth=0        # Node global packages
pipx list 2>/dev/null; pip list 2>/dev/null   # Python
brew list 2>/dev/null      # macOS
```

### 5. Check env vars (gateways / model routing)

Model-gateway aliases usually bake in env vars. Look for them:

```bash
env | grep -iE '<name>|anthropic|model|base_url'
```

Missing `ANTHROPIC_BASE_URL` / `ANTHROPIC_MODEL` strongly indicates the gateway alias was never configured (or was removed).

### 6. Check the host tool's settings

```bash
grep -nE '<name>|model|BASE_URL' ~/.claude/settings.json 2>/dev/null
```

Out-of-workspace files may not be readable by the file tool but are still greppable through the shell — grep via Bash rather than giving up.

### 7. Search the repo / docs for stale references

Grep the workspace (or `search_code`) for the name. Classify every hit:

- Hits only in a **skill / README / doc** and nowhere in real code → **documentation ghost**. The tool does not exist; the doc is stale.
- Hits in actual config / source → the tool may be wired up for real.

### 8. Check the origin

If the doc references a home path or username from **another person's machine** (e.g. `/Users/tpatinya/...` on a `/home/pheem49` box), the skill was copied/imported from a community set, not set up locally — reinforcing that the command never existed here.

## Output shape

1. **Verdict line** — one of: *still installed and working* / *gone (no trace on this machine)* / *docs-only ghost (documentation remains, command does not)*.
2. **Evidence table** — one row per check: what you checked, the command, the result. Label each fact with its source.
3. **"The docs still lie" callout** — name the files that still reference a nonexistent command and warn that invoking them will fail.
4. **Decision options** for the user — typically: (a) deprecate the doc (keep history, add a clear "command no longer exists" note), (b) delete the doc entirely, (c) acknowledge only / do nothing. Recommend the honest, low-effort one; deleting is rarely necessary if the tool died naturally.
5. **Answer in the user's language** (Thai triggers like "ผมไม่ได้ใช้แล้วนะ หรือยังมีอยู่" → reply in Thai).

## Gotchas

- **Aliases are invisible to non-interactive `command -v`.** Always grep the rc files; emptiness there plus a clean `command -v` is the only real proof of removal. State this limitation explicitly in the report.
- **A doc mentioning a command proves nothing.** Separate "real command" from "documentation about the command" — they can diverge for months.
- **A stale skill is worse than no skill.** It shows up in every session's skill list and will make an agent burn time trying a nonexistent command. Flag it, don't just note it.
- **Don't trust one signal.** Run the full ladder (PATH → alias → config → packages → env → host settings → repo grep); a single empty `command -v` is not conclusive on its own.
- **Config outside the workspace** may be unreadable via the file tool — read it through the shell instead.
- **Don't rush to delete.** If the user simply stopped using it, the tooling is already inert; the only actionable cleanup is warning about or deprecating the stale docs.
