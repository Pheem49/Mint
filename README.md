# 🌿 Mint — AI Desktop Agent

Mint เป็น Desktop Agent ที่ขับเคลื่อนด้วย AI (Google Gemini) สร้างด้วย Electron  
พิมพ์หรือพูดคำสั่งภาษาธรรมชาติ แล้วให้ Mint จัดการให้ทุกอย่าง

---

## ✨ Features

| ความสามารถ | รายละเอียด |
|-----------|-----------|
| 💬 AI Chat | คุยด้วยภาษาธรรมชาติผ่าน Google Gemini พร้อมจำบทสนทนา |
| ✨ Proactive Assistant | AI วิเคราะห์หน้าจอ + พฤติกรรมผู้ใช้ แล้วเสนอความช่วยเหลือก่อนถูกถาม (ทำงานเมื่อเปิด Smart Context) |
| 🧠 Smart Context AI | จับหน้าจอแบบ Silent อัตโนมัติทุกครั้งที่ส่งข้อความ เพื่อให้ AI เข้าใจบริบทโดยไม่ต้องอัปโหลดเอง |
| 📚 Local Knowledge (RAG) | คุยและถามตอบความรู้จากไฟล์ในเครื่องของคุณ (txt, md) โดย AI จะอ้างอิงข้อมูลเฉพาะจากไฟล์ |
| 👁️ Screen Vision | ให้ AI ดูหน้าจอ หรือแคปรูปเฉพาะส่วนไปวิเคราะห์ได้เลย |
| 🖼️ Image Drop | รองรับการ Copy & Paste และ Drag & Drop ไฟล์รูปภาพ |
| 🌐 Open URL | สั่งเปิดเว็บไซต์ได้เลย |
| 🔍 Web Search | ค้นหา Google โดยอัตโนมัติ |
| 🚀 Open App | สั่งเปิดโปรแกรมบนเครื่อง |
| 🤖 Multi-Step Web Automation | ระบบ Agentic Loop ทำงานอัตโนมัติบนเว็บเป็นขั้นเป็นตอน (ค้นหา → อ่าน → สรุป) |
| 📁 File Operations | สร้างโฟลเดอร์, เปิดไฟล์, ลบไฟล์ผ่านคำสั่ง AI |
| 📋 Clipboard | สั่งให้ copy ข้อความไปยัง Clipboard ได้เลย |
| 🌡️ System Info | ถาม RAM, CPU, เวลา หรืออากาศได้ทุกเมื่อ |
| ⚙️ Settings | ตั้งค่า API Key, Theme, Accent Color, เว็บเบราว์เซอร์, และ Proactive Interval ผ่าน UI |
| 🎨 Premium Themes | Dark / Light / Midnight พร้อม Glassmorphism Effect และ Custom Accent Color |
| 🖥️ Window Management | ย่อ ขยาย (Maximize) หรือรันซ่อนใน Background ผ่าน System Tray เข้าถึงง่าย |
| 🎙️ Voice Input | พูดสั่งด้วย Web Speech API ภาษาไทย |
| 📥 Tray Icon | รันซ่อนใน Background ผ่าน System Tray เข้าถึงง่าย |
| ⌨️ Global Shortcut | `Ctrl+Shift+Space` เรียก Mint ได้ทุกที่ |
| 🔌 Plugin System | รองรับ Plugin เพิ่มเติม (Spotify, Discord, Docker ฯลฯ) |

---

## 🤖 Proactive Assistant

Proactive Assistant คือระบบที่ทำให้ Mint **เริ่มบทสนทนาก่อน** โดยไม่ต้องรอให้ผู้ใช้สั่ง

```
เปิด Smart Context Toggle
        ↓
AI จับหน้าจอทุก N วินาที (ปรับได้ใน Settings)
        ↓
Gemini วิเคราะห์ context + Behavior Memory
        ↓
ถ้าพบสิ่งที่น่าช่วย → ✨ Suggestion Bar โผล่ขึ้นมา
        ↓
[ใช่ค่ะ] → AI ดำเนินการทันที   [✕] → ปิด
```

**ตัวอย่าง:**
- เปิด Chrome → AI: "คุณต้องการเปิด YouTube ไหมคะ?"
- กำลัง Code นานๆ → AI: "ต้องการค้นหาข้อมูลอะไรไหมคะ?"
- เปิด Spotify → AI: "ต้องการเล่นเพลงเลยไหมคะ?"

**ปรับได้ใน ⚙️ Settings:**
| ค่า | ช่วง | ค่าเริ่มต้น |
|---|---|---|
| ความถี่ Capture | 30วิ – 5นาที | 60 วิ |
| ช่วงพักระหว่าง Suggestion | 1 – 10 นาที | 2 นาที |

---

## ⚡ Performance Optimizations

Mint ถูกออกแบบมาให้ประหยัดทรัพยากรเครื่องที่สุด แม้จะมีฟีเจอร์ AI แบบ Real-time:
- **JPEG Downscaling:** บีบอัดภาพหน้าจอลง 50% และแปลงเป็น JPEG (Quality 60) ก่อนส่งให้ AI
- **Smart Idle Detection:** เมื่อไม่ได้ใช้งานเมาส์/คีย์บอร์ดเกิน 5 นาที ระบบ Proactive + Screen Capture จะหยุดพักตัวเองเพื่อประหยัดแบตเตอรี่และ CPU อัตโนมัติ
- **UI IPC Throttling:** ป้องกันแอปค้างจากการสแปมหน้าต่างแชท

---

## 🛠️ Tech Stack

- **[Electron](https://www.electronjs.org/)** — Framework สำหรับ Desktop App
- **[Google Gemini API](https://ai.google.dev/)** (`@google/genai`) — AI Brain, Planner & Proactive Engine
- **[Puppeteer](https://pptr.dev/)** — Web Automation (รองรับ Bundled Chromium & System Firefox)
- **[dotenv](https://github.com/motdotla/dotenv)** — จัดการ Environment Variables

---

## 🚀 Getting Started

### 1. Clone โปรเจกต์

```bash
git clone https://github.com/Pheem49/Luna-Mint.git
cd Luna-Mint
```

### 2. ติดตั้ง Dependencies

```bash
npm install
```

### 3. ตั้งค่า Environment Variables

สร้างไฟล์ `.env` ที่ root ของโปรเจกต์:

```env
GEMINI_API_KEY=your_google_gemini_api_key_here
```

> 🔑 ขอ API Key ได้ที่ [Google AI Studio](https://aistudio.google.com/)  
> หรือตั้งค่าผ่านหน้า **⚙️ Settings** ใน app ได้เลย

### 4. รัน App

```bash
npm start
```

---

## 💬 ตัวอย่างคำสั่ง

```
"เปิด YouTube"                                    → เปิดเว็บ YouTube
"เปิด VS Code"                                    → เปิดโปรแกรม
"ค้นหาข่าว AI วันนี้"                              → ค้นหา Google
"สร้างโฟลเดอร์ชื่อ Projects"                       → สร้างโฟลเดอร์บน Desktop
"RAM เหลือเท่าไหร่"                                → แสดงข้อมูลระบบ
"อากาศที่กรุงเทพวันนี้เป็นยังไง"                   → ดึงข้อมูลอากาศ
"Copy ข้อความ Hello World"                         → copy ไป Clipboard
"แปลข้อความในรูปนี้ให้หน่อย"                       → (ใช้คู่กับปุ่ม Vision 👁️ หรือลากรูปลงช่องแชท)
"หาข่าวล่าสุดเกี่ยวกับ AI แล้วสรุปให้ฟังหน่อย"    → AI วางแผนและเชื่อมต่อเบราว์เซอร์หาข้อมูลให้
"จดจำไฟล์ /path/to/my_notes.txt"                   → AI จะดึงเนื้อหาจากไฟล์มารวมเข้ากับ Knowledge Base (RAG)
```

---

## 📁 Project Structure

```
Mint/
├── assets/
│   └── icon.png                    # App Icon
├── main.js                         # Entry point (Electron Main Process)
├── preload.js                      # Preload — Chat window
├── preload-settings.js             # Preload — Settings window
├── preload-picker.js               # Preload — Vision Picker window
├── package.json
├── .env                            # API Keys (ห้ามขึ้น Git!)
└── src/
    ├── AI_Brain/
    │   ├── Gemini_API.js           # Gemini Chat Session + RAG Injection
    │   ├── proactive_engine.js     # วิเคราะห์หน้าจอ + สร้าง Proactive Suggestion
    │   ├── behavior_memory.js      # จำพฤติกรรมและ Context ของผู้ใช้
    │   └── knowledge_base.js       # Local RAG System สร้าง Vector ค้นหาเอกสาร
    ├── Automation_Layer/
    │   ├── open_app.js             # เปิดโปรแกรม
    │   ├── open_website.js         # เปิดเว็บ / ค้นหา
    │   ├── browser_automation.js   # Puppeteer automation
    │   └── file_operations.js      # สร้าง/เปิด/ลบไฟล์
    ├── Plugins/
    │   ├── plugin_manager.js       # โหลดและจัดการ Plugin
    │   ├── spotify.js              # Spotify Plugin
    │   └── ...                     # Plugin เพิ่มเติม
    ├── System/
    │   ├── system_info.js          # RAM, CPU, เวลา, อากาศ
    │   ├── config_manager.js       # อ่าน/เขียน config.json
    │   └── chat_history_manager.js # จัดการประวัติการสนทนา
    ├── Command_Parser/
    │   └── parser.js               # แปลง AI response เป็น action
    └── UI/
        ├── index.html              # Chat Window
        ├── styles.css              # Styling + Themes + Proactive Bar
        ├── renderer.js             # Chat Logic + Proactive UI
        ├── settings.html           # Settings Window
        ├── settings.css            # Settings Styling
        ├── settings.js             # Settings Logic
        ├── screenPicker.html       # Vision Screen Overlay
        └── screenPicker.js         # Vision Logic & Region Selector
```

---

## ⌨️ Global Shortcut

| Shortcut | Action |
|----------|--------|
| `Ctrl + Shift + Space` | แสดง / ซ่อน Mint |

---

## 📝 License

**MIT + Commons Clause**

ใช้งานและแก้ไขได้อิสระ แต่ **ห้ามนำไปขายหรือทำกำไร**  
ดูรายละเอียดเพิ่มเติมได้ที่ [LICENSE](./LICENSE)
