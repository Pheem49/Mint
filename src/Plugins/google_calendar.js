const { shell } = require('electron');

module.exports = {
    name: 'google_calendar',
    description: 'Quickly open Google Calendar to add a new event or view the calendar. Instruction should be the event title (e.g., "Meeting with team"). If no title, just put "open".',
    
    async execute(instruction) {
        const inst = (instruction || '').trim();
        
        if (!inst || inst.toLowerCase() === 'open') {
            shell.openExternal('https://calendar.google.com/');
            return 'กำลังเปิดหน้าต่างปฏิทินให้ค่ะ 📅';
        }
        
        // Encode the event title for the URL
        const title = encodeURIComponent(inst);
        const url = `https://calendar.google.com/calendar/r/eventedit?text=${title}`;
        
        try {
            shell.openExternal(url);
            return `กำลังเปิดหน้าต่างสร้างกิจกรรม "${inst}" ใน Google Calendar ให้ลูกพี่ค่ะ 📅✨`;
        } catch (e) {
            return `เกิดข้อผิดพลาดในการเปิด Calendar ค่ะ: ${e.message}`;
        }
    }
};
