# Mint Release Notes

## v1.5.0 Update - Provider Visibility, OAuth Integrations & Plugin Expansion

This update expands Mint's integration layer and improves provider transparency across the desktop and CLI experiences.

### Highlights

- **Provider/Model Visibility:** Desktop chat and CLI status now show the actual provider and model that answered, including fallback results.
- **Gmail API Integration:** Added a safe Gmail plugin for searching/reading email and creating drafts.
- **Gmail OAuth Helper:** Added `mint gmail auth` and `mint gmail auth --no-open` to generate and save Gmail refresh tokens.
- **Google Calendar API Integration:** Calendar plugin can now list events and create events through the Calendar API, with browser fallback when OAuth is not configured.
- **Notion API Integration:** Added Notion plugin support for creating pages, querying databases, and appending page blocks.
- **Onboarding Expansion:** `mint onboard` now supports Google Calendar API, Gmail API, and Notion API setup.
- **Provider Routing Cleanup:** OpenAI/local placeholders and empty local URLs no longer cause misleading fallback attempts.
- **Expanded Tests:** Test suite increased to **121 passing tests** across **19 test suites**.

### New Integrations

- **Gmail Plugin**
  - Added `src/Plugins/gmail.js`.
  - Supports:
    - Gmail search/list queries, such as inbox and unread mail.
    - Reading full message content by message ID.
    - Creating drafts for review before sending.
  - Does **not** send email automatically.
  - Uses OAuth scopes:
    - `https://www.googleapis.com/auth/gmail.readonly`
    - `https://www.googleapis.com/auth/gmail.compose`

- **Gmail OAuth CLI**
  - Added `src/CLI/gmail_auth.js`.
  - Added CLI commands:
    - `mint gmail auth`
    - `mint gmail auth --no-open`
  - `--no-open` prints the OAuth URL without opening a browser.
  - Onboarding can now start Gmail OAuth automatically after Gmail Client ID/Secret are configured and the refresh token is blank.
  - Saved config fields:
    - `gmailClientId`
    - `gmailClientSecret`
    - `gmailRefreshToken`
    - `gmailUserId`
    - `pluginGmailEnabled`

- **Google Calendar Plugin**
  - Reworked `src/Plugins/google_calendar.js`.
  - Supports:
    - Listing today's or upcoming events.
    - Creating timed or all-day events.
    - Browser fallback for opening Google Calendar or event creation when OAuth is not configured.
  - Saved config fields:
    - `googleCalendarClientId`
    - `googleCalendarClientSecret`
    - `googleCalendarRefreshToken`
    - `googleCalendarId`
    - `pluginCalendarEnabled`

- **Notion Plugin**
  - Added `src/Plugins/notion.js`.
  - Supports:
    - Creating Notion pages/notes.
    - Querying a Notion database.
    - Appending blocks to a Notion page.
  - Saved config fields:
    - `notionApiKey`
    - `notionDatabaseId`
    - `notionPageId`
    - `notionTitleProperty`
    - `pluginNotionEnabled`

### CLI and Settings Improvements

- `mint` startup now prints `Active AI: provider • model`.
- CLI status bar updates to the provider/model that most recently answered.
- Desktop chat displays provider/model metadata for AI responses.
- Settings now includes Gmail and Notion plugin toggles.
- `mint list` now includes `mint gmail auth`.
- `README.md` now documents Gmail, Google Calendar, Notion, and provider/model visibility.

### Provider and Config Fixes

- `localApiBaseUrl` now defaults to an empty value instead of `http://localhost:1234/v1`.
- `ollamaHost` now defaults to an empty value in settings/config so users can set it manually.
- Placeholder keys such as `your_openai_key_here` are treated as unset before API calls.
- Provider attempt order now skips configured providers that are not actually available.
- Onboarding resets `aiProvider` back to Gemini when no optional AI provider is selected, preventing stale OpenAI/local selections.

### New Tests

- Added Gmail plugin tests.
- Added Gmail OAuth helper tests.
- Added Google Calendar plugin tests.
- Added Notion plugin tests.
- Updated provider routing tests for unavailable providers.

### Current Test Status

- **121 tests passed**
- **19 test suites passed**

## v1.5.0 - Unified Agent, Safety Layer & Architecture Hardening

Mint 1.5.0 is a major agent-readiness release. It turns the CLI into a true unified agent loop, adds a central safety manager, hardens dependency security, introduces CI, and refactors the Electron main process into focused modules.

### Highlights

- **Unified CLI Agent:** The default `mint` command now sends normal chat and coding requests through the same agent loop.
- **Thinking by Default:** Every CLI message goes through an agent decision step before responding or using tools.
- **Central Safety Manager:** New deterministic safety layer for shell commands, high-risk actions, path boundaries, and action logging.
- **Provider Fallback:** Code Agent can fall back from local OpenAI-compatible providers to cloud providers such as Gemini.
- **Main Process Refactor:** `main.js` was reduced to a bootstrap/wiring file and split into dedicated system modules.
- **Security Baseline:** `npm audit --audit-level=high` now reports `0 vulnerabilities`.
- **Expanded Tests:** Test suite increased to **121 passing tests** across the full v1.5.0 update line.

### New Features

- **Unified Agentic CLI**
  - `mint` now behaves as a single AI agent instead of separating normal chat and Code Mode by default.
  - Casual messages can finish directly in one step.
  - Coding or workspace tasks can inspect files, search code, run approved commands, apply patches, and verify results.
  - The CLI status reflects Agent/Chat mode while keeping one unified interaction flow.

- **Safety Manager**
  - Added `src/System/safety_manager.js`.
  - Introduced permission tiers:
    - `safe`
    - `approval`
    - `dangerous`
    - `blocked`
  - Added deterministic command blocking for high-risk shell commands, including:
    - `rm -rf`
    - `git reset --hard`
    - `git clean -f`
    - `mkfs`
    - raw disk writes
    - `shutdown`
    - `reboot`
    - `sudo`
    - `chmod -R 777`
    - `curl | sh`
    - `wget | bash`
  - Added path traversal protection with `resolveWithinRoot()`.
  - Added local action audit logging to:
    - `~/.config/mint/action-log.jsonl`

- **Action Safety Integration**
  - Integrated safety checks into:
    - Code Agent shell execution
    - Electron/system action executor
    - CLI action bridge
    - Autonomous brain file actions
  - Dangerous actions such as `delete_file` and destructive `system_automation` now require explicit permission.

- **Provider Fallback for Code Agent**
  - Code Agent now builds a supported provider order.
  - If `local_openai` is configured but offline, the agent can fall back to another available provider such as Gemini.
  - Fallback logs are hidden by default and can be shown with `MINT_DEBUG=1`.

- **Internal Google TTS URL Generator**
  - Removed vulnerable `google-tts-api`.
  - Added internal TTS URL generation in `src/System/google_tts_urls.js`.
  - Added tests for empty text, URL generation, and chunk splitting.

- **Safer Excel Parsing**
  - Removed vulnerable `xlsx`.
  - Replaced spreadsheet reading with `read-excel-file`.
  - Knowledge base can still index `.xlsx` files across sheets.

### Architecture Changes

- **Electron Main Process Split**
  - `main.js` is now a small bootstrap file.
  - Added focused system modules:
    - `src/System/window_manager.js`
    - `src/System/ipc_handlers.js`
    - `src/System/proactive_loop.js`
    - `src/System/screen_capture.js`
    - `src/System/action_executor.js`
  - This reduces coupling between window creation, IPC handlers, proactive analysis, screen capture, and action execution.

- **Action Executor Consolidation**
  - Shared action execution now lives in `src/System/action_executor.js`.
  - Electron and proactive action handling use the same executor path.

- **Improved Code Agent Flow**
  - Conversational/trivial requests are now encouraged to `finish` directly without unnecessary workspace inspection.
  - Tool result progress no longer duplicates thought output.
  - Shell, patch, and write actions are logged when approved.

### CI, Testing & Maintenance

- **GitHub Actions CI**
  - Added `.github/workflows/ci.yml`.
  - CI now runs:
    - `npm ci`
    - `npm test -- --runInBand`
    - `npm audit --audit-level=high`

- **New Tests**
  - Added safety manager tests for:
    - destructive command blocking
    - safe command classification
    - dangerous action classification
    - explicit permission enforcement
    - path traversal prevention
  - Added action executor safety tests for dangerous action blocking.
  - Added Google TTS URL generation tests.

- **Current Test Status**
- **121 tests passed**
- **19 test suites passed**

### Documentation

- Rewrote `README.md` to document all major current features.
- Added sections for:
  - Unified CLI Agent
  - Desktop Assistant
  - Automation
  - Knowledge and Memory
  - Multi-provider AI
  - Messaging Bridges and Plugins
  - MCP Extensions
  - Safety System
  - Project Structure
  - Runtime Notes

### Security Notes

- This release makes Mint safer by adding deterministic policy checks in code, not just prompt instructions.
- Some system-level plugins and desktop automation paths can still be expanded further with deeper permission prompts and UI-level confirmation.
- `system_automation` remains powerful and should be used carefully.

### Migration Notes

- If you used `xlsx` directly through Mint internals, it has been replaced with `read-excel-file`.
- If you relied on `google-tts-api`, Mint now generates Google Translate TTS URLs internally.
- Action logs may now appear under `~/.config/mint/action-log.jsonl`.
- Local OpenAI-compatible providers should be running before use; otherwise Mint can fall back to other configured providers.
