# Mint Release Notes

## v1.5.2

This release improves desktop readability and adds repository-aware CLI tools for summarizing codebases, indexing symbols, and searching source code semantically.

### Repository Intelligence, Semantic Code Search, and CLI Readability

This patch makes Mint more useful inside a project workspace. The CLI can now inspect a repository directly, build a lightweight source symbol index, and create a semantic code index backed by Gemini embeddings.

#### Highlights

- **Desktop UI Polish:** Updated the app interface layout and control styling for a cleaner, more organized desktop experience.
- **Clearer Conversation Flow:** Improved chat presentation so user and AI messages are easier to distinguish while reading.
- **Readable AI Replies:** Adjusted AI response formatting to produce cleaner, more structured replies with better spacing and less clutter.
- **Thai Reply Handling:** Improved response behavior for Thai conversations so replies remain natural and easier to read.
- **Reduced Visual Noise:** Tightened UI states and message rendering so important conversation content stays the focus.
- **Repository Summary:** Added `mint summarize [path]` and `/summarize [path]` to report project structure, package metadata, git state, languages, directories, and important files without requiring a full AI agent turn.
- **Symbol Index:** Added `mint symbols [path]` and `/symbols [path]` to scan JavaScript, TypeScript, Python, and Rust source files for functions, classes, exports, interfaces, types, structs, enums, traits, and related symbols.
- **Semantic Code Search:** Added `mint semantic-code index` and `mint semantic-code search <query>` for embedding source chunks and searching code by meaning.
- **Natural Workspace Requests:** The interactive CLI can detect plain-language requests such as repository summaries, symbol indexes, and semantic code searches, including Thai phrasing, and route them to the local tools.
- **Working Timer:** The CLI now shows how long Mint has been working while the agent is thinking.
- **Cleaner Streaming:** Empty assistant/system chunks are filtered so live replies do not create blank messages.
- **Unified Patch Preview:** Code-agent patch approvals now show a unified-diff style preview with surrounding context before applying edits.

#### New CLI Commands

- `mint summarize [path]`
- `mint summarize [path] --json`
- `mint symbols [path]`
- `mint symbols [path] --json --limit 120`
- `mint semantic-code index [path]`
- `mint semantic-code search "<query>" --path <path> --top-k 5`

#### New Interactive Commands

- `/summarize [path] [--json]`
- `/summary [path] [--json]`
- `/symbols [path] [--json] [--limit n]`
- `/symbol-index [path] [--json] [--limit n]`
- `/semantic-code index [path]`
- `/semantic-code search <query>`
- `/semantic index [path]`
- `/semantic search <query>`

#### Repository Summary

The repository summary tool reports:

- root path
- scanned file count
- package name, version, description, scripts, dependencies, and dev dependencies
- git branch, short status, and diff stat
- top-level directory counts
- language counts by extension
- important files such as README, package metadata, release notes, source files, tests, config files, and CI files

#### Symbol Index

The symbol indexer scans supported source files locally and reports:

- source files scanned
- files containing symbols
- total symbol count
- counts by symbol kind
- counts by language
- symbol names with file and line references

Supported source extensions:

- `.js`, `.cjs`, `.mjs`, `.jsx`
- `.ts`, `.tsx`
- `.py`
- `.rs`

#### Semantic Code Search

Semantic code search creates workspace-specific indexes under `~/.config/mint/semantic-code`.

- Uses `gemini-embedding-001` by default.
- Reads the Gemini API key from Mint config or `GEMINI_API_KEY`.
- Chunks source files with overlap so search results can point back to file and line ranges.
- Includes file path, language, and local symbol names in each embedded chunk.
- Stores file hashes with the index metadata for workspace tracking.
- Supports formatted output and `--json` output.

#### Code Agent Patch Preview

Patch approvals now use unified diff previews:

- shows `--- a/<path>` and `+++ b/<path>` headers
- includes `@@` range metadata
- includes nearby unchanged context
- displays removed and added lines with `-` and `+`
- falls back to a clear preview error if a hunk cannot be matched

## v1.5.1

This release updates the desktop Live2D experience, CLI response loop, learned skills, and image attachment UX.

### Desktop Live2D Model and Interaction UI

This patch adds an optional Live2D assistant model to the desktop UI and improves the top toolbar for easier model control.

#### Highlights

- **Live2D Assistant Model:** Added a Shiroko Live2D model panel to the Electron desktop assistant.
- **Model Visibility Preference:** The model panel can be shown or hidden from the toolbar. New installs start with the model hidden until enabled.
- **Expression Cycling:** Added a toolbar control for cycling Live2D expressions.
- **Expression Toast:** The active expression name is shown in a small on-canvas toast after changing expressions.
- **Click-to-Chat Reactions:** Clicking named model areas can trigger temporary expressions and send a short contextual prompt into the normal chat flow.
- **Interaction Area Overlay:** Added a transparent guide overlay for clickable regions such as `Head Pat`, `Cheek Poke`, `Hand Tap`, `Shoulder Tap`, and `Careful`.
- **Safe Missed Clicks:** Clicking outside the defined interaction zones no longer changes expression, sends chat prompts, consumes AI tokens, or starts cooldown.
- **Interaction Cooldown:** Model reactions use a 3-second cooldown to reduce accidental repeated AI calls.
- **Live2D Lip Sync:** Mint animates mouth parameters while speaking and resets them when speech ends.
- **Toolbar Redesign:** Desktop header controls are grouped into clearer model, utility, and window-control sections with readable labels.

#### Live2D Desktop Model

- The model is loaded from `models/Shiroko_Model`.
- The desktop model can be toggled from the `Model` toolbar button.
- The first launch defaults to hiding the model panel.
- The visibility setting is stored in `localStorage` as `mint-model-hidden`.
- Expression controls use the configured Live2D expression files and reset expression parameters before switching to avoid stuck expressions.
- Temporary click reactions return to the previously selected expression after 2 seconds.

#### Interaction Zones

The interaction guide overlay labels clickable regions without blocking pointer input:

- `Head Pat`
- `Cheek Poke`
- `Hand Tap`
- `Shoulder Tap`
- `Careful`

Only clicks inside these configured regions trigger Live2D reactions. Clicks outside the regions are ignored.

### Fast Mode, Live Replies, and Learned Skills

This patch improves the Mint CLI interaction loop and adds persistent skill/instruction learning from local markdown or text files.

#### Highlights

- **Agent Status Label:** The interactive CLI now shows `[Agent]` instead of `[Chat]` for the normal agent loop.
- **Fast Mode:** Added `/fast`, `/fast on`, `/fast off`, and `/fast status`.
- **Fast Status:** When Fast Mode is enabled, the status bar shows `[Fast]`.
- **Quiet Trace Output:** Fast Mode keeps `● Mint is thinking...` visible but hides `Thinking:` and intermediate tool/progress trace messages.
- **Live Mint Replies:** Final Mint responses now render into one live-updating `Mint` message block instead of appearing all at once or splitting into multiple `Mint` messages.
- **Learned Skills:** Added `mint learn <path>` and `/learn <path>` for importing local `.md` or `.txt` files as persistent skill/instruction context.
- **Skill Management:** Learned skills can be listed and deleted from both the command line and interactive CLI.
- **Expanded Slash Suggestions:** `/memory skills` is now available in the `/` command suggestions.
- **Expanded Tests:** Test suite now reports **137 passing tests**.

#### Fast Mode

New interactive commands:

- `/fast` toggles Fast Mode.
- `/fast on` enables Fast Mode.
- `/fast off` disables Fast Mode.
- `/fast status` shows the current Fast Mode state.

Fast Mode changes presentation only. Agent routing, tool calls, approvals, final answers, and provider/model behavior continue to work normally.

#### Live Response Rendering

- `Thinking:` messages can appear before the final answer when Fast Mode is off.
- Final responses stream into one active `Mint` message block.
- The completed message is stored in history after the response finishes.
- Fast Mode still streams the final answer, but hides the internal `Thinking:` trace.

#### Learned Skills

New command-line examples:

- `mint learn ./skill.md`
- `mint learn ./instructions.txt`
- `mint learn --list`
- `mint learn --delete <id|path|name>`

New interactive CLI examples:

- `/learn ./skill.md`
- `/memory skills`
- `/memory skills delete <id|path|name>`

Learned files are stored in Mint's SQLite memory as persistent skill/instruction documents. Mint injects learned skills into long-term context on later turns.

### CLI Image Attachments & Paste UX

This patch improves the interactive Mint CLI input experience, especially for screenshots, clipboard images, and long pasted text.

#### Highlights

- **CLI Image Attachments:** Added `--image <path>` support for `mint chat` and `mint code`.
- **Interactive Image Paste:** `Ctrl+V` can attach clipboard images inside the interactive CLI without sending immediately.
- **Multiple Images:** Users can attach several images before pressing Enter; the UI labels them as `[Image #1]`, `[Image #2]`, and so on.
- **Attachment Controls:** `Ctrl+Backspace` removes the latest pending image, and `Esc` clears pending images or pasted content before exiting.
- **Long Paste Placeholder:** Long or multi-line pasted text is collapsed in the input as `[Pasted Content N chars]` while preserving the full content for sending.
- **Natural Composition Order:** Text typed before an image or pasted content, the attachment/content itself, and text typed afterward are sent in the same natural order.
- **Vision Context Follow-up:** The CLI keeps lightweight context from the latest image turn so follow-up questions can refer back to recently attached images.
- **Expanded Tests:** Test suite now reports **137 passing tests**.

#### CLI Image Input

- Added `src/CLI/image_input.js`.
- Supported image formats:
  - `.png`
  - `.jpg`
  - `.jpeg`
  - `.webp`
  - `.gif`
- New command-line examples:
  - `mint chat --image ./screenshot.png "What is on this screen?"`
  - `mint code --image ./mockup.png "Build this UI"`
- Interactive CLI examples:
  - `Ctrl+V` to attach a clipboard image.
  - `/paste [prompt]` to attach a clipboard image through a command.
  - `/image ./screenshot.png [prompt]` to attach an image file while already inside `mint`.

#### Interactive CLI UX

- Clipboard images are now attached first and sent only when the user presses Enter.
- Pending image labels are rendered inside the input box.
- Multiple pending images are shown as compact labels:
  - `[Image #1] [Image #2]`
- Fixed terminal input edge cases where `Ctrl+V` could insert stray `v` characters.
- Long pasted text no longer breaks the input border or terminal layout.
- Pasted content can be combined with typed text:
  - text before paste
  - pasted content
  - text after paste
- Image attachments follow the same composition model:
  - text before image
  - image labels
  - text after image

#### Provider and Vision Handling

- Gemini image MIME handling now preserves the real image MIME type instead of assuming PNG.
- Chat provider paths can receive multiple images in one request where supported.
- Code Agent multimodal requests now pass attached image context to supported providers.

### Current Test Status

- **137 tests passed**
- **20 test suites passed**
