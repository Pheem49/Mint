# Release Notes - Mint Agent v1.9.1

We are excited to release **Mint Agent v1.9.1**! This version introduces major enhancements to the Command Line Interface (CLI) user experience, featuring rich terminal loaders, animated status wave highlights, code modularization, and workflow automation polishes.

---

## 🚀 Key Features & Enhancements

### 🖥️ 1. Rich Terminal Spinner Integration (`indicatif`)
- Added steady-tick green spinner loaders to keep the CLI interactive and visually responsive during blocking background operations:
  - Repository metadata fetching and AI analysis (`GithubOverview`).
  - Codebase indexing & syntax tree parsing (`Command::Symbols`).
  - Semantic vector database embedding indexing (`SemanticCodeCommand::Index`).
  - AI image generation (`Command::Imagine`).
  - Wait for OAuth browser redirection flow (`GmailCommand::Auth`).
  - Application updates check and NPM dependency installations (`updater`).

### 🤖 2. Dynamic Moon Walk Thinking Loader
- Replaced standard loaders with an elegant Moon phase vector animation loop (`🌑`, `🌒`, `🌓`, `🌔`, `🌕`, `🌖`, `🌗`, `🌘`) forced into a clean text-presentation style and glowing in a mint-green color.
- Implemented trailing dynamic dot padding (`""` -> `"."` -> `".."` -> `"..."` -> `""`) to keep line length constant and completely prevent terminal jitter.

### 🎨 3. Glowing Bold Wave Text Scanner
- Added a floating light wave effect (`apply_wave_effect`) that slides color gradients (Cyan `BLUE`, Mint `MINT`, and Gray `DIM`) dynamically across the letters of the thinking text from left to right.
- Changed the font weight to bold (`\x1b[1m`) and characters to full-width (`Ｔｈｉｎｋｉｎｇ` / `ｉｓ  ｔｈｉｎｋｉｎｇ`) to match CJK character scaling, making the text physically larger and highly prominent in the console.

### 📦 4. Codebase Modularization & Refactoring
- Split `main.rs` into modular helper components (`markdown.rs`, `actions.rs`, `interactive.rs`) to improve structure and readability.
- Cleaned up unused compiler warning imports and standardized process exit codes.

### 🧪 5. Automated CI/CD Workflow Releases
- Reconfigured the GitHub Actions compiler pipeline (`release.yml`) to automatically parse and publish this `Release_Note.md` directly as the release description body.

---

## 🛠️ Codebase Changes Summary
- **Tauri Backend**: Refactored learned skill directory resolver, process suggestion backgrounds, and added global active task cancel hooks (`ACTIVE_AGENTS`).
- **Web/Desktop Frontend**: Refactored component layout structures, consolidated client-side Speech-to-Text hooks, integrated Material Icon SVGs for agent file explorations, and cleaned up clipboard paste warnings.
- **CLI Agent**:
  - Reorganized autocomplete commands alphabetically and paginated console inputs to 5 commands max per page.
  - Added dynamic skill prompts (`$`).
  - Added a Crossterm-based interactive arrow-key selection menu (themed in active Blue/Cyan highlight) to `/models`, `/image-provider`, `/fast`, `/multi-agent`, and `/clear` commands.
  - Implemented a custom 24-bit Truecolor Mint-to-Blue gradient renderer for the ASCII welcome banner logo.
  - Upgraded the "Thinking" status loader with a smooth 24-bit Truecolor sine-wave gradient wave animation, and restored standard English characters for cleaner rendering.
  - Improved the MCP safety policy error with direct instructions to run `/mcp allow` to authorize blocked tools.
  - Implemented background `stderr` and `stdout` monitoring for MCP servers to automatically detect OAuth URLs, increased default tool timeout to 30 seconds for slow `npx` resolution, and launch the default system browser with extended timeouts.
  - Injected the list of available MCP servers directly into the system prompt's `mcp_tool` description in `orchestration.rs` to prevent agent naming hallucinations.
  - Implemented the `mcp_list_tools` capability to allow the AI agent to query the complete list of registered tools from any configured MCP server dynamically, enabling self-discovery of tool APIs.
  - Upgraded `/mcp` command to use a two-step interactive menu (themed in active Blue/Cyan highlight) to select a configured server first, show its status, and authorize all tools on the spot.
- **Workspace Skills Loading**: Modified skill loading to only supply metadata (name, description, path) for workspace-relative skills in the initial context, forcing the AI agent to explicitly invoke `read_file` to read the skill files. This makes skill reading visible as tool call logs in the user interface. Added chat history check to mark skills as READ on subsequent turns, preventing redundant reads on every turn.
