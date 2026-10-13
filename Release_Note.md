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
