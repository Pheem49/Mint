# Release Notes - Mint Agent v1.8.1

We are excited to release **Mint Agent v1.8.1**! This release introduces powerful local workspace file management features, real-time AI-generated workflow suggestions in the user's active language, mobile layout optimization, and general UI/UX polish.

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

---

## 🛠️ Codebase Changes

### Tauri Backend (`src-tauri` & `crates/mint-core`)
- Register Tauri commands: `create_workspace_file`, `create_workspace_folder`, and `delete_workspace_item` in `src-tauri/src/lib.rs`.
- Standardize process suggestion monitor logic to run AI prompt translations in the background thread inside `src-tauri/src/workflows.rs`.
- Change `submit_tool_approval` signature and `ApprovalsState` pending channel type to support `ApprovalOutcome` in `src-tauri/src/lib.rs`.

### Desktop/Web Frontend (`src/renderer`)
- Integrate creation/deletion actions and focus/polling lifecycle listeners inside `WorkspacePanel.tsx`.
- Redesign actions bar grid layouts inside `styles.css`.
- Update inline markdown parser inside `ChatPanel.tsx` to wrap `@` mentions, and style `.chat-mention` in `index.css`.
- Update `submitToolApproval` API and component state handlers (`MintDashboard.tsx`, `ChatPanel.tsx`) in both desktop and web directories to render and submit custom answers.
- Update `activitiesFrom` and `AgentActivity` structure to parse and append the user's answer into the active `ask_user` tool target block upon `ToolEnd`.

### CLI Agent (`crates/mint-cli`)
- Redefine live status print lines (`plan_lines`, `tasks_lines`, `activities_lines`, `explored_lines`) to accept progress tick state and apply the `get_bullet` helper.
- Update `render_live_status` in `crates/mint-cli/src/agent.rs` to compute true physical lines of terminal wrapped text, using a new `is_thai_combining` filter.
- Modify `confirm` in `crates/mint-cli/src/main.rs` to indent confirmation prompts.
