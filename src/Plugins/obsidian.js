const fs = require('fs');
const path = require('path');

function getNotesDir() {
    let base = process.env.HOME || process.env.USERPROFILE || process.cwd();
    // Default to Documents/Mint_Notes
    const dir = path.join(base, 'Documents', 'Mint_Notes');
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
    return dir;
}

module.exports = {
    name: 'obsidian',
    description: 'Manage local Markdown notes (like Obsidian/Notion). Instruction MUST be one of: "list", "read: [filename]", "write: [filename] | [content]".',
    
    async execute(instruction) {
        const dir = getNotesDir();
        
        if (instruction.startsWith('list')) {
            const files = fs.readdirSync(dir).filter(f => f.endsWith('.md'));
            if (files.length === 0) return "ยังไม่มีโน้ตอยู่ในระบบค่ะ 📝";
            return `รายการโน้ตทั้งหมด:\n${files.join('\n')}`;
        }
        
        if (instruction.startsWith('read:')) {
            let filename = instruction.replace('read:', '').trim();
            if (!filename.endsWith('.md')) filename += '.md';
            const filepath = path.join(dir, filename);
            if (fs.existsSync(filepath)) {
                return `เนื้อหาของโน้ต ${filename}:\n\n${fs.readFileSync(filepath, 'utf8')}`;
            }
            return `ไม่พบโน้ตชื่อ ${filename} ค่ะ ❌`;
        }
        
        if (instruction.startsWith('write:')) {
            const parts = instruction.replace('write:', '').split('|');
            if (parts.length < 2) return "รูปแบบคำสั่งไม่ถูกต้องค่ะ ต้องเป็น write: filename | content";
            let filename = parts[0].trim();
            const content = parts.slice(1).join('|').trim();
            
            if (!filename.endsWith('.md')) filename += '.md';
            const filepath = path.join(dir, filename);
            
            // Log timestamp
            const timestamp = new Date().toLocaleString('th-TH');
            const entry = `\n---บันทึกเมื่อ ${timestamp}---\n${content}\n`;
            
            fs.appendFileSync(filepath, entry);
            return `บันทึกข้อความลงในโน้ต ${filename} เรียบร้อยแล้วค่ะ ✅`;
        }
        
        return "คำสั่งโน้ตไม่ถูกต้องค่ะ ลองใช้ 'list', 'read: ชื่อไฟล์', หรือ 'write: ชื่อไฟล์ | เนื้อหา' นะคะ";
    }
};
