const { exec } = require('child_process');
let shell;
try {
    shell = require('electron').shell;
} catch (e) {
    shell = null;
}
const fs = require('fs');
const path = require('path');
const os = require('os');

/**
 * Smartly resolves a path.
 * If a path starts with '/' but doesn't exist at root, checks if it exists relative to home.
 * Also handles '~/' expansion.
 */
function resolveSmartPath(target) {
    if (!target) return target;

    // 1. If it exists as is (absolute or relative to CWD), use it
    if (fs.existsSync(target)) return target;

    const commonFolders = ['Downloads', 'Desktop', 'Documents', 'Videos', 'Pictures', 'Music', 'vscode', 'Games'];

    // 2. If it starts with / and doesn't exist at root, try home directory
    if (target.startsWith('/')) {
        const homeRelative = path.join(os.homedir(), target.substring(1));
        if (fs.existsSync(homeRelative)) return homeRelative;
        
        const cwdRelative = path.join(process.cwd(), target.substring(1));
        if (fs.existsSync(cwdRelative)) return cwdRelative;

        const firstPart = target.split('/')[1];
        if (commonFolders.includes(firstPart)) return homeRelative;
    }

    // 3. Handle ~ manually
    if (target.startsWith('~/')) {
        return path.join(os.homedir(), target.substring(2));
    }

    // 4. If it's just a name, search in common folders
    for (const folder of commonFolders) {
        const potentialPath = path.join(os.homedir(), folder, target);
        if (fs.existsSync(potentialPath)) return potentialPath;
    }

    return target;
}
/**
 * สร้างโฟลเดอร์ใหม่
 * target: ชื่อโฟลเดอร์ หรือ absolute path
 * ถ้าเป็นชื่อเดียว จะสร้างบน Desktop
 */
function createFolder(target) {
    if (!target) return { success: false, message: 'No folder name provided.' };

    let folderPath = resolveSmartPath(target);

    // If still not absolute (was just a name), default to Desktop
    if (!path.isAbsolute(folderPath)) {
        const desktopPath = path.join(os.homedir(), 'Desktop');
        folderPath = path.join(desktopPath, folderPath);
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
    const resolvedPath = resolveSmartPath(target);
    
    if (shell) {
        const result = await shell.openPath(resolvedPath);
        if (result) console.error('openFile error:', result);
    } else {
        exec(`xdg-open "${resolvedPath}"`, (err) => {
            if (err) console.error("Failed to open path via xdg-open:", err);
        });
    }
}

/**
 * ลบไฟล์หรือโฟลเดอร์ (ย้ายไป Trash)
 */
async function deleteFile(target) {
    if (!target) return { success: false, message: 'No path provided.' };
    const resolvedPath = resolveSmartPath(target);

    if (shell) {
        try {
            await shell.trashItem(resolvedPath);
            return { success: true };
        } catch (err) {
            console.error('deleteFile error:', err);
            return { success: false, message: err.message };
        }
    } else {
        return new Promise((resolve) => {
            exec(`gio trash "${resolvedPath}"`, (err) => {
                if (err) {
                    console.error("Failed to trash item via gio trash:", err);
                    resolve({ success: false, message: err.message });
                } else {
                    resolve({ success: true });
                }
            });
        });
    }
}

module.exports = { createFolder, openFile, deleteFile };
