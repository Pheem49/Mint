# Release Notes - Mint Agent v1.8.2

We are excited to release **Mint Agent v1.8.2**! This release introduces a configurable Multi-Agent system for sequential collaboration, powerful local workspace file management features, real-time AI-generated workflow suggestions in the user's active language, mobile layout optimization, and general UI/UX polish.

---

## 🔍 Web Search Source Cards
- **Favicon Source Cards:** After a web search, compact clickable source cards now appear above the AI response bubble — each showing the website's favicon, domain name, and a tooltip with the page snippet. Clicking a card opens the source in the browser.
- **Frontend-only change:** Parsed directly from existing `AgentProgress` `ToolEnd` events — no backend changes required.
- The AI response also includes a plain-text `Sources:` section listing title and URL for each result used.

---


## 🚀 Key Features & Enhancements

### 📂 1. Native Workspace File Explorer Operations

You can now manage your workspace directory directly from the sidebar without switching to an external editor or using the terminal:
- **New File & New Folder Buttons:** Replaced the legacy "Use Agent" action button with dedicated creation triggers. Inputting a name will immediately create the empty file/folder on disk.
- **Right-Click Deletion:** Right-clicking on any file or directory in the tree view prompts a safety confirmation modal. Upon approval, the item is securely removed from your filesystem.
- **Workspace Auto-Sync:** The file list stays up-to-date automatically using two smart mechanisms:
  - **Window Focus Reloading:** Whenever you switch back to the Mint Desktop window (e.g., after editing code in VS Code), it refreshes the file list immediately.
  - **Periodic Polling:** Auto-polls the directory structure every 15 seconds.

### 💬 2. Drag-and-Drop Mentions with Accent Color Highlight
Referencing files in your conversations is now smoother and visually richer:
- **Trailing Spacing:** Dragging files from the sidebar into the chat input automatically appends spacing (e.g. `@index.html `), allowing you to keep typing immediately.
- **Accent Theme Highlight:** In your chat history, any `@filename` mention is parsed and rendered inside an outline pill container styled dynamically with your selected **Accent Color** theme.

### 🌐 3. Dynamic Language AI Suggestions
- Process monitor triggers (such as opening terminal processes) now format real-time queries to your active AI provider.
- Suggestions are dynamically translated and rewritten to match your configured user language (`config.language`), replacing the Thai-only hardcoded templates.
- Features a safe fallback to static trigger defaults if the AI request times out or encounters network errors.

### 📱 4. Mobile Layout & Navigation Optimizations (Web View)
- **Image Studio Mobile Fit:** Fixed position clashing media queries resetting Image Studio `.img-studio` offsets to fit smaller screen sizes correctly.
- **Hamburger Menu Visibility:** Rendered the sidebar drawer button (☰) inside the Image Studio header on mobile viewports so users can navigate back.
- **Header Overlaps Resolved:** Added a solid opaque background overlay, glass blur, and borders to the mobile chat header, preventing scrolled messages from clashing with header typography.
- **Mobile Sidebar Drawer Fix:** Prevented the sidebar from collapsing into desktop icon-only configurations on screen widths under 760px, resolving layout drawer compression bugs.

### 💅 5. General UX Polish
- **Widescreen Chat Layout:** Expanded the chat conversation box max-width to `1100px` when the assistant avatar model is hidden, optimizing readability on widescreen displays.
- **Global Text Selection:** Unlocked full text selection and copying across the entire application interface (sidebars, panels, buttons, headers) by overriding restricted user-select rules.

### 🖥️ 6. CLI Agent Interface Polish & Terminal Line Wrapping Fix
Improved CLI status rendering aesthetics, interactive pacing, and terminal output reliability:
- **Consistent Terminal Padding:** Indented status blocks (Plan, Tasks, Activity, Explored) and confirmation prompt headers by 2 spaces to align with the standard prompt margin and prevent text from sticking to the left edge of the terminal window.
- **Dynamic Breathing Circle System:** Consolidated status symbols into a unified, colored circle layout (`● plan`, `● tasks`, `● activity`, `● explored`) which pulses dynamically between solid and hollow (`●` ⇄ `○`) while the AI is thinking to signify active processing.
- **Wrapping & Thai Combining Characters Fix:** Solved the duplicate/orphaned terminal lines issue caused by text wrapping on narrow screens. It now strips ANSI sequences, filters out Thai combining characters/tone marks that stack vertically, and counts true physical display lines to correctly clear stdout between updates.

### 💬 7. AskUser Text Input Modal in Desktop & Web App
Brought parity to the interactive questioning feature between the CLI and GUI versions:
- **Dynamic Input Field:** When the agent runs the `ask_user` tool (asking questions like requesting answers or inputs), a sleek text input textarea is displayed inside the Desktop and Web application's approval card.
- **Answer Submission Backend:** Updated the Tauri backend to handle `ApprovalOutcome::Intercepted(answer)` channels, sending user-typed answers back to the orchestration layer.
- **Activity Log Integration:** Once submitted, the user's typed answer is appended directly to the tool's execution target within the expandable "Working through task" drawer (e.g. `คุณภีมชอบสีอะไรคะ? (Answered: "สีเขียวมิ้นต์")`), maintaining a clear visual history of interactions.

### 🐙 8. GitKraken MCP Auto-Detection
Enabled a seamless developer onboarding experience by auto-discovering GitKraken on the host machine:
- **Automatic Discovery:** Scans the user's local PATH during configuration load to check if `gk` (GitKraken CLI) is available on the system.
- **Auto-Config Injection:** If detected, automatically configures and adds the GitKraken MCP server (`gk mcp`) to the user's `mint-config.json` file.
- **Github Release Integration:** Updated the CI/CD `.github/workflows/release.yml` file to parse and use `Release_Note.md` directly for the body description of GitHub Releases.

### 🏷️ 9. File-System Based AI Skills & Source Badges
Upgraded the AI skills loading pipeline and settings interface:
- **Dual Workspace Scanning:** Automatically discovers and loads `.md` / `.txt` skills from both `.agents/skills/` and `skills/` folders in the active workspace.
- **Location Color Badges:** Displays tags indicating whether skills come from Workspace (🟢 green), Global (🔵 blue), or Taught database (🟣 purple) in settings UI and CLI list output (`mint learn --list` and `/learn`).
- **Clean List UI:** Hides the lengthy file content preview block under each skill, rendering a much cleaner and concise layout.

### 🛠️ 10. Unified MCP Servers & Toggle Management
- **Consolidated List:** Merged all system-discovered tools (Docker, Git, GitHub, Node) into the unified MCP Servers list.
- **Individual Toggle Switches:** Enables/disables or installs MCP servers dynamically with instant toggle state styling.
- **Red Trash Can Buttons:** Added a red trash icon button to easily remove custom/manually configured MCP servers.

### 🛑 11. Escape (Esc) Key & Stop Button to Cancel AI Agent / Chat
You can now cancel or stop an active AI stream or agent loop at any point in the web and desktop applications:
- **Escape Key Press:** Pressing the `Esc` key while the AI is thinking or executing automatically aborts the active stream.
- **Red Stop Composer Button:** When the AI is running, the composer "Send" button turns into a red "Stop" square icon button, allowing users to stop the generation instantly.
- **Thinking Label Hints:** Updated the progress indicator label to explicitly display "Thinking for Xs (Esc to cancel)" so users are aware of the shortcut.

### 🎙️ 12. Microphone Active Breathing Aura Glow
When the microphone/voice mode is active, the boundaries of the chat composer box and the voice status bar light up with a dynamic, breathing green aura, providing intuitive visual feedback.

### 🤖 13. Configurable Multi-Agent System & Collaboration Pipeline
Users can now configure and run multiple specialized AI agents (e.g. Planner, Coder, Reviewer) within the desktop and web settings panel, allocating custom models, providers, and instructions to each agent:
- **Global Toggle Switch:** Added a main "Enable Multi-Agent Collaboration" toggle switch at the top of the new "Multi-Agent (Beta)" settings tab to globally turn on/off the agent routing pipeline.
- **Provider-Specific Dropdowns:** Replaced the text input for model selection with provider-specific dropdown select lists matching the models available in General settings.
- **Disable All Button:** Added a "Disable All" button allowing users to disable all configured agents instantly in one click.
- **Real-Time Agent & Model Indicators:** The live thinking message bubble in the chat panel dynamically displays the active agent's name and model in real-time (e.g. `Planner (gemini-2.5-flash) is thinking...`).

### 🖥️ 14. CLI Suggestions Pagination & Ordering
Improved the autocomplete experience in the terminal:
- **Pagination (5 Items Max):** Autocomplete suggestions now display a maximum of 5 commands at a time with a page indicator (e.g., `Suggestions (1/4)`), preventing long list clutter.
- **Alphabetical Sorting:** Autocomplete suggestions are sorted alphabetically at source.
- **Simplified Exit Options:** Removed `/quit` from the autocomplete suggestions and help output (while retaining hidden execution support) to clean up the interface.
- **Real-Time Agent & Model Status:** The CLI live thinking status bar dynamically displays which agent and model is executing.

### 📋 15. Fix Clipboard Permission Popup on Text Paste (Ctrl+V)
- Fixed a bug where pasting text (or pressing `Ctrl + V`) would trigger a browser-level clipboard permission warning popup (asking to "Allow Paste").
- Removed the redundant global `keydown` Ctrl+V listener and the async `navigator.clipboard.read()` fallback on text paste events. The application now correctly relies on standard, synchronous `event.clipboardData` values during paste events, ensuring seamless pasting of both images and text without prompting the user.

### 📦 16. Codebase Refactoring & Shared Utilities
- **Shared UI & Progress Helpers:** Refactored duplicate helper functions and TypeScript interfaces from the desktop renderer (`src/renderer/src`) and web renderer (`src/renderer/src-web`) into the unified `shared` codebase.
- **Maintainability:** Standardized helper functions across both application targets to prevent future regressions.

### 🖥️ 17. Horizontal Scrolling for CLI Input Box
- **Arbitrary Input Length:** Removed the single-line input constraint (previously restricted to `term_width - 4`) in the CLI interactive prompt, allowing users to type or edit long prompt instructions.
- **Dynamic Sliding Window:** Implemented a horizontal scrolling viewport for the single-line prompt box. It dynamically slices and displays a subset of characters surrounding the cursor position to preserve console alignment and layout integrity.
- **Interactive Cursor Navigation:** Bounded the arrow keys (`Left` and `Right` navigation) to trigger full box redraws, enabling the visible text window to scroll back and forward dynamically as the cursor moves.

### 🖥️ 18. Browser Automation Visual Indicator (Green Aura)
- **Visual Feedback:** Injects a breathing green glowing border overlay around the viewport of any browser page under active AI control.
- **Non-Intrusive Design:** Built with `pointer-events: none` and a high z-index to ensure it sits on top of all page elements without blocking clicks, scrolling, or user interactions.

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


### Github CI/CD Workflows
- Modify `Publish GitHub release` step in `.github/workflows/release.yml` to use `body_path: Release_Note.md`.
