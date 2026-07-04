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

---

## 🛠️ Codebase Changes

### Tauri Backend (`src-tauri` & `crates/mint-core`)
- Register Tauri commands: `create_workspace_file`, `create_workspace_folder`, and `delete_workspace_item` in `src-tauri/src/lib.rs`.
- Standardize process suggestion monitor logic to run AI prompt translations in the background thread inside `src-tauri/src/workflows.rs`.

### Desktop/Web Frontend (`src/renderer`)
- Integrate creation/deletion actions and focus/polling lifecycle listeners inside `WorkspacePanel.tsx`.
- Redesign actions bar grid layouts inside `styles.css`.
- Update inline markdown parser inside `ChatPanel.tsx` to wrap `@` mentions, and style `.chat-mention` in `index.css`.
