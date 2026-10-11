# Mint Mods

Mint Mods are single-file TypeScript or JavaScript extensions that add panels,
status bands, commands and tool guards. Desktop/Web use React; the full-screen
CLI renders the same UI definitions in Ratatui. No Node.js installation is
required to compile or execute mods.

## Create from a prompt

Select a workspace, then ask Mint, for example:

> Create a mod showing this turn's token usage above the composer.

Mint uses `mod_help`, `mod_list`, `mod_validate` and `mod_publish` to prepare and
preview the source. Basic mods open in the current chat after successful
validation. Workspace reads and requests to existing tools require a separate
permission decision. Drafts are retained beside Mint's configuration under
`mods/workspaces/<workspace-hash>/drafts/`; validation errors include their path.
A failed draft never replaces the active revision.

Source is stored in `<workspace>/mint-mods/<id>/` with `manifest.json`, `main.ts`
(or `main.js`) and `README.md`. Commit these files to share a mod. Merely cloning
a repository or importing a package does not enable its mods.

Compiled source, immutable revisions, activation decisions and per-chat state
are kept beside Mint's configuration under `mods/`, outside the workspace.
Activation and state use the canonical workspace identity. A global activation
installs a personal copy; changes in the original workspace do not silently
update that copy. Source changes keep the old active revision until validated
and enabled again, or successfully reloaded in an opted-in session. Special permission decisions belong to the exact source and
manifest hash.

## Manage

Type `/mods` in interactive chat to open the mod manager on Full-Screen TUI,
Desktop and Web. It shows total and active mod counts. Select a mod with the
arrow keys and Enter in the TUI, or select the displayed choices on Desktop/Web,
then choose **Enable for this chat** or **Disable for this chat**. Mods with
permissions require a separate approval before enabling. After a successful
change, the menu closes, focus returns to the composer, and chat displays
`[Mods] Enabled/Disabled <id> for this chat.` Reopen `/mods` to see updated counts,
or use `/mods list` for a text listing.

Use **Mods** above the composer on Desktop/Web to inspect source, diagnostics,
permissions and revisions; enable for the current chat, workspace or globally;
export/import `.mintmod` packages; or restore a source revision.

```sh
mint mods list
mint mods source token-meter
mint mods validate token-meter
mint mods test task-checklist
mint mods --chat <chat-id> usage
mint mods enable token-meter
mint mods enable task-checklist workspace
mint mods enable custom-reader chat --grant-permissions
mint mods disable token-meter
mint mods logs token-meter
mint mods revisions token-meter
mint mods rollback token-meter <revision-hash>
mint mods export token-meter token-meter.mintmod
mint mods import token-meter.mintmod
mint mods import token-meter.mintmod --replace
mint mods --chat <chat-id> list
```

In interactive chat use `/mods hot-reload on|off` to watch active basic mods for this session; Desktop/Web offer a Hot reload toggle. Invalid changes keep the working revision and report reload diagnostics. Privileged changes still require review. Closing the session clears the opt-in and temporary revision override.

Use `mint mods test <id>` or `/mods test <id>` for runtime tests; Desktop/Web have Run tests with per-case results. Tests never activate mods or perform real filesystem/tool operations. See [TESTING.md](TESTING.md).

In interactive chat use `/mods export <id> [file]`,
`/mods import <file> [--replace]`, and `/mods:<id>:<command> [value]`.
Quote paths containing spaces. F7 opens the
terminal panels; Tab/Shift+Tab select a control, Enter activates it, and Escape
returns to chat. Input controls accept text before Enter. In a narrow terminal
the panel overlays the transcript while retaining the composer. `/mods run-tools`
reviews queued tool requests; the standalone equivalent is `mint mods run-tools`.

Restore changes source; review and enable the restored revision separately.
Permission approval is never included in exported packages.

## Examples and SDK

Copy a folder from `examples/mods/` into your workspace's `mint-mods/` directory:

- `token-meter`: displays actual usage, with explicit unavailable state.
- `task-checklist`: input, button and table backed by per-chat state.
- `tool-guard`: blocks recursive `rm` commands as an example guard; it is not a
  comprehensive shell security policy.

Read [AUTHORING.md](AUTHORING.md) for the entrypoint, events and workflow, and
[mint-mods.d.ts](mint-mods.d.ts) for the TypeScript interface.

## Runtime and limits

Each instance runs in its own process with a restricted QuickJS runtime. The
SDK grants access through Mint; it does not provide Node.js, direct shell,
filesystem, network or browser DOM access. The host checks capabilities even
when a mod bypasses its JavaScript SDK wrappers. This is process separation and
capability restriction, not an OS sandbox for native-runtime vulnerabilities.

The runtime caps memory at 64 MiB and callback execution at 250 ms, with a
three-second IPC deadline. UI trees are limited to 256 nodes, 12 nesting levels
and 64 KiB. State/output is limited to 256 KiB; source to 512 KiB. Packages are
limited to 1 MiB and contain only manifest, entry and optional README. External
imports, additional package entries and symlinked sources are rejected.

Tool requests pass through Mint's existing tools, permissions, disabled-tool
checks and plan-mode restrictions. A mod never grants itself permissions.
Pre-tool hooks may observe or block; they cannot request tools or rewrite tool
inputs. A failed pre-tool hook blocks subsequent calls until the mod is fixed
or disabled. Other observer errors stop the affected instance.

## Packaging

The CLI package builds `mint` and `mint-mod-host`. The desktop executable also
contains an internal host entrypoint, so bundled Desktop installations can
start isolated workers without an external Node executable or sidecar path.
The Web server uses the same host launcher on the server machine.

Context indicators can read `mint.session.usage()` immediately and consume `usage_update`, `context_compacted`, and `turn_end` with nullable latest context tokens, Mint's planning window, percentage and signed completed-turn delta. UI nodes support five named colors and row stacks in CLI, Desktop and Web. See [authoring contract](AUTHORING.md) and [SDK types](mint-mods.d.ts). Anthropic normalization follows [the provider's prompt-cache usage fields](https://platform.claude.com/docs/en/build-with-claude/prompt-caching).

For Claude-style context readings, use `usage.context` (latest input). `usage.contextIncludingOutput` and the existing flat fields include output as well. `mint.ui.viewport()` and `ui_resize` support responsive bands; graphical text dimensions are estimates while pixel dimensions are measured.
