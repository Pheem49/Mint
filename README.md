# Mint

Mint is an Electron desktop assistant powered by Google Gemini. It combines chat, screen capture, local automation, proactive suggestions, and a small plugin system in a single desktop app.

## Current Features

- AI chat backed by `@google/genai`
- Screen Vision for sending a selected screen region into chat
- Live Translate mode for continuously translating a selected on-screen area
- Smart Context / Proactive Assistant with periodic screen analysis
- Local knowledge indexing for `.txt` and `.md` files
- Clipboard read/write helpers
- Weather and system info commands
- Web automation via Puppeteer
- Voice input through the browser Speech Recognition API
- Tray integration and global shortcut
- Plugin loading for Spotify, Docker, and a placeholder Discord plugin

## Live Translate

The current screen picker supports two flows:

- Standard Screen Vision: capture a region once and send it back to the chat window
- Live Translate: drag a box over text on screen and keep translating that area continuously

Current behavior:

- After selecting a region, the screen returns to normal brightness
- The selected translation box outline remains visible
- Mouse clicks and middle-mouse scrolling pass through to the app underneath
- Press `Esc` to exit Live Translate

Reliability safeguards:

- Retry automatically on Gemini `502` and `503` errors
- Prevent overlapping translation requests
- Apply a `15s` cooldown after repeated retryable failures

## Settings

The Settings window currently exposes:

- Gemini API key
- Automation browser engine
- Theme
- Accent color
- Proactive capture interval
- Proactive suggestion cooldown

Config is stored in Electron user data as `mint-config.json`.

## Plugins

Plugins are loaded from `src/Plugins/`.

Current bundled plugins:

- `spotify`: controls playback using `playerctl`
- `docker`: list/start/stop/restart containers
- `discord`: placeholder command handler

## Requirements

- Node.js
- npm
- A Gemini API key

Optional but useful:

- `playerctl` for Spotify control on Linux
- Docker CLI for Docker plugin commands
- Firefox installed at `/usr/bin/firefox` if you want to use the system Firefox automation option

## Getting Started

### 1. Install dependencies

```bash
npm install
```

### 2. Create `.env`

```env
GEMINI_API_KEY=your_google_gemini_api_key
```

You can also update the API key later from the Settings window.

### 3. Start the app

```bash
npm start
```

## Usage Notes

- Global shortcut: `Ctrl+Shift+Space`
- Closing the main window hides the app to tray instead of quitting
- Smart Context only runs when enabled from the UI
- Proactive analysis pauses when the system is idle

## Example Commands

- `เปิด YouTube`
- `เปิด VS Code`
- `copy ข้อความ Hello World`
- `อากาศที่กรุงเทพเป็นยังไง`
- `สร้างโฟลเดอร์ชื่อ Projects`
- `หาข่าว AI ล่าสุดแล้วสรุปให้หน่อย`
- `จดจำไฟล์ /absolute/path/to/notes.md`

## Project Structure

```text
Mint/
├── assets/
├── main.js
├── preload.js
├── preload-picker.js
├── preload-settings.js
├── package.json
└── src/
    ├── AI_Brain/
    ├── Automation_Layer/
    ├── Command_Parser/
    ├── Plugins/
    ├── System/
    └── UI/
```

Key files:

- `main.js`: Electron main process, IPC, tray, capture loops
- `src/AI_Brain/Gemini_API.js`: chat and image translation calls
- `src/UI/renderer.js`: main chat window logic
- `src/UI/screenPicker.js`: screen selection and Live Translate overlay
- `src/System/config_manager.js`: persistent app settings

## License

This project uses the license terms in [LICENSE](./LICENSE).
