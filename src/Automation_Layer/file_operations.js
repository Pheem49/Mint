const { shell } = require('electron');
const fs = require('fs');
const path = require('path');
const os = require('os');

/**
 * สร้างโฟลเดอร์ใหม่
 * target: ชื่อโฟลเดอร์ หรือ absolute path
 * ถ้าเป็นชื่อเดียว จะสร้างบน Desktop
 */
function createFolder(target) {
    if (!target) return { success: false, message: 'No folder name provided.' };

    let folderPath = target;

    // ถ้าไม่ใช่ absolute path ให้สร้างบน Desktop
    if (!path.isAbsolute(target)) {
        const desktopPath = path.join(os.homedir(), 'Desktop');
        folderPath = path.join(desktopPath, target);
    }

    try {
        fs.mkdirSync(folderPath, { recursive: true });
        console.log(`Folder created: ${folderPath}`);
        return { success: true, path: folderPath };
    } catch (err) {
        console.error('createFolder error:', err);
        return { success: false, message: err.message };
    }
}

/**
 * เปิดไฟล์หรือโฟลเดอร์ด้วย default app ของระบบ
 */
async function openFile(target) {
    if (!target) return;
    const result = await shell.openPath(target);
    if (result) {
        console.error('openFile error:', result);
    }
}

/**
 * ลบไฟล์หรือโฟลเดอร์ (ย้ายไป Trash)
 */
async function deleteFile(target) {
    if (!target) return { success: false, message: 'No path provided.' };
    try {
        await shell.trashItem(target);
        return { success: true };
    } catch (err) {
        console.error('deleteFile error:', err);
        return { success: false, message: err.message };
    }
}

module.exports = { createFolder, openFile, deleteFile };
