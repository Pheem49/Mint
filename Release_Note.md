# Release Notes - Mint Agent v1.8.2

We are excited to release **Mint Agent v1.8.2**! This release introduces a configurable Multi-Agent system for sequential collaboration, powerful local workspace file management features, real-time AI-generated workflow suggestions in the user's active language, mobile layout optimization, and general UI/UX polish.

---

## 🔍 Web Search Source Cards
- **Favicon Source Cards:** After a web search, compact clickable source cards now appear above the AI response bubble — each showing the website's favicon, domain name, and a tooltip with the page snippet. Clicking a card opens the source in the browser.
- **Frontend-only change:** Parsed directly from existing `AgentProgress` `ToolEnd` events — no backend changes required.
- The AI response also includes a plain-text `Sources:` section listing title and URL for each result used.

---


## 🚀 Key Features & Enhancements

### 🛑 1. Escape (Esc) Key & Stop Button to Cancel AI Agent / Chat
You can now cancel or stop an active AI stream or agent loop at any point in the web and desktop applications:
- **Escape Key Press:** Pressing the `Esc` key while the AI is thinking or executing automatically aborts the active stream.
- **Red Stop Composer Button:** When the AI is running, the composer "Send" button turns into a red "Stop" square icon button, allowing users to stop the generation instantly.
- **Thinking Label Hints:** Updated the progress indicator label to explicitly display "Thinking for Xs (Esc to cancel)" so users are aware of the shortcut.

### 🎙️ 2. Microphone Active Breathing Aura Glow
When the microphone/voice mode is active, the boundaries of the chat composer box and the voice status bar light up with a dynamic, breathing green aura, providing intuitive visual feedback.

### 🤖 3. Configurable Multi-Agent System & Collaboration Pipeline
Users can now configure and run multiple specialized AI agents (e.g. Planner, Coder, Reviewer) within the desktop and web settings panel, allocating custom models, providers, and instructions to each agent:
- **Global Toggle Switch:** Added a main "Enable Multi-Agent Collaboration" toggle switch at the top of the new "Multi-Agent (Beta)" settings tab to globally turn on/off the agent routing pipeline.
- **Provider-Specific Dropdowns:** Replaced the text input for model selection with provider-specific dropdown select lists matching the models available in General settings.
- **Disable All Button:** Added a "Disable All" button allowing users to disable all configured agents instantly in one click.
- **Real-Time Agent & Model Indicators:** The live thinking message bubble in the chat panel dynamically displays the active agent's name and model in real-time (e.g. `Planner (gemini-2.5-flash) is thinking...`).

### 🖥️ 4. CLI Suggestions Pagination & Ordering
Improved the autocomplete experience in the terminal:
- **Pagination (5 Items Max):** Autocomplete suggestions now display a maximum of 5 commands at a time with a page indicator (e.g., `Suggestions (1/4)`), preventing long list clutter.
- **Alphabetical Sorting:** Autocomplete suggestions are sorted alphabetically at source.
- **Simplified Exit Options:** Removed `/quit` from the autocomplete suggestions and help output (while retaining hidden execution support) to clean up the interface.
- **Real-Time Agent & Model Status:** The CLI live thinking status bar dynamically displays which agent and model is executing.

### 📋 5. Fix Clipboard Permission Popup on Text Paste (Ctrl+V)
- Fixed a bug where pasting text (or pressing `Ctrl + V`) would trigger a browser-level clipboard permission warning popup (asking to "Allow Paste").
- Removed the redundant global `keydown` Ctrl+V listener and the async `navigator.clipboard.read()` fallback on text paste events. The application now correctly relies on standard, synchronous `event.clipboardData` values during paste events, ensuring seamless pasting of both images and text without prompting the user.

### 📦 6. Codebase Refactoring & Shared Utilities
- **Shared UI & Progress Helpers:** Refactored duplicate helper functions and TypeScript interfaces from the desktop renderer (`src/renderer/src`) and web renderer (`src/renderer/src-web`) into the unified `shared` codebase.
- **Maintainability:** Standardized helper functions across both application targets to prevent future regressions.

### 🖥️ 7. Horizontal Scrolling for CLI Input Box
- **Arbitrary Input Length:** Removed the single-line input constraint (previously restricted to `term_width - 4`) in the CLI interactive prompt, allowing users to type or edit long prompt instructions.
- **Dynamic Sliding Window:** Implemented a horizontal scrolling viewport for the single-line prompt box. It dynamically slices and displays a subset of characters surrounding the cursor position to preserve console alignment and layout integrity.
- **Interactive Cursor Navigation:** Bounded the arrow keys (`Left` and `Right` navigation) to trigger full box redraws, enabling the visible text window to scroll back and forward dynamically as the cursor moves.

### 🖥️ 8. Browser Automation Visual Indicator (Green Aura)
- **Visual Feedback:** Injects a breathing green glowing border overlay around the viewport of any browser page under active AI control.
- **Non-Intrusive Design:** Built with `pointer-events: none` and a high z-index to ensure it sits on top of all page elements without blocking clicks, scrolling, or user interactions.

### 🎙️ 9. Client-Side Speech-to-Text Microphone Consolidation
- **Consolidated Flow**: Standardized the voice interface to client-side Speech-to-Text (`SpeechRecognition`), enabling compatibility with all AI models (Gemini, OpenAI, Claude, etc.) by transcribing voice to text before sending.
- **Accurate Speech End Detection**: Configured the Speech-to-Text engine to run in continuous mode (`continuous = true`) so it keeps listening continuously across normal speaking pauses. Added a custom silence timer that triggers sending only after a full 2 seconds of silence, and fixed a loop bug by triggering the timer only after the user starts speaking (preventing the mic from cycling on and off during initial silence).
- **TTS Language Auto-Detection**: Added automatic Thai character detection in both frontend system voice lookup (now matching language tags case-insensitively) and backend Google Translate TTS generation, resolving the issue where Thai responses were read aloud in English voices.
- **Codebase Simplification & Hook Refactoring**: Extracted all Speech-to-Text logic, Web Speech APIs, silence timers, and React states into a shared, reusable custom hook `useSpeechToText` in [speech.ts](file:///home/pheem49/vscode/Project/Mint-CLI/src/renderer/shared/utils/speech.ts). This cleaned up ~150 lines of duplicate code in both desktop and web `ChatPanel.tsx` components.
- **Native App Guidance**: Added a helper prompt to notify users if they attempt to record voice in a browser or WebView that doesn't natively support SpeechRecognition (such as Firefox, Safari, or the Tauri desktop application on Linux/Windows), guiding them to use Google Chrome or Microsoft Edge instead.

### 🖼️ 10. Saved Pictures Gallery Pagination & Caching
- **24-Picture Pagination**: Refactored the Saved Pictures library component to load exactly 24 images initially, replacing the buggy automatic timer loading with a clean, manual "Load More" button at the bottom of the grid.
- **Cache-Busting Image URLs**: Appended a dynamic timestamp query parameter (`?_t=...`) to the static API picture fetches and individual image URLs. This prevents the browser from loading stale lists or caching broken images, ensuring that clicking "Refresh" successfully resolves any missing or incomplete pictures.

### 🤖 11. AI Model Self-Awareness Context Injection
- **Dynamic Context Injection**: Updated the core prompt compilation layer to automatically append an `[Active Environment Context]` block to the system instructions. This supplies the active AI model name and provider to the AI model itself, allowing the AI to successfully know its own model configurations in real-time when queried by the user.

---

## 🛠️ Codebase Changes

### Tauri Backend (`src-tauri` & `crates/mint-core`)
- Register Tauri commands: `create_workspace_file`, `create_workspace_folder`, and `delete_workspace_item` in `src-tauri/src/lib.rs`.
- Add `location` field to `LearnedSkillDto` and populate it based on workspace, global config, or database sources in `src-tauri/src/lib.rs`.
- Update `workspace_root` path resolver in `src-tauri/src/lib.rs` to strip `src-tauri` from the active directory during dev mode.
- Standardize process suggestion monitor logic to run AI prompt translations in the background thread inside `src-tauri/src/workflows.rs`.
- Change `submit_tool_approval` signature and `ApprovalsState` pending channel type to support `ApprovalOutcome` in `src-tauri/src/lib.rs`.
- Add executable search helper `which` and GitKraken auto-detection hooks to `load_config_from` in `crates/mint-core/src/config.rs`.
- Support multiple source directory scanning and deduplication of learned skills in `crates/mint-core/src/skills.rs`.
- Introduce a global `ACTIVE_AGENTS` thread-safe hash map registry and `cancel_agent` API in `crates/mint-core/src/lib.rs` to track active tokio tasks.
- Register Tauri command `cancel_chat_message` in `src-tauri/src/lib.rs`.
- Implement `POST /api/cancel-chat` in `crates/mint-core/src/api_server.rs` to cancel running web agent tasks.
- Add `enable_agent_collaboration` config parameter (defaulting to `false`) in `crates/mint-core/src/config.rs`.
- Update `resolve_agent_config` in `crates/mint-core/src/orchestration.rs` to return a tuple containing the active agent name and model, and check `enable_agent_collaboration`.
- Expand `AgentProgress::Thinking` in `crates/mint-core/src/orchestration.rs` to optionally contain `agent_name` and `model_name`.
- Inject active AI provider and model name environment context inside `enrich_request` and the initial prompt observation inside `crates/mint-core/src/orchestration.rs` so that the model becomes fully self-aware of its configuration and execution metadata.
- Inject a green aura viewport border style and DOM overlay (`#mint-browser-aura`) into the active browser page before executing CDP automation actions in `crates/mint-core/src/browser.rs` to visually notify users of active AI control.


### Desktop/Web Frontend (`src/renderer`)
- Integrate creation/deletion actions and focus/polling lifecycle listeners inside `WorkspacePanel.tsx`.
- Redesign actions bar grid layouts inside `styles.css`.
- Update inline markdown parser inside `ChatPanel.tsx` to wrap `@` mentions, and style `.chat-mention` in `index.css`.
- Update `submitToolApproval` API and component state handlers (`MintDashboard.tsx`, `ChatPanel.tsx`) in both desktop and web directories to render and submit custom answers.
- Update `activitiesFrom` and `AgentActivity` structure to parse and append the user's answer into the active `ask_user` tool target block upon `ToolEnd`.
- Update `LearnedSkill` TypeScript interface in `src/renderer/src/tauri.ts` to include optional `location`.
- Render colored location badges, strip lengthy content text boxes, and unify MCP toggles/delete controls in `src/renderer/src/components/Settings/PluginsTab.tsx` and `src/renderer/src-web/components/Settings/PluginsTab.tsx`.
- Implement `cancelChatMessage` API helpers in `tauri.ts` for desktop and web.
- Bind global Window keydown listeners for `Escape` key and dynamic "Stop" buttons in `ChatPanel.tsx` and `MintDashboard.tsx` under desktop and web source directories.
- Enhance microphone connection/permission alert dialogs in `ChatPanel.tsx` to print the exact Web API error message for easier debugging.
- Add voice-active class state and breathing keyframe animation effects around the chat input boundaries when microphone mode is active.
- Refine the voice status bar rendering logic to prevent displaying redundant text (such as "Listening Listening...") and display actual user speech transcripts wrapped inside quotes instead.
- Create new settings tab `AgentsTab.tsx` in desktop and web source directories supporting CRUD operations, provider-specific model dropdown selects, "Disable All" button, and global collaboration toggle.
- Register the `Multi-Agent (Beta)` tab in the sidebar of `SettingsWindow.tsx`.
- Update `ChatPanel.tsx` in desktop and web source directories to render the active agent and model names inside the live thinking status message.
- Remove duplicate "Enable Multi-Agent Review" checkbox from `AutomationTab.tsx`.
- Remove redundant `handleWindowKeyDown` Ctrl+V listeners, unused `onReadClipboardImage` prop, and `navigator.clipboard.read()` async fallback routines from `ChatPanel.tsx` and `MintDashboard.tsx` in both desktop and web directories to resolve browser clipboard permission warning popups during text pasting.
- Refactor duplicated types (`DiffHunk`, `FileChange`), helper functions (`numericSetting`, `errorMessage`, `readImage`, `readDocument`, `createTrimmedImagePreview`, `lightenColor`, `hexToRgb`, `applyThemeStyles`), and progress parsers (`parseFileChangesFromProgress`) from the desktop and web components to a central `src/renderer/shared` repository. Corrected a type checking issue by exporting `AgentProgress` and `InteractionMemory` from `shared/agentProgress.ts`.
- Consolidate microphone interface to client-side Speech-to-Text (`SpeechRecognition`), extract STT states, silence-detection timers, and event listeners into a reusable React hook `useSpeechToText` in [speech.ts](file:///home/pheem49/vscode/Project/Mint-CLI/src/renderer/shared/utils/speech.ts), and clean up both desktop (`src/renderer/src/components/ChatPanel.tsx`) and web (`src/renderer/src-web/components/ChatPanel.tsx`) `ChatPanel` components.
- Implement 24-picture pagination, a manual "Load More" button, and API/image fetch cache-busters in `PicturesLibrary.tsx` and `tauri.ts` across desktop and web directories.

### CLI Agent (`crates/mint-cli`)
- Redefine live status print lines (`plan_lines`, `tasks_lines`, `activities_lines`, `explored_lines`) to accept progress tick state and apply the `get_bullet` helper.
- Update `render_live_status` in `crates/mint-cli/src/agent.rs` to compute true physical lines of terminal wrapped text, using a new `is_thai_combining` filter.
- Modify `confirm` in `crates/mint-cli/src/main.rs` to indent confirmation prompts.
- Format `mint learn --list` and `/learn` CLI output into a bullet-point summary showing active skill locations and source paths.
- Register `/multi-agent` slash command to display configured agents and toggle collaboration status via `/multi-agent on` and `/multi-agent off` in `crates/mint-cli/src/main.rs`.
- Reorder `AUTOCOMPLETE_COMMANDS` alphabetically and simplify exit suggestions by keeping only `/exit`.
- Paginate suggestions to display at most 5 items per page with page numbers.
- Fixed `AgentProgress::Thinking` pattern matching compiler error (E0027) in `crates/mint-cli/src/agent.rs` and render active agent and model names in the live thinking terminal status bar.
- Implement horizontal scrolling viewport for the CLI prompt input box in `crates/mint-cli/src/main.rs`, slicing the visible text to fit the terminal screen and adjusting the cursor column offset accordingly, allowing arbitrary length instructions.
- Save `provider_change` system interaction events to the database and render a beautiful styled horizontal divider in the console when switching models via `/models` inside `crates/mint-cli/src/main.rs`.


### Github CI/CD Workflows
- Modify `Publish GitHub release` step in `.github/workflows/release.yml` to use `body_path: Release_Note.md`.
