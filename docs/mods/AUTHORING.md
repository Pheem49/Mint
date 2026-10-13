# Mint mod authoring

Load this guide with `mod_help` when the user asks to create or change a Mint mod.

Deliver a working mod through the tools. For a self-contained SDK mod, use the reference below, choose reasonable defaults and send the draft source directly in `mod_validate` arguments. A progress note is at most one short sentence; the source belongs in the tool call. Continue to `mod_publish` after successful validation and report its result. Consult additional workspace files only for a concrete dependency or diagnostic.

1. Call `mod_list` to inspect existing source, hashes, permissions and active revisions.
2. Create one module exporting `register(mint)`. Draft the manifest with sdkVersion 1, a lowercase kebab-case id, name, version, entry `main.ts` or `main.js`, and a permissions array.
3. Call `mod_validate` with arguments `{manifest, source, readme}`. Mint retains the draft outside the workspace; validation errors include its path. Preview has no filesystem or tools. Defer those calls to commands or event handlers. Fix diagnostics, with at most three validation attempts; then report any remaining error.
4. Call `mod_publish` with the same arguments. For an existing mod also include `expectedHash` from mod_list. Publishing checks for disk changes, creates new IDs in `.mint-mods/<id>/` (existing legacy IDs are edited in `mint-mods/<id>/`), and opens the validated revision in this chat. Basic capabilities need no extra activation decision; requested permissions are reviewed by the user.
5. If tests are requested, add `tests/*.test.ts` exporting `registerTests(test)` and run `mint mods test <id>` (or `/mods test <id>`). Follow [TESTING.md](TESTING.md). Report the folder, activation state and validation/test results with tool output as evidence.

Use these interfaces:
- `mint.session.usage()` synchronously reads the latest usage snapshot, including before the first event; no special permission or model request.
- `mint.ui.viewport()` reads the current band dimensions and surface. Subscribe to `ui_resize` to redraw on resizing or panel placement changes. CLI columns are exact; Desktop/Web columns/rows are estimates, with measured widthPx/heightPx. Unknown values are null.
- `mint.ui.set('above_composer' | 'side_panel', node)`
- `mint.on(event, handler)` for session_start/end, turn_start/end, pre_tool_use, post_tool_use, usage_update, context_compacted, ui_resize, tool_result.
- `mint.commands.register(name, handler, description?)`; UI button/input `action` is a registered command name. Input values arrive as handler arguments. Slash commands are `/mods:<id>:<name>`.
- `mint.state.get(key)` / `mint.state.set(key, JSON value)`: instance state survives restart and stays separate per chat/workspace.
- `mint.log(message)` for diagnostics.
- `mint.workspace.read(relativePath)` synchronously reads up to 256 KiB with `workspace.read` permission.
- `mint.tools.request(action, input)` returns a request ID with `tools.request` permission. Requests run through Mint's tools and approvals; handle their eventual outcome through `tool_result`, matching `requestId`. Pre-tool hooks cannot request tools.

UI node types:
- text/badge: `{type, text}`
- stack: `{type:'stack', direction?:'row'|'column', children:[nodes]}`
- button: `{type:'button', label, action}`
- input: `{type:'input', label, action, value?}`; Enter submits.
- progress: `{type:'progress', label, value:number, max:number}`
- table: `{type:'table', columns:string[], rows:(string|number|null)[][]}`

All nodes accept `color: 'default' | 'yellow' | 'cyan' | 'blue' | 'magenta' | 'red'`. Colors inherit from stacks; `default` restores normal text. Use a row stack for a one-line band with separately colored labels and plain metrics. Desktop/Web scroll narrow rows horizontally; CLI truncates at terminal width. Arbitrary CSS, HTML and ANSI escapes are not supported.

`usage_update`, `context_compacted` and `turn_end` include:
- `turnId`, `provider`, `model`, `totalTokens` (accumulated API usage this turn, nullable).
- `contextUsedTokens` (latest provider-reported input + output), `contextInputTokens`, `contextOutputTokens`, `contextPercent` (all nullable).
- `contextWindowTokens` (nullable), `contextWindowSource: 'mint_config'`. This window comes from Mint's provider defaults/configuration; it is a planning limit, not discovered model capacity. Never hardcode 200k for every provider.
- `contextStatus: 'measured' | 'unavailable' | 'compacted'`, `compacted` (whether compaction occurred in this turn).
- `lastTurnDelta` (signed token change from the prior completed turn; null on the first turn, missing usage, provider/model/window changes, and until turn_end).

`usage.context` is `{tokens, window, percent, basis:'input', lastTurnDelta}` and measures the input answered over by the latest response. `usage.contextIncludingOutput` has the same shape with `basis:'input_output'`. The original flat `contextUsedTokens`, `contextPercent` and `lastTurnDelta` retain the input + output definition for compatibility. Use `usage.context` consistently for a Claude-style Token Weather forecast, including its delta; check tokens for null. Both readings use Mint's planning window.

Context occupancy is a latest-response snapshot; tool results appended afterwards are reflected in the next provider response. Repeated requests resend history: do not sum their token counts to estimate occupancy. Anthropic input includes uncached, cache-read and cache-created tokens. Successful compaction emits `context_compacted` with cleared occupancy; wait for the next measured response rather than retaining the old value. `turn_end` also includes success/cancelled and the final usage snapshot, including when a mod was published after the latest usage_update. Use it to append exactly one history point per turn; keep the last 12 in mint.state for a weather sparkline. Show unavailable text for null measurements/deltas. Reading current usage at registration/session_start initializes a newly created mod without waiting for another turn. UI changes require mint.ui.set; state.set alone does not redraw. Refer to `mint-mods.d.ts` for types.

`pre_tool_use` receives action, input, chatId, workspace and callId; return `{block:'reason'}` to reject, or return nothing to allow. Other observers cannot change tool input. `usage_update.totalTokens` is null when unavailable. `turn_start` resets a per-turn token meter. Use explicit unavailable text rather than inventing usage.

Hot reload is opt-in for the live session: `/mods hot-reload on|off`, or the Mods panel toggle. Refresh/events check local active source for changes, validate in a candidate worker, keep state and replace only the current chat's instance. A bad candidate reports reloadError and leaves the old worker active. Changes to any mod with privileged old/new revisions require source-hash review and explicit activation. Global personal copies stay independent of local source. Closing/cancelling the session clears the reload opt-in and session revision override; the original activation revision remains the default for reopening. Invalidate old UI action IDs on reload. Publishing through mod_publish already validates and activates edits; it does not require hot reload to be enabled.

Modules run in restricted QuickJS with 64 MiB memory and a 250 ms CPU deadline per callback. Use SDK capabilities instead of OS globals. Source is one file; type imports from `@mint/mods` are supported. The built-in `defineMod` is an identity helper; export `register` directly. UI is declarative and renders in React and Ratatui.

Example:
```ts
export function register(mint: any) {
  mint.ui.set('above_composer', {type:'text', text:'Usage unavailable'});
  mint.on('usage_update', (event: any) => {
    mint.ui.set('above_composer', {
      type:'text', text:event.totalTokens == null ? 'Usage unavailable' : `${event.totalTokens} tokens`
    });
  });
}
```
