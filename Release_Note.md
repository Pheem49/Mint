# Release Notes - Mint Agent v1.16.0

## Highlights

- Updated the locked `source-map-js` dependency from 1.2.1 to 1.2.2 to fix GHSA-68fv-2mgg-jv7q and restore the CI `npm audit --audit-level=high` check.

- New workspace mods use **`.mint-mods/<id>/`**; existing `mint-mods/<id>/` sources remain supported. The hidden folder takes precedence for duplicate IDs, while legacy edits stay in place. Moved this workspace’s `token-weather` mod into `.mint-mods` with its files unchanged.
- Added **Mint Mods**: custom panels, commands, and tool guards across CLI, Desktop, and Web, with hot reload, permission review, and import/export.
- Moved Live2D into the optional **Mint Companion** app, which follows a selected Mint conversation without starting another backend.
- Changed Mint’s project license to **MIT**. Third-party dependencies and bundled assets retain their own licenses.

## Terminal and chat

- Corrected parallel tool activity: identical calls retain separate cards and call IDs, blocked calls display failure, and parallel subagent dispatches report live status and actual elapsed time.

- TUI `/resume` now restores saved tool cards, full input/output through **Ctrl+O**, reasoning through **Ctrl+T**, and per-turn completion/failure summaries. New tool events persist elapsed time; older activity without timing displays **time unavailable**. Unfinished saved calls display **Interrupted** without restarting a spinner, and reloading or switching sessions clears stale viewers and avoids duplicate history.

- Restored pinned live tool details above the TUI composer after compact tool cards hid them: active counts, abbreviated commands, animated spinners, and elapsed timers stay visible while scrolling the transcript and take priority over older status lines. Finished calls leave this live area while their compact cards remain in history.

- TUI tool calls now use compact three-line cards that update in place on completion, keeping long commands and output out of the default transcript. **Ctrl+O** opens full input/output, **Tab/Shift+Tab** switches calls, and **Ctrl+Y** copies all details. Active counts separate shell commands from other tools without counting completed calls; failures display in red and successful calls in green.

- TUI now streams reasoning into a preview capped at six terminal rows, with a live **Thinking** timer and automatic scrolling to the latest text. Completed blocks collapse to **Thought · … (Ctrl+T to view)**. Ctrl+T opens the full reasoning during and after an agent turn, updates as text arrives, and preserves interrupted reasoning for review.

- Added a startup TUI update menu with **Update now**, **Skip** (selected by default), and **Skip until next version**. Skipped versions persist across launches; newer releases still notify. Installation runs the existing approved npm updater outside the TUI and exits on success so Mint can be restarted. Slow background checks keep a passive notice instead of interrupting chat.

- Improved Full-Screen TUI scrolling, drag selection, Back to bottom, multiline input, and long approval previews.
- Added collapsible long pastes: **F2** opens or folds the full text for editing without losing submitted content.
- Improved Desktop/Web chat performance, model selection, slash-command navigation, activity summaries, and responsive layouts.
- Added clearer context-compaction status and background-terminal counts.

## Workspace and background jobs

- Added persistent file history and **Undo**, including AI edits without Git. File removal uses Trash with confirmation.
- Fixed project switching and workspace selection so navigation preserves conversation ownership and drafts.
- Added workspace previews for HTML, Markdown, and images, plus improved Review and Desktop file-pane controls.
- Added background-job logs, Stop, finished-job removal, and browser launch controls. **`/mint-web`** starts a tracked Web session; **`/shells`** manages jobs across interfaces.
- Improved shutdown cleanup. Background jobs do not resume across backend restarts.

## Agent, tools, and approvals

- Unified skill discovery, model choices, media storage, messaging webhooks, and approval handling across the shared Rust core.
- Skill creation and edits now require a separate proposal review; automatic skill suggestions default off.
- Improved cancellation, plan-mode handling, repeated-tool-failure detection, and stale approval protection.
- Added schema validation, persistent attachments, and explicit success/failure states for MCP tools, with configurable timeouts and cancellation. Timed-out or cancelled calls may still finish remotely and are not retried automatically.
- Added opt-in **Mint Device** command verification with a mock connector. Real hardware requires a device-specific adapter; cancellation does not issue a physical stop.

## Desktop and Web

- Added **Sources** beside Tools in the Web chat header. Its mobile-friendly session panel shares Desktop's source loading, attachments, web-search links and favicons, with loading/retry states and Library navigation that preserves drafts.
- Workspace context Sources now shows website favicons for web-search pages on Desktop/Web, using the existing source metadata. Unavailable favicons fall back to the globe; broken attachment previews use their file-type icon without changing thumbnail sizing.
- `view_image` now shows viewed images inline in Desktop/Web chat during a run and in saved history, with filenames and an expandable preview. Images remain visible with Activity collapsed, while tool details show a readable status instead of base64. CLI/TUI keeps its text status.
- Renamed **Pictures** to **Library** on Desktop and Web, with shared All / Images / Videos / Files filters, filename search, current-conversation scope, cursor pagination, previews, original-name downloads, and reversible Library Trash. `/library` is canonical; `/pictures` remains compatible.
- CLI/TUI uploads now use the same pre-AI Library intake and turn references as Desktop/Web, retaining files on provider failure. Agent-generated images/videos retain their conversation and running-turn references. Library refresh preserves loaded cursor pages while thumbnails update; Sources opens Library inside the app without discarding the composer.
- Library imports flush original files before acquiring the catalog writer lock, reducing contention during concurrent uploads. Failed catalog batches and duplicate retries clean up their staged files.
- Library previews now show the saved source message in up to two lines, omitting chat transport and attachment markers; entries without message text omit the excerpt.
- Added **Remove from Mint** in Library Trash on Desktop and Web. It moves remaining originals and thumbnails to the system Trash on the machine running Mint, then removes their Library entries and source references. Missing-file records can be removed; failed moves retain the entry for retry. Ordinary Library deletion still moves entries to Library Trash for restore.
- Uploaded chat images, videos, and supported documents/code files are now saved before document extraction or AI calls. Library stores originals once on disk with a SQLite catalog, stable IDs, names, MIME types, sizes, and conversation/turn references. Retries reuse a submission receipt; failed requests retain the composer until acceptance. AI-generated media uses the same catalog.
- Migrates the old Pictures JSON catalog idempotently, preserves existing IDs and file paths, and keeps a backup of the original index. Missing originals remain visible. Thumbnail work runs asynchronously with at most two workers and pending work resumes at startup.
- Workspace context **Sources** lists session images, files, videos, and AI web-search results with expandable groups. New attachments link to the same Library original and thumbnail; Trash and missing-file states stay visible. Clearing or deleting chat history removes its references while Library files remain. Earlier metadata-only attachments are retained as metadata; their originals cannot be reconstructed.
- Moved **Mods** and **Background tasks** from the chat composer into Tools tabs on Desktop and Web, accessible from the launcher and add-tool menu without a duplicate button row below the tabs. Added a Web Tools button and a running-task badge that stays visible while Tools is closed; the Desktop interactive Terminal remains separate. Mod bands and background-job tracking continue while the panel is closed.
- Refined voice recording, Gemini Live calls, notifications, error recovery, theme styling, and accessibility.
- Added **Reduce effects** and reduced unnecessary rendering work.
- Improved Saved Pictures browsing and previews, image-search cards, and MCP settings.
- Live Translate now uses the selected chat provider and model, including local/custom providers; unsupported image input reports an error instead of switching providers.

## Installation and documentation

- Ignored workspace-local `.mint-mods/` in Git; shareable mod examples remain in `examples/mods/`.
- Consolidated Desktop launch/build commands and improved source-checkout rebuild detection; failed builds stop before launching an older binary.
- Development builds read release notes live; shipping builds embed a snapshot.
- Shortened the README, moved detailed guides into `docs/`, fixed installation navigation, clarified mod sharing, and updated the roadmap around the primary TUI workflow.
- Condensed these release notes by topic. The original detailed change log is preserved in [v1.16.0 details](docs/releases/v1.16.0-details.md).
