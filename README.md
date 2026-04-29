# Mint

<p align="center">
  <img src="assets/icon.png" alt="Luna Mint Icon" width="160">
</p>

<p align="center">
  <strong>An advanced AI Assistant built for the modern workflow — now on Desktop & Terminal.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/badge/Node.js-LTS-green?style=for-the-badge&logo=node.js" alt="Node.js">
  <img src="https://img.shields.io/badge/Electron-Latest-47848F?style=for-the-badge&logo=electron" alt="Electron">
  <img src="https://img.shields.io/badge/Powered%20By-Gemini-orange?style=for-the-badge&logo=google-gemini" alt="Gemini">
</p>

---

**Mint** is a powerful AI Assistant built with **Electron**, **Node.js**, and **Google Gemini**. It bridges the gap between your Desktop and Terminal, featuring real-time screen vision, web automation, local knowledge search, and a professional CLI for developers who love the command line.

## 🌟 Highlights

- **Dual-Mode AI**: Switch between a beautiful **Desktop GUI** and a professional **CLI**.
- **Code Agent Mode**: Use `mint code "<task>"` to inspect, edit, and verify a project directly from the current terminal workspace.
- **Chat-First Workflow**: Regular Mint chat can now auto-route coding requests into workspace Code Mode, so the main chat acts as the control surface.
- **Visible Mode State**: The CLI status bar now shows whether Mint is currently in `Chat` or `Code` mode.
- **Interactive Slash Commands**: Manage models and settings in the terminal with `/model`, `/config`, `/clear`, etc.
- **Smart TUI Experience**: Professional message framing, character-wrapped Thai text support, and mouse scroll wheel navigation.
- **System Information Action**: Retrieve OS, Kernel, and Architecture details via natural language.
- **Dynamic UI Aesthetics**: Animated **Aura Glow** for the AI widget and **Glassmorphism** design.
- **Minimize-to-Tray**: Keep Mint running in the background via the System Tray.
- **Vision-Ready (Desktop)**: Capture, analyze, and translate any part of your screen in real-time.
- **Automation First**: Control your system and browser via natural language from both App and Terminal.
- **Background Agent**: Install Mint as a `systemd` user service for proactive monitoring.

---

## 📸 Screenshots

<p align="center">
  <img src="assets/Agent_Mint.png" alt="Agent Mint UI" width="48%">
  <img src="assets/Settings.png" alt="Settings Window" width="48%">
</p>
<p align="center">
  <em>Desktop Interface and personalized Settings window</em>
</p>

---

## ⌨️ CLI Usage (Power Users)

Mint CLI is built for speed and efficiency. Use the **`mint`** command from anywhere.

<p align="center">
  <img src="assets/CLI_Screen.png" alt="Mint CLI Preview" width="100%">
</p>

### 🚀 Professional Commands
- **`mint`** : Start interactive chat mode (Default).
- **`mint code "task"`** : Run Mint as a workspace-aware coding agent in the current project folder.
- **`mint agent`** : Run Mint as a headless background agent (Monitoring mode).
- **`mint agent "task"`** : **[NEW]** Start agent and execute an autonomous task immediately.
- **`mint task "task"`** : Delegate a multi-step task to an already running background agent.
- **`mint onboard`** : Setup API Key, Model, and initialize global configuration in `~/.mint/`.
- **`mint list`** : See full list of automation actions and plugins.

---

### 🤖 Autonomous Agent (Task Delegate)
Mint isn't just a chatbot—it's an autonomous worker. Assign complex tasks that require multiple steps of reasoning.

### 👨‍💻 Code Agent Mode
Mint now includes a dedicated coding workflow as the first step toward a Claude-Code-like experience in the terminal.

**What it can do now:**
- Inspect the current workspace before editing
- Search code across the repo
- Read targeted file ranges
- Run non-destructive shell commands in the project with interactive approval
- Apply patch-based file edits with approval before changes are written
- Reuse lightweight session memory and git/test context per project

**Usage Example:**
```bash
mint code "fix the failing CLI command and verify it"
```

**Chat-first Example:**
```bash
mint
> สำรวจโปรเจคนี้ให้หน่อย
```

Mint will classify the request, switch the status bar to `Code`, and return to `Chat` mode after the coding flow finishes.

**Supported Autonomous Tools:**
- 🌐 **Web Automation**: Full Puppeteer-based browsing, info extraction, and research.
- 📁 **File System**: Create, Write, Delete, and Manage folders using `~/` path expansion.
- 🔍 **Knowledge Search**: Query local files and documentation (RAG).
- 🛡️ **Safety Mode (Bash)**: Mint proposes commands via notifications; you choose whether to run them.

**Usage Examples:**
```bash
# Research and write a report
mint agent 'Search for the latest iPhone reviews and write a SUMMARY.md to my desktop.'

# Background Task
mint task 'Process these 5 files and move them to ~/Documents/Archive'
```

---

### ⚡ Slash Commands (Interactive Chat)
While in terminal chat, type **`/`** to access advanced tools with the new visual autocomplete system.
> [!TIP]
> Use **Up/Down arrow keys** to navigate the suggestion list and press **Enter** to select!

- `/help` : View all commands and descriptions.
- `/models` : List and switch between Gemini/Ollama models.
- `/config` : Check your active API keys and preferences.
- `/clear` / `/reset` : Clean terminal or reset AI context.

---

### 🕒 Proactive Monitoring
When running in `agent` mode, Mint monitors your system in the background:
- 🔋 **Battery Alerts**: Notifies you when power is low or charging status changes.
- 🌐 **Network Status**: Alerts you when connection status changes.
- 📊 **Resource Usage**: Proactive tips if system load is too high (System Metrics).


---

## 🎨 Desktop GUI Features

- **Floating Widget**: A persistent AI character on your desktop.
- **Animated Aura**: The widget breathes and glows when Mint is thinking or proactive.
- **Minimize to Tray**: Click the dash icon (-) to hide the window to your system tray. Mint stays active!
- **Widget Toggle**: Enable or disable the desktop widget anytime from **Settings > General**.
- **Screen Overlay Glow**: Visual feedback when Mint is capturing your screen for analysis.

---

## 🛡️ Security & Privacy

- **Push Protection**: Automated `.gitignore` patterns for `mint-config.json` and `.env` files.
- **History Scrubbing**: Integrated tools to ensure API keys are never leaked to Git history.
- **Local First**: Built-in **Ollama** support for 100% private, offline AI processing.

---

## 🛠️ Getting Started

### Prerequisites
- [Node.js](https://nodejs.org/) (LTS)
- [npm](https://www.npmjs.com/)
- A **Google Gemini API Key** (Get one at [Google AI Studio](https://aistudio.google.com/))

### Installation
1. **Install via NPM (Recommended)**
   ```bash
   npm install -g @pheem49/mint@latest
   ```

2. **Manual Installation (For Developers)**
   ```bash
   git clone https://github.com/Pheem49/Mint.git
   cd Mint
   npm install
   sudo npm link
   ```

---

## 📂 Project Structure

```text
Mint/
├── src/
│   ├── AI_Brain/        # Gemini/Ollama integration & logic
│   ├── Automation/      # Puppeteer and browser scripts
│   ├── CLI/             # CLI Onboarding and Feature list
│   ├── Plugins/         # Spotify, Docker, Obsidian, Git, etc.
│   ├── System/          # Config, Daemon, and Event Monitoring
│   └── UI/              # Electron renderer (Glassmorphism & Aura)
├── mint-cli.js          # CLI Main Entry
├── main.js              # Electron Main process
└── package.json         # Binaries and dependencies
```

---

## 🏛️ License

Distributed under the **GNU Affero General Public License v3.0**. See `LICENSE` for details.
