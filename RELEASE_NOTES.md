# Mint Release Notes

## v1.5.5

This release introduces task cancellation support, visual UI updates, customizable Gemini API safety settings, validation libraries, and critical bug fixes.

### Task Cancellation & UI Visual Updates

- **Cancellation Support:** Implemented task cancellation via `AbortController` in `code_agent.js` and `interactive_chat.js`, allowing users to interrupt long-running operations.
- **UI Enhancements:** Redesigned interactive chat interface elements and updated the Desktop UI styles (`styles.css`) for a more responsive and modern user experience.

### Gemini API Integration

- **Unrestricted Interactions:** Configured `safetySettings` (set to `BLOCK_NONE`) across all categories for Gemini API requests.
- **System Instructions:** Updated default agent and chatbot instructions to handle mature, creative, and uncensored interactions without unwarranted refusals.

### Dependencies & Build Workflow

- **Data Validation:** Added the `zod` dependency for schema validation and parsing.
- **Dependency Overrides:** Added a package override for `gh-pages` to resolve build configuration conflicts.
- **CI Environments:** Enabled `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` in Github Actions CI to ensure compatibility with Node.js 24 runtime environments.

### Bug Fixes

- **Directory Walker Fallback:** Fixed the fallback directory-walking logic in `code_agent.js` when ripgrep is not installed or fails, ensuring it scans the correct `searchRoot` instead of defaulting to the full `workspaceRoot`.

## v1.5.4

This release introduces automated one-liner installation scripts for macOS, Linux, and Windows, replaces AppImage with `.tar.gz` for Linux packaging, fixes missing dependencies, and improves continuous integration workflows.

### One-Liner Automated Installers

We have introduced automated installation scripts to make setting up and updating Mint CLI easier than ever.

- **Linux & macOS (One-liner):**
  ```bash
  curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash
  ```
- **Windows (One-liner via PowerShell):**
  ```powershell
  powershell -Command "iwr -useb https://raw.githubusercontent.com/Pheem49/Mint/main/install.ps1 | iex"
  ```
- **Smart Permission Handling:** The Linux/macOS script automatically detects NVM environments and user write permissions, avoiding unnecessary `sudo` requests when installing the NPM package globally.

### Packaging & CI Improvements

- **Linux Packaging:** Replaced `.AppImage` with `.tar.gz` packaging for better compatibility across different Linux distributions.
- **CI Workflow Update:** Updated the GitHub Actions workflow to run `npm install` instead of `npm ci`, resolving dependency synchronization issues and ensuring smooth builds.

### Dependency & Bug Fixes

- **Missing Dependency:** Added `read-excel-file` to the package dependencies to fix a runtime module error when launching the `mint` command.
