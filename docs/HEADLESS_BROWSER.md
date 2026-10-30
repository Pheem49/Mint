# Headless Browser Automation & Workflows

Mint provides a modern, fast, and loop-resilient browser automation architecture designed for autonomous agents and deterministic CLI workflows without requiring a heavy headed browser UI.

---

## Architecture Overview

Instead of maintaining a persistent, headed browser window or running background daemon ports (`mint auto`), Mint uses:

1. **Agent-Facing Headless Chromium via Shell**: Autonomous agents inspect websites, SPAs, and local HTML using lightweight, disposable headless Chromium processes executed through `run_shell`.
2. **Deterministic Workflow Runner (`mint browser run`)**: Predefined multi-step automation and testing workflows defined in JSON files run headlessly via a dedicated Rust CDP engine.
3. **Loop & Repetition Protection**: Multi-layered guards prevent infinite screenshot/scroll loops, deduplicate image reviews, and intercept stuck actions.
4. **System Default Browser for Previews**: User-facing artifact and HTML previews open cleanly in your operating system's default browser across CLI, Desktop, and Web.

---

## Agent Headless Playbook

When an agent needs to inspect a web page or render a local artifact, it executes headless Chromium directly using standard shell commands:

### DOM & Text Extraction (Preferred)

For reading page content, finding selectors, and extracting information, the agent uses `--dump-dom`:

```bash
google-chrome --headless=new --disable-gpu --dump-dom "https://example.com"
```

### Visual Screenshot Capture

When visual rendering or verification is required, the agent captures a screenshot:

```bash
google-chrome --headless=new --disable-gpu --window-size=1440,900 --screenshot=capture.png "https://example.com"
```

To view the captured screenshot, the agent calls `view_image`:

```json
{
  "path": "capture.png"
}
```

### Managed Headless Runtime Enhancements

The shell execution engine automatically optimizes headless Chromium calls:
- **Sandbox Safety**: In containerized or root environments (e.g., Docker, CI), `--no-sandbox` is added automatically to prevent startup failures.
- **Isolated Per-Run Profiles**: Runtimes use isolated temporary user-data directories (`--user-data-dir=...`) that are cleaned up automatically after execution.
- **Virtual Time Budget**: Adds `--virtual-time-budget=5000` when needed to allow dynamic SPAs and web fonts to settle before capture.
- **Image Deduplication**: The agent's `view_image` tool computes SHA-256 content hashes of viewed images, preventing repetitive analysis of identical page states.

---

## Repetition Guard & Anti-Loop Safeguards

To prevent agents from getting caught in infinite visual inspection loops:

- **Rolling Result Window**: The orchestration engine tracks command and tool result fingerprints across a rolling window of 8 executions.
- **Stuck Loop Interception**: If an identical command is executed $\ge 3$ times with unchanged output, the loop guard intercepts execution with actionable guidance (`InterceptStuckLoop`), requiring the agent to synthesize from existing findings or try a different approach.
- **Single Visual Review**: Viewing an identical screenshot multiple times is suppressed with a cache notice, steering the model toward progressive execution.

---

## Deterministic Workflow Runner (`mint browser run`)

For scripted testing and repeatable multi-step tasks, Mint provides a headless workflow runner:

```bash
mint browser run --file workflow.json
```

### Workflow File Format

A workflow file specifies an array of steps using supported browser actions:

```json
{
  "steps": [
    { "action": "browser_open", "input": { "url": "http://localhost:3000" } },
    { "action": "browser_fill", "input": { "selector": "input[name=username]", "text": "testuser" } },
    { "action": "browser_click", "input": { "selector": "button[type=submit]" } },
    { "action": "browser_wait", "input": { "condition": "text", "value": "Dashboard" } },
    { "action": "browser_screenshot", "input": { "mode": "fullPage" } }
  ]
}
```

### Supported Workflow Actions

The workflow runner supports 14 headless browser actions:
- `browser_open`: Navigate to a URL.
- `browser_fill`: Replace an input field's value.
- `browser_type`: Type characters into an active field.
- `browser_click`: Click an element matching a CSS or XPath selector.
- `browser_select`: Select an option in a dropdown menu.
- `browser_scroll`: Scroll the page or target element.
- `browser_wait`: Wait for text, URL, or element visibility/invisibility.
- `browser_screenshot`: Capture viewport or full-page screenshots to `.mint/browser-runs/`.
- `browser_observe`: Extract structured page state and controls.
- `browser_tabs`: Manage browser tabs (`list`, `open`, `select`, `close`).
- `browser_read`: Extract text content from the page.
- `browser_hover`: Hover over a selector.
- `browser_press_key`: Send keyboard key combinations.
- `browser_evaluate`: Run JavaScript expressions in the page context.

### Execution Boundaries

- **Step Limits**: 1 to 100 steps per workflow.
- **Payload Size**: Up to 1 MiB JSON file size.
- **Timeout**: 120 seconds total workflow duration; up to 30 seconds per individual step.
- **Strictly Headless**: The runner executes headlessly with isolated loopback ports; headed GUI windows and manual overrides are rejected.
- **Output Storage**: Run reports and screenshots are written to `.mint/browser-runs/<timestamp>/`.

---

## Slash Command `/browser`

The `/browser` slash command is dedicated to diagnostic status:

```
/browser
/browser status
```

It checks and reports:
- The path to the detected Chromium-based executable (`google-chrome`, `chromium`, `brave`, `msedge`, or configured `browserExecutable`).
- Binary version and readiness.
- Configuration status.

---

## Opening Previews in System Browser

When working with HTML artifacts or starting local previews:
- In **CLI Full-Screen TUI / Classic**: Previews open via the operating system's default browser handler (`xdg-open` on Linux, `open` on macOS, `start` on Windows).
- In **Desktop UI**: Clicking **Open preview** in the Artifact Preview panel invokes the system browser via Tauri desktop action.
- In **Web UI**: Clicking **Open preview** opens the served URL in a new browser tab.
