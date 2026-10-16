# Release Notes - Mint Agent v1.16.0

## Highlights

- New workspace mods use **`.mint-mods/<id>/`**; existing `mint-mods/<id>/` sources remain supported. The hidden folder takes precedence for duplicate IDs, while legacy edits stay in place. Moved this workspace’s `token-weather` mod into `.mint-mods` with its files unchanged.
- Added **Mint Mods**: custom panels, commands, and tool guards across CLI, Desktop, and Web, with hot reload, permission review, and import/export.
- Moved Live2D into the optional **Mint Companion** app, which follows a selected Mint conversation without starting another backend.
- Changed Mint’s project license to **MIT**. Third-party dependencies and bundled assets retain their own licenses.

## Terminal and chat

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
