
# Mint Release Notes

## v1.4.0 - The "Memory & Stream" Update

Mint 1.4.0 เป็น release ที่ใหญ่ที่สุดนับตั้งแต่ต้น — Mint ตอนนี้ **จำคุณได้** ข้ามทุก session, **ตอบแบบ streaming** แบบ real-time, และมี **Spotify Plugin** ที่ใช้งานได้จริงครบ loop พร้อม unit test suite ครบถ้วนครั้งแรก!

### ✨ New Features

* **⚡ Streaming Responses (Gemini):**
    * Mint ตอบสนองแบบ real-time แล้วค่ะ! ข้อความปรากฏทีละคำแบบ typewriter แทนที่จะรอ response ทั้งหมดก่อน
    * ใช้ `chat.sendMessageStream()` ของ Gemini SDK — สกัด `response` field ออกจาก JSON buffer แบบ progressive
    * Provider อื่น (Ollama, Anthropic, OpenAI, HuggingFace) ยังทำงานเหมือนเดิม 100%

* **🧠 Long-Term Memory (`memory_store.js`):**
    * Mint จำคุณได้ข้ามทุก session แล้วค่ะ! เก็บใน SQLite DB เดียวกับ Knowledge Base
    * สิ่งที่ Mint จำ: ภาษาที่ใช้บ่อย, project ล่าสุด, จำนวน interactions, topics/tools ที่ใช้ประจำ
    * Context นี้ถูก inject เข้า system prompt อัตโนมัติทุก session — ทำให้ Mint เป็น "ผู้ช่วยส่วนตัวจริงๆ"
    * รองรับ: session summaries, usage pattern tracking, user profile CRUD

* **🎵 Spotify Plugin — Complete Edition:**
    * ครบ loop ผ่าน `playerctl` (ไม่ต้อง OAuth):
    * **Playback:** `play`, `pause`, `stop`, `next`, `previous`
    * **Now Playing:** `now_playing` / `status` — แสดงชื่อเพลง, ศิลปิน, อัลบั้ม
    * **Volume:** `volume <0-100>` — ปรับระดับเสียง
    * **Shuffle:** `shuffle on` / `shuffle off` / `shuffle toggle`
    * **Search:** `search <query>` — เปิด Spotify search ใน browser

### 🛠️ Improvements & Refactoring

* **`buildSystemPrompt()` Helper:**
    * รวม code ที่ซ้ำกัน 5 ก้อนใน Gemini/Anthropic/OpenAI/HuggingFace/Ollama/Ollama handlers ให้เป็น function เดียว
    * ทุก provider ใช้ system prompt เดียวกัน รวมถึง MCP Tools, Plugin Descriptions, และ Long-Term User Context

### 🧪 Testing

* **Jest Unit Test Suite (53 tests, 3 suites — all pass):**
    * `tests/config_manager.test.js` — readConfig, writeConfig, getAvailableProviders (13 tests)
    * `tests/memory_store.test.js` — profile CRUD, language detection, patterns, session summaries, getUserContext (20 tests)
    * `tests/spotify.test.js` — ทุก command, error cases, interface validation (20 tests)
    * Test isolation ที่แท้จริง: แต่ละ test มี temp DB/config ของตัวเอง ไม่ยุ่งกับ production files

### 📦 Installation

Update ผ่าน npm:
```bash
npm install -g @pheem49/mint@latest
```

Run tests:
```bash
npm test
```

---

## v1.3.0 - The "Agent & Plugin Power-Up" Update

Mint 1.3.0 มาพร้อมกับการยกระดับระบบ Agent และ Plugin ให้ฉลาดและยืดหยุ่นยิ่งขึ้น พร้อมฟีเจอร์ใหม่ที่ช่วยให้การใช้งาน Mint สนุกและทรงพลังยิ่งกว่าเดิม!

### ✨ New Features
* **Agent Framework:**
    * รองรับการสร้างและสลับ Agent หลายตัวใน CLI (เช่น code agent, browser agent, knowledge agent)
    * เพิ่มคำสั่ง `/agent` สำหรับจัดการ agent และดูสถานะปัจจุบัน
* **Plugin System 2.0:**
    * ปรับปรุงระบบปลั๊กอินให้โหลด/ปิดใช้งานแบบไดนามิกได้ทันที ไม่ต้องรีสตาร์ท
    * เพิ่มคำสั่ง `/plugins` สำหรับดูและจัดการปลั๊กอิน
* **Contextual Help:**
    * เพิ่มระบบช่วยเหลืออัตโนมัติ (context-aware help) แนะนำคำสั่งและปลั๊กอินที่เกี่ยวข้องตามสถานการณ์
* **Thai Language UX:**
    * ปรับปรุงการรองรับภาษาไทยใน UI และข้อความตอบกลับ

### 🛠️ Improvements & Bug Fixes
* **Performance Boost:** ปรับปรุงความเร็วการตอบสนองของ CLI และลดการใช้หน่วยความจำ
* **Better Error Handling:** แจ้งเตือนข้อผิดพลาดแบบเข้าใจง่ายและแนะนำวิธีแก้ไข
* **Config Hot Reload:** ตั้งค่า mint-config.json สามารถรีโหลดได้ทันทีโดยไม่ต้องปิดโปรแกรม
* **Security:** อัปเดต dependencies และเพิ่มการตรวจสอบความปลอดภัยของปลั๊กอิน

---

## v1.2.4 - The "Smart Path & Dynamic Version" Update

This update makes Mint much more "street smart" when it comes to finding your files and provides a more consistent versioning experience across the CLI.

### ✨ New Features
*   **Smart Path Resolution:** 
    *   **Automatic Root Correction:** If you (or the AI) specify a path like `/Downloads/...` that doesn't exist at the system root, Mint now automatically checks your Home directory.
    *   **Common Directory Search:** Simply type a folder name (e.g., `Games` or `vscode`), and Mint will automatically search in your most common directories (Downloads, Desktop, Documents, Videos, Pictures, Music, vscode, Games).
    *   **Tilde (~) Expansion:** Added support for `~/` path expansion even when commands are executed in a quoted shell environment.
*   **Dynamic Versioning:**
    *   The CLI now dynamically pulls its version number directly from `package.json`. No more manual version bumps in multiple files!
    *   **Visibility:** Added version display to the startup header and the `/config` slash command.

### 🛠️ Improvements & Bug Fixes
*   **Robust File Operations:** Updated `createFolder`, `openFile`, and `deleteFile` to all use the new smart path resolution logic, preventing "No such file or directory" errors.
*   **CLI Consistency:** Fixed an issue where `mint --version` would report an outdated hardcoded version.

---

## v1.2.3 - The "Smart TUI" Update

This release focuses on enhancing the CLI experience with better system awareness, a polished terminal interface, and improved stability.

### ✨ New Features
*   **System Awareness:** Mint can now retrieve and report system information, including OS, Kernel version, and Architecture (x86_64).
*   **Enhanced TUI Layout:** 
    *   Implemented a new "Bubble-Lite" message style with distinct left-borders for the assistant and proper indentation for all multi-line messages.
    *   **Manual Text Wrapping:** Added smart text wrapping to ensure long Thai/English messages stay within the frame without breaking the layout.
*   **Mouse Support:** Enabled mouse scroll wheel support for navigating chat history effortlessly.
*   **Startup Info:** Added a clean startup header showing the **Active Model** and current **Timestamp**.

### 🛠️ Improvements & Bug Fixes
*   **Terminal Cleanup:** Fixed an issue where "garbage" mouse coordinate characters (e.g., 35;35;44M) appeared after exiting. Added robust terminal state restoration (disabling mouse tracking and restoring cursor).
*   **Slash Command Aliases:** Added `/model` as a convenient alias for `/models`.
*   **Silence Background Logs:** Suppressed technical logs from `dotenvx` and model initialization to provide a cleaner chat experience.
*   **User Selection Hint:** Added UI hints for text selection (Shift+Drag) while mouse mode is active.

### 📦 Installation
Update to the latest version via npm:
`npm install -g @pheem49/mint@latest`
