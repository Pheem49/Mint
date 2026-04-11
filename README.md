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

## Highlights

- **Dual-Mode AI**: Switch between a beautiful **Desktop GUI** and a professional **CLI**.
- **Persona-Driven**: Features **Mint**, a smart and highly helpful assistant.
- **Natural Replies**: Longer AI responses are split into multiple chat bubbles/lines with human-like pacing.
- **Vision-Ready (Desktop)**: Capture and translate any part of your screen in real-time.
- **Automation First**: Control your system and browser via natural language from both App and Terminal.
- **Professional CLI**: Onboarding wizard, subcommands, and background daemon support.
- **Background Agent**: Install Mint as a `systemd` user service to run tasks in the background.

---

## Screenshots

<p align="center">
  <img src="assets/Agent_Mint.png" alt="Agent Mint UI" width="48%">
  <img src="assets/Settings.png" alt="Settings Window" width="48%">
</p>
<p align="center">
  <em>Desktop Interface and Personalized Settings window</em>
</p>

---

## Getting Started

### Prerequisites
- [Node.js](https://nodejs.org/) (LTS recommended)
- [npm](https://www.npmjs.com/)
- A **Google Gemini API Key** (Get one at [Google AI Studio](https://aistudio.google.com/))

### 1. Installation

1. **Clone and Install Dependencies**
   ```bash
   git clone https://github.com/Pheem49/Mint.git
   cd Mint
   npm install
   ```

2. **Setup CLI Globally (Recommended)**
   ```bash
   sudo env "PATH=$PATH" npm link
   ```

### 2. Configuration

Mint will guide you through the setup automatically when you first run it, or you can manually create a `.env` file:
```env
GEMINI_API_KEY=your_google_gemini_api_key
```

---

## Desktop Usage

Launch the full Electron experience with a floating character widget and screen vision:
```bash
npm start
```
**Pro Tip:** Use `Ctrl+Shift+Space` as the default global shortcut to summon Mint Desktop instantly!

---

## CLI Usage

Mint CLI is built for power users. Use the **`mint`** command from anywhere in your terminal.

### First-Time Setup
Run the onboarding wizard to configure your API key and model:
```bash
mint onboard
```

### Install Background Daemon (Linux Only)
Keep Mint running in the background to handle proactive features:
```bash
mint onboard --install-daemon
```

<p align="center">
  <img src="assets/CLI_Screen.png" alt="Mint CLI Preview" width="100%">
</p>

### Direct Commands
Ask Mint something directly without entering interactive mode:
```bash
mint "What are my upcoming calendar events?"
mint "Open YouTube and search for Lo-fi music"
```

### Interactive Chat & Features
Start a conversation or see all available abilities:
```bash
mint        # Start chat
mint list   # Show all features and plugins
```

---

## Key Features

### Intelligent Core
- **AI Persona**: A professional and friendly assistant with a distinct helpful personality.
- **Multi-Bubble Responses**: AI replies appear in multiple lines for better readability.
- **Extended Local Knowledge (RAG)**: Index and search `.pdf`, `.docx`, `.md`, and live **Website URLs**.

### Screen Vision & Automation
- **Live Translate (Desktop)**: Drag a box over any window to translate text dynamically.
- **System Control**: Open apps, create folders, and control system volume/brightness via chat.
- **Web Automation**: Puppeteer-driven browser control for searching or extracting data.

### Plugin System
Mint is highly modular. Manage these integrations from the Settings panel or CLI:
- **Spotify**, **Docker**, **Obsidian**, **Google Calendar**, **DevTools (Git)**, and more.

---

## Project Structure

```text
Mint/
├── src/
│   ├── AI_Brain/        # Gemini integration & Headless Agent
│   ├── Automation/      # Puppeteer and browser scripts
│   ├── CLI/             # CLI Onboarding and Command logic
│   ├── Plugins/         # Spotify, Docker, etc.
│   ├── System/          # Config, Daemon Management, and State
│   └── UI/              # Electron renderer and styles
├── mint-cli.js          # CLI Entry point
├── main.js              # Electron Main process
└── package.json         # Project metadata & Binaries
```

---

## License

Distributed under the **GNU Affero General Public License v3.0**. See `LICENSE` for more information.
