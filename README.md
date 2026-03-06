# 🌿 Mint — AI Desktop Agent

Mint เป็น Desktop Agent ที่ขับเคลื่อนด้วย AI (Google Gemini) สร้างด้วย Electron  
พิมพ์หรือพูดคำสั่งภาษาธรรมชาติ แล้วให้ Mint จัดการให้ทุกอย่าง

---

## ✨ Features

| ความสามารถ | รายละเอียด |
|-----------|-----------|
| 💬 AI Chat | คุยด้วยภาษาธรรมชาติผ่าน Google Gemini พร้อมจำบทสนทนา |
| 🌐 Open URL | สั่งเปิดเว็บไซต์ได้เลย |
| 🔍 Web Search | ค้นหา Google โดยอัตโนมัติ |
| 🚀 Open App | สั่งเปิดโปรแกรมบนเครื่อง |
| 🤖 Web Automation | ทำงานอัตโนมัติบนเว็บด้วย Puppeteer |
| 📁 File Operations | สร้างโฟลเดอร์, เปิดไฟล์, ลบไฟล์ผ่านคำสั่ง AI |
| 📋 Clipboard | สั่งให้ copy ข้อความไปยัง Clipboard ได้เลย |
| 🌡️ System Info | ถาม RAM, CPU, เวลา หรืออากาศได้ทุกเมื่อ |
| ⚙️ Settings | ตั้งค่า API Key, Theme, Accent Color ผ่าน UI |
| 🎨 Multiple Themes | Dark / Light / Midnight + Custom Accent Color |
| 🎙️ Voice Input | พูดสั่งด้วย Web Speech API ภาษาไทย |
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
"เปิด YouTube"                       → เปิดเว็บ YouTube
"เปิด VS Code"                        → เปิดโปรแกรม
"ค้นหาข่าว AI วันนี้"                 → ค้นหา Google
"สร้างโฟลเดอร์ชื่อ Projects"          → สร้างโฟลเดอร์บน Desktop
"RAM เหลือเท่าไหร่"                   → แสดงข้อมูลระบบ
"อากาศที่กรุงเทพวันนี้เป็นยังไง"      → ดึงข้อมูลอากาศ
"Copy ข้อความ Hello World"            → copy ไป Clipboard
```

---

## 📁 Project Structure

```
Mint/
├── main.js                         # Entry point (Electron Main Process)
├── preload.js                      # Preload — Chat window
├── preload-settings.js             # Preload — Settings window
├── package.json
├── .env                            # API Keys (ห้ามขึ้น Git!)
└── src/
    ├── AI_Brain/
    │   └── Gemini_API.js           # Gemini Chat Session (มี History)
    ├── Automation_Layer/
    │   ├── open_app.js             # เปิดโปรแกรม
    │   ├── open_website.js         # เปิดเว็บ / ค้นหา
    │   ├── browser_automation.js   # Puppeteer automation
    │   └── file_operations.js      # สร้าง/เปิด/ลบไฟล์
    ├── System/
    │   ├── system_info.js          # RAM, CPU, เวลา, อากาศ
    │   └── config_manager.js       # อ่าน/เขียน config.json
    ├── Command_Parser/
    │   └── parser.js               # แปลง AI response เป็น action
    └── UI/
        ├── index.html              # Chat Window
        ├── styles.css              # Styling + Themes
        ├── renderer.js             # Chat Logic
        ├── settings.html           # Settings Window
        ├── settings.css            # Settings Styling
        └── settings.js             # Settings Logic
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
