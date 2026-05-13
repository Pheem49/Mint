# Mint

<p align="center">
  <img src="assets/icon.png" alt="Mint Icon" width="160">
</p>

<p align="center">
  <strong>The Unified AI Desktop Assistant & Agentic Coding CLI. Built for speed, power, and local control.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/badge/Node.js-LTS-green?style=for-the-badge&logo=node.js" alt="Node.js">
  <img src="https://img.shields.io/badge/Electron-40.x-47848F?style=for-the-badge&logo=electron" alt="Electron">
  <img src="https://img.shields.io/badge/CLI-Unified_Agent-orange?style=for-the-badge" alt="CLI Agentic">
</p>

Mint is an advanced AI assistant designed to live in your workspace. It features a rich Electron desktop interface for day-to-day assistance and a powerful, unified CLI agent that seamlessly merges conversational chat with complex coding tasks.

## 🌟 What's New in v1.4.1

- **Unified Agent Loop:** No more switching modes. Every interaction in the CLI is now handled by a sophisticated agent that can think, plan, and execute tools autonomously.
- **Agentic Web Search:** Mint can now search the internet in real-time to answer questions with the latest information using integrated web tools.
- **Upgraded TUI:** A redesigned terminal interface featuring a Braille spinner, thinking timer, and a cleaner "✓ ActionName" logging style.
- **Interactive "Ask User":** The agent can now pause and ask you for clarification or preferences during complex multi-step tasks.
- **Enhanced System Control:** Reliable multi-fallback support for opening files, folders, and applications on Linux (Pop!_OS/Ubuntu), macOS, and Windows.

## 🚀 Key Features

### 💻 Unified CLI Agent
Mint CLI is not just a chat wrapper; it's a full agentic workflow.
- **Think & Plan:** Every response starts with a reasoning phase where Mint plans its next move.
- **Autonomous Tools:** `web_search`, `list_files`, `read_file`, `search_code`, `run_shell`, `apply_patch`, `write_file`, `open_folder`, and more.
- **User-in-the-Loop:** Safety first. Mint asks for your approval before running shell commands or making file edits.
- **Workspace Aware:** Automatically understands your project structure, git status, and testing framework.

### 🖥️ Desktop Assistant
- **Screen Vision:** Capture and analyze your screen for instant help with what you're looking at.
- **Real-time Translation:** Instantly translate text from your screen into Thai or English.
- **Proactive Engine:** Mint monitors your system events to provide helpful suggestions before you even ask.
- **System Tray & Floating Widgets:** Quick access to Mint from anywhere on your desktop.

### 🛠️ Multi-Provider Support
Mint supports the latest LLMs and local backends:
- **Cloud:** Gemini 1.5/2.0 Pro & Flash, Anthropic Claude 3.5, OpenAI GPT-4o.
- **Local:** Ollama, LM Studio, Hugging Face Inference API.
- **MCP:** Full support for Model Context Protocol to extend Mint's capabilities with external tools.

## 📸 Screenshots

<p align="center">
  <img src="assets/Agent_Mint.png" alt="Mint Desktop UI" width="48%">
  <img src="assets/Settings.png" alt="Mint Settings" width="48%">
</p>

<p align="center">
  <img src="assets/CLI_Screen.png" alt="Mint CLI" width="100%">
</p>

## 📦 Installation

### Global Install
```bash
npm install -g @pheem49/mint@latest
```

### Local Development
```bash
git clone https://github.com/Pheem49/Mint.git
cd Mint
npm install
```

## ⚡ Quick Start

1. **Setup Mint:**
   ```bash
   mint onboard
   ```
2. **Start Chatting:**
   ```bash
   mint
   ```
3. **Run the Desktop App:**
   ```bash
   npm start
   ```

## ⌨️ CLI Commands

- `mint` / `mint chat` : Start the unified interactive agent UI.
- `mint code "<task>"` : Run a specific coding task in the current workspace.
- `mint task "<task>"` : Queue a background task for the headless agent.
- `mint mcp` : Manage Model Context Protocol (MCP) servers.
- `mint list` : Display all available features and commands.

## 🔌 MCP Management (Extensions)

Mint supports the **Model Context Protocol (MCP)**, allowing you to extend its capabilities via the CLI without manual config editing.

### Add a New Server
```bash
# Template
mint mcp add <name> <command> --args <args...> --env <KEY=VALUE>

# Example: Google Search
mint mcp add google-search npx --args -y @modelcontextprotocol/server-google-search --env GOOGLE_API_KEY=your_key GOOGLE_SEARCH_ENGINE_ID=your_id

# Example: Filesystem Access
mint mcp add my-files npx --args -y @modelcontextprotocol/server-filesystem /path/to/folder
```

### List Configured Servers
```bash
mint mcp list
```

### Remove a Server
```bash
mint mcp remove google-search
```

### Clear All Servers
```bash
mint mcp clear
```

## 🏗️ Project Structure

```text
Mint/
├── src/
│   ├── AI_Brain/          # Gemini API, Unified Agent Client, Memory Store
│   ├── Automation_Layer/  # File Ops, Browser Automation, System Control
│   ├── CLI/               # TUI logic, Chat Router, Code Agent
│   ├── Plugins/           # MCP Manager, System Monitor, Third-party integrations
│   ├── System/            # Config, Notifications, Daemons
│   └── UI/                # Electron Renderer, Settings, Widgets
├── mint-cli.js            # Main CLI entry point
└── package.json
```

## 🛡️ Security & Privacy
- **Local Control:** Mint prioritizes local execution and user privacy.
- **Approval System:** No destructive command or file change happens without your explicit `y/n` confirmation.
- **Secure Config:** API keys are stored locally on your machine and never transmitted outside of the chosen AI provider.

## 📜 License
Mint is licensed under the **GNU Affero General Public License v3.0**.
See the [LICENSE](LICENSE) file for more details.

---
<p align="center">Made with 💚 by <a href="https://github.com/Pheem49">Pheem49</a></p>
