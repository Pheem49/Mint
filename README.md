# 🌿 Mint — AI Desktop Agent

Mint เป็น Desktop Agent ที่ขับเคลื่อนด้วย AI (Google Gemini) สร้างด้วย Electron  
พิมพ์คำสั่งภาษาธรรมชาติ แล้วให้ Mint จัดการให้ — เปิดเว็บ, เปิดแอป, ค้นหาข้อมูล, หรือทำ Web Automation อัตโนมัติ

---

## ✨ Features

| ความสามารถ | รายละเอียด |
|-----------|-----------|
| 💬 AI Chat | คุยด้วยภาษาธรรมชาติผ่าน Google Gemini |
| 🌐 Open URL | สั่งเปิดเว็บไซต์ได้เลย |
| 🔍 Web Search | ค้นหา Google โดยอัตโนมัติ |
| 🚀 Open App | สั่งเปิดโปรแกรมบนเครื่อง |
| 🤖 Web Automation | ทำงานอัตโนมัติบนเว็บด้วย Puppeteer |
| ⌨️ Global Shortcut | `Ctrl+Shift+Space` เรียก Mint ได้ทุกที่ |

---

## 🛠️ Tech Stack

- **[Electron](https://www.electronjs.org/)** — Framework สำหรับ Desktop App
- **[Google Gemini API](https://ai.google.dev/)** (`@google/genai`) — AI Brain
- **[Puppeteer](https://pptr.dev/)** — Web Automation
- **[dotenv](https://github.com/motdotla/dotenv)** — จัดการ Environment Variables

---

## 🚀 Getting Started

### 1. Clone โปรเจกต์

```bash
git clone <repo-url>
cd Mint
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

### 4. รัน App

```bash
npm start
```

---

## 📁 Project Structure

```
Mint/
├── main.js                    # Entry point (Electron Main Process)
├── preload.js                 # Preload script (Context Bridge)
├── package.json
├── .env                       # API Keys (ห้ามขึ้น Git!)
└── src/
    ├── AI_Brain/
    │   └── Gemini_API.js      # เชื่อมต่อ Google Gemini
    ├── Automation_Layer/
    │   ├── open_app.js        # เปิดโปรแกรม
    │   ├── open_website.js    # เปิดเว็บ / ค้นหา
    │   └── browser_automation.js  # Puppeteer automation
    ├── Command_Parser/
    │   └── parser.js          # แปลง AI response เป็น action
    └── UI/
        ├── index.html         # หน้า UI หลัก
        ├── styles.css         # Styling
        └── renderer.js        # Renderer Process Logic
```

---

## ⌨️ Global Shortcut

| Shortcut | Action |
|----------|--------|
| `Ctrl + Shift + Space` | แสดง / ซ่อน Mint |

---

## 📝 License

ISC
