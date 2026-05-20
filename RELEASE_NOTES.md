# Mint Release Notes

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
