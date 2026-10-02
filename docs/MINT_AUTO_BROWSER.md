# Mint Auto browser tasks

Mint Auto starts an isolated Chrome/Chromium profile. Browser agent runs create a dedicated tab and keep every operation pinned to it. The profile retains browser login state; the task session and its element references live only for the current run.

## Agent workflow

1. Navigate with `browser_open` and inspect its returned observation.
2. Use an `elementRef` from that observation to click, fill, or select a control. Existing CSS, text, and XPath selectors still work.
3. Inspect the observation returned by the action. References from previous observations expire.
4. Use `browser_wait` to check the expected URL, text, or element visibility. A dispatched click alone is not evidence of task completion.
5. Include observed evidence in `finish.verification`, or explain that the task is blocked or unverified.

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
| `browser_screenshot` | Capture the selected tab as an image. Coordinate input requires a screenshot from this tab within 30 seconds. Navigation and viewport changes invalidate it. |

Supply exactly one of `elementRef` and `selector` for targeted operations. Supported text fields are textarea and text, search, URL, telephone, password, email, and number inputs. Both edit tools pin the original node and verify it is connected, enabled, editable, and focused after clicking and again after keyboard preparation. `focus_changed` or `stale_reference` stops the operation before insertion/deletion; the tools do not refocus or retry. Password values are excluded from observations and browser typing logs.

Action results distinguish `input_dispatched` from `failed_or_ambiguous` and include an observation, verification status, observed changes, and new tabs. Verified field values and wait conditions are evidence for that operation, not a guarantee that the entire requested task is complete.

## Recovery and limits

CDP calls time out after 10 seconds. Observation failures caused by a disconnected or changing page are retried once; modifying actions are never automatically repeated after an ambiguous response.

Three attempts at the same action, DOM target, and relevant arguments without an action outcome change block further modifying actions, including tab opening and closing. Refreshed references, unrelated reads, clocks, carousels, and other page text changes cannot reset the count. Progress means navigation, a newly opened tab, changes to the original target's value/checked/expanded state, or verified scrolling. A successful explicit `browser_wait` clears only the attempt associated with the preceding action; clicks that change another region need this confirmation. Other attempt entries remain intact.

While blocked, tab listing/selection, observations, screenshots, reads, and waits remain available. Selecting another tab or satisfying a wait preserves the recovery stop. For login, CAPTCHA, or a recovery stop, the agent asks for manual takeover. After the user confirms completion, `browser_observe` with `takeoverComplete: true` refreshes references and clears the recovery stop.

Open shadow roots are supported. Iframes are counted in `unsupportedFrames`, but their contents and controls are not traversed. Rich editors, closed shadow roots, file uploads, and specialized date/range controls are outside this version's structured controls.

A run still has the existing 40-step limit. Sessions do not survive a Mint restart. The browser profile is shared, so separate task tabs share cookies and website session state. Tab leases prevent simultaneous Mint control of the same tab; they do not prevent a human from interacting with it.

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
