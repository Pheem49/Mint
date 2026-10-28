# Mint Auto browser tasks

Mint Browser and the agent share the Rust CDP engine. Agent tasks automatically start an isolated **headless** Chrome/Chromium/Edge runtime when needed; opening Mint Browser yourself still shows a window. Agent runtimes use a temporary profile and dynamically assigned loopback port, and are closed on completion or cancellation. Element references live only for the current run.

Choose **Automation → Agent browser mode → Show window** for visible agent work. `browserExecutable` optionally selects an installed Chromium-based executable. Mint does not download a browser automatically. The legacy `mint auto` command still opens its persistent automation profile. Use headed mode to reuse that running browser, or explicitly configure `browserUseExisting: true`. Non-default/external `browserDebugUrl` endpoints are borrowed: Mint closes its task-created tabs but never shuts down the external browser. Borrowed tabs share cookies; isolated headless runs do not reuse the user's login profile.

Headless login/CAPTCHA stops require user help. Restart in headed mode when interaction is needed; browser state is not silently migrated between modes. Firefox is not supported by this CDP engine. `enableHeadlessTaskQueue` controls scheduled tasks and is unrelated to the browser mode.

## Agent workflow

1. Navigate with `browser_open` and inspect its returned observation.
2. Use an `elementRef` from that observation to click, fill, or select a control. Existing CSS, text, and XPath selectors still work.
3. Inspect the observation returned by the action. References from previous observations expire.
4. Use `browser_wait` to check the expected URL, text, or element visibility. A dispatched click alone is not evidence of task completion.
5. Include observed evidence in `finish.verification`. Set `completionStatus` to `verified`, `unverified`, `blocked`, or `failed`; explain a limited completion honestly. Written claims cannot override failed/running checks.

| Tool | Behavior |
| --- | --- |
| `browser_observe` | URL, title, readable text, interactive controls, and fresh references. Supports `textOffset` and `elementOffset` pagination. |
| `browser_tabs` | `list`, `open`, `select`, or `close`. Selection and closure require `tabId`. Opening selects the new tab. Omitted, empty, and whitespace-only URLs open `about:blank`; supplied HTTP(S) URLs are validated before creating the tab. Popup tabs require explicit selection. |
| `browser_click` | Click a reference or selector after scrolling and checking visibility, enabled state, and obstruction. |
| `browser_type` | Append text to a supported field and verify its value. |
| `browser_fill` | Replace a supported field's value, including an empty value, and verify it. |
| `browser_select` | Select a single native dropdown option by its `value`. |
| `browser_scroll` | Scroll the viewport or a target by `x`/`y` pixels. Defaults to 600 pixels downward. |
| `browser_wait` | Wait for `url`, `text`, `visible`, or `hidden`. URL and text conditions require a nonempty `value`; visibility conditions require a target. Default timeout is 10 seconds, capped at 30 seconds. |
| `browser_read` | Existing plain-text page read. |
| `browser_screenshot` | Capture `mode: "viewport"` (default), `"fullPage"`, or `"element"` with a target. Every requested screenshot becomes a durable chat artifact and model image. Coordinate input requires a fresh viewport screenshot; other capture modes invalidate coordinate evidence. |

Supply exactly one of `elementRef` and `selector` for targeted operations. Supported text fields are textarea and text, search, URL, telephone, password, email, and number inputs. Both edit tools pin the original node and verify it is connected, enabled, editable, and focused after clicking and again after keyboard preparation. `focus_changed` or `stale_reference` stops the operation before insertion/deletion; the tools do not refocus or retry. Password values are excluded from observations and browser typing logs.

Action results distinguish `input_dispatched` from `failed_or_ambiguous` and include an observation, verification status, observed changes, and new tabs. Verified field values and wait conditions are evidence for that operation, not a guarantee that the entire requested task is complete.

## Command-line workflows

Run predefined steps through the same engine in one process/session:

```sh
mint browser run --file browser-task.json
mint browser run --file browser-task.json --headed
```

Example `browser-task.json`:

```json
{
  "steps": [
    { "action": "browser_open", "input": { "url": "http://localhost:3000" } },
    { "action": "browser_fill", "input": { "selector": "input[name=search]", "text": "ทดสอบ" } },
    { "action": "browser_type", "input": { "selector": "input[name=search]", "text": " Mint" } },
    { "action": "browser_click", "input": { "selector": "button[type=submit]" } },
    { "action": "browser_wait", "input": { "condition": "text", "value": "Search results" } },
    { "action": "browser_screenshot", "input": { "mode": "fullPage" } }
  ]
}
```

Use selectors for targets defined in a workflow file. Use native browser tools and fresh `elementRef` values when the agent must inspect a page before choosing its next action. Workflows allow 1–100 browser steps, at most 1 MiB of input, and 120 seconds total. Individual steps are bounded at 31 seconds; explicit waits accept up to 30 seconds. Non-browser actions, disabled tools, and Plan Mode mutations are rejected before execution.

The command writes JSON to stdout, logs the real Chrome invocation and progress to stderr, and returns a nonzero exit code on failure. It preserves successful screenshots before a later failure. Standalone results and `report.json` live under `.mint/browser-runs/`; remove these workspace outputs when no longer needed. Ctrl+C closes the owned browser; running report checkpoints retain images from completed steps.

When an agent invokes a direct `mint browser run --file ...` command through foreground `run_shell`, Mint supplies a private report channel and imports its screenshots into the conversation automatically, including partial results from failed workflows. Successful imports remove their temporary workspace copies. Shell pipelines/wrappers and explicitly backgrounded commands do not use this automatic import path; use native tools or open the produced files explicitly with `view_image` instead.

## Images and history

Screenshots and native `view_image` results use the shared image artifact pipeline. Chat cards show the source URL, capture time and dimensions, with an expandable preview and original-image download. They remain visible with Activity collapsed, after progress trimming, and in saved completed, failed or interrupted turns. Reopening an existing artifact does not duplicate the card:

```json
{ "artifactId": "the-artifact-id-returned-by-the-tool" }
```

`view_image` still accepts a workspace `path` instead. Supply exactly one of `path` and `artifactId`. Artifacts cannot be opened by an agent from another conversation. Deleting a conversation removes its saved artifacts; unavailable files display an attachment error without breaking the chat. Legacy inline-image history remains supported.

PNG/JPEG inputs and screenshots are checked before decoding: at most 20 MiB and 32 million pixels. Images over 2 MB are resized/compressed for the model while preserving the original download. Very large captures fail with guidance to capture the viewport or one element. Existing small GIF/WebP/SVG attachments retain their legacy inline path; convert larger files to PNG/JPEG for model resizing. A text-only provider cannot perform visual verification even when the user can see a chat preview.

## Agent execution evidence

Shell outcomes retain typed status, exit code and job ID independently of stdout/stderr. Text resembling an exit line cannot change an actual successful process into a failed one. Commands promoted to background remain running until `shell_output` observes their final result. Poll jobs before retrying an unknown/running command.

A command may run again after relevant edits or changes. Three identical failures without changed files/output stop further attempts at that command; they do not mean the task succeeded. `run_shell.retryReason` explains a deliberate retry. Checks made before the latest file edit become stale. `finish.completionStatus` and saved run telemetry distinguish verified, unverified, blocked and failed results. Legacy finish calls remain accepted, but their status is derived from execution evidence, and limited results are explicitly annotated.

## Recovery and limits

CDP calls time out after 10 seconds. Observation failures caused by a disconnected or changing page are retried once; modifying actions are never automatically repeated after an ambiguous response.

Three attempts at the same action, DOM target, and relevant arguments without an action outcome change block further modifying actions, including tab opening and closing. Refreshed references, unrelated reads, clocks, carousels, and other page text changes cannot reset the count. Progress means navigation, a newly opened tab, changes to the original target's value/checked/expanded state, or verified scrolling. A successful explicit `browser_wait` clears only the attempt associated with the preceding action; clicks that change another region need this confirmation. Other attempt entries remain intact.

While blocked, tab listing/selection, observations, screenshots, reads, and waits remain available. Selecting another tab or satisfying a wait preserves the recovery stop. For login, CAPTCHA, or a recovery stop, the agent asks for manual takeover. After the user confirms completion, `browser_observe` with `takeoverComplete: true` refreshes references and clears the recovery stop.

Open shadow roots are supported. Iframes are counted in `unsupportedFrames`, but their contents and controls are not traversed. Rich editors, closed shadow roots, file uploads, and specialized date/range controls are outside this version's structured controls.

A run still has the existing 40-step limit. Sessions do not survive a Mint restart. Borrowed/persistent browser profiles share cookies and website session state; managed headless runs have separate temporary profiles. Tab leases prevent simultaneous Mint control of the same tab; they do not prevent a human from interacting with it.

## Coordinate evidence

A screenshot expires after 30 seconds and is invalidated by tab, document, navigation, scroll, or viewport changes. Capture stores a decoded image and [CDP DOMSnapshot layout evidence](https://chromedevtools.github.io/devtools-protocol/tot/DOMSnapshot/) from before and after the image. The coordinate is unknown when the first screenshot is requested, so its two layout snapshots are checked when coordinate input is requested: the original target must have the same node identity and geometry in both. At the coordinate, paint order and node ancestry identify the original target; ambiguous overlaps require a fresh screenshot or an element action.

During revalidation, the requested coordinate is known. A current capture retries once if its target identity/geometry or viewport changes around image capture. Changes to unrelated DOM nodes, text, or layout are allowed throughout capture and revalidation, including continuously updating distant tickers and carousels.

Location hit tests convert viewport coordinates to document coordinates using the captured scroll offsets; mouse input remains in viewport coordinates.

Before coordinate input, Mint requires the same DOM node and target geometry, an unobstructed [CDP location hit test](https://chromedevtools.github.io/devtools-protocol/tot/DOM/), and unchanged pixels in a 64×64 region around the coordinate. Changes outside that region are allowed. Cursor and aura overlays are hidden during evidence capture/comparison. Validation repeats after pointer movement, immediately before mouse press, to catch hover overlays. Failed evidence returns `screenshot_stale` and dispatches no mouse press. Canvas pixel changes and visually identical replacement nodes invalidate evidence too.

## Validation

Fast unit checks:

```sh
cargo test -p mint-core --lib browser:: --offline
cargo test -p mint-core --lib prompts:: --offline
```

Full browser acceptance checks require Chrome, permission to bind local ports, and network access for the two public-page smoke checks. They launch temporary headless browser profiles and remove those profiles afterward:

```sh
XDG_CONFIG_HOME=/tmp/mint-browser-acceptance cargo test -p mint-core --lib browser:: --offline -- --include-ignored --nocapture
```

The reliability and positive acceptance scenarios use the actual agent tool executor, including `AgentInput` deserialization and serialization defaults. Run the five-fix regressions without external websites:

```sh
XDG_CONFIG_HOME=/tmp/mint-browser-reliability cargo test -p mint-core --lib browser_reliability --offline -- --ignored --nocapture --test-threads=1
```

CLI (including the Full-Screen TUI), Desktop chat, and Web chat all call the shared Rust orchestration/session implementation. Gemini Live on Desktop and Web runs all voice tool calls within one browser session for the lifetime of the voice task, including successive tool-call batches and spoken turns. Ending the voice task releases its tab leases; the next voice session starts independently. The Live regression uses the production background runner and actual tool executor with local Chrome fixtures; it does not contact the Gemini service. Desktop's explicit manual navigation/click/type commands use the shared CDP targeting and editable-node helper; run-scoped agent recovery and screenshot evidence are enforced at the common agent tool executor.

The acceptance scenarios do not measure model accuracy or guarantee success on arbitrary websites.
