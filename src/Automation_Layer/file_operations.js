const { execFile } = require('child_process');
let shell;
try {
    shell = require('electron').shell;
} catch (e) {
    shell = null;
}
const fs = require('fs');
const path = require('path');
const os = require('os');

const IGNORED_DIRECTORY_NAMES = new Set([
    '.git',
    'node_modules',
    '.cache',
    'dist',
    'build',
    'coverage'
]);

function getSearchRoots() {
    return Array.from(new Set([
        process.cwd(),
        os.homedir()
    ]));
}

/**
 * Smartly resolves a path.
 * If a path starts with '/' but doesn't exist at root, checks if it exists relative to home.
 * Also handles '~/' expansion.
 */
function resolveSmartPath(target) {
    if (!target) return target;

    const home = os.homedir();
    const commonFolders = ['Downloads', 'Desktop', 'Documents', 'Videos', 'Pictures', 'Music', 'vscode', 'Games'];

    // 1. If it's already an absolute path and exists, use it
    if (path.isAbsolute(target) && fs.existsSync(target)) return target;

    // 2. If it starts with ~/ expand it
    if (target.startsWith('~/')) {
        const expanded = path.join(home, target.substring(2));
        if (fs.existsSync(expanded)) return expanded;
    }

    // 3. If it starts with / but doesn't exist at root, try home directory
    if (target.startsWith('/')) {
        const homeRelative = path.join(home, target.substring(1));
        if (fs.existsSync(homeRelative)) return homeRelative;
    }

    // 4. Check if the target itself starts with a common folder (e.g., "Downloads/resume.pdf")
    const parts = target.split(/[/\\]/);
    const firstPart = parts[0];
    if (commonFolders.includes(firstPart)) {
        const potentialPath = path.join(home, target);
        if (fs.existsSync(potentialPath)) return potentialPath;
    }

    // 5. Try searching the filename in all common folders
    for (const folder of commonFolders) {
        const potentialPath = path.join(home, folder, target);
        if (fs.existsSync(potentialPath)) return potentialPath;
    }

    // 6. Final fallback: just return as is (might be relative to CWD)
    return target;
}

function findPath(target, options = {}) {
    if (!target || !target.trim()) {
        return { success: false, message: 'No search query provided.', matches: [] };
    }

    const normalizedType = ['file', 'dir', 'any'].includes(options.type) ? options.type : 'any';
    const loweredQuery = target.trim().toLowerCase();
    const exactMatches = [];
    const partialMatches = [];
    const visited = new Set();
    const maxResults = options.maxResults || 20;
    const searchRoots = Array.isArray(options.roots) && options.roots.length > 0
        ? options.roots
        : getSearchRoots();

    function buildMatch(entryPath, entryType, rootPath, exactNameMatch) {
        const relativeToCwd = path.relative(process.cwd(), entryPath);
        const pathDepth = entryPath.split(path.sep).length;
        return {
            path: entryPath,
            type: entryType,
            exactNameMatch,
            inCurrentWorkspace: !relativeToCwd.startsWith('..') && !path.isAbsolute(relativeToCwd),
            pathDepth,
            rootPath
        };
    }

    function sortMatches(matches) {
        return matches.sort((a, b) => {
            if (a.exactNameMatch !== b.exactNameMatch) return a.exactNameMatch ? -1 : 1;
            if (a.inCurrentWorkspace !== b.inCurrentWorkspace) return a.inCurrentWorkspace ? -1 : 1;
            if (a.pathDepth !== b.pathDepth) return a.pathDepth - b.pathDepth;
            return a.path.localeCompare(b.path);
        });
    }

    function visit(currentPath, rootPath) {
        let entries = [];
        try {
            entries = fs.readdirSync(currentPath, { withFileTypes: true });
        } catch (_) {
            return;
        }

        for (const entry of entries) {
            const absoluteEntryPath = path.join(currentPath, entry.name);
            if (visited.has(absoluteEntryPath)) continue;
            visited.add(absoluteEntryPath);

            const entryType = entry.isDirectory() ? 'dir' : 'file';
            if (entry.isDirectory() && IGNORED_DIRECTORY_NAMES.has(entry.name)) {
                continue;
            }
            const relativePath = path.relative(rootPath, absoluteEntryPath);
            const searchablePath = relativePath || entry.name;
            const matchesType = normalizedType === 'any' || normalizedType === entryType;
            const lowerEntryName = entry.name.toLowerCase();
            const exactNameMatch = lowerEntryName === loweredQuery;
            const partialMatch = lowerEntryName.includes(loweredQuery) || searchablePath.toLowerCase().includes(loweredQuery);

            if (matchesType && partialMatch) {
                const match = buildMatch(absoluteEntryPath, entryType, rootPath, exactNameMatch);
                if (exactNameMatch) {
                    exactMatches.push(match);
                    if (exactMatches.length >= maxResults) return;
                } else if (exactMatches.length === 0) {
                    partialMatches.push(match);
                    if (partialMatches.length >= maxResults) return;
                }
            }

            if (entry.isDirectory() && exactMatches.length < maxResults && partialMatches.length < maxResults) {
                visit(absoluteEntryPath, rootPath);
                if (exactMatches.length >= maxResults || partialMatches.length >= maxResults) return;
            }
        }
    }

    for (const rootPath of searchRoots) {
        if (!fs.existsSync(rootPath)) continue;
        visit(rootPath, rootPath);
        if (exactMatches.length >= maxResults || partialMatches.length >= maxResults) break;
    }

    const matches = exactMatches.length > 0
        ? sortMatches(exactMatches).slice(0, maxResults)
        : sortMatches(partialMatches).slice(0, maxResults);

    if (matches.length === 0) {
        return {
            success: false,
            message: `ไม่พบ${normalizedType === 'dir' ? 'โฟลเดอร์' : normalizedType === 'file' ? 'ไฟล์' : 'ไฟล์หรือโฟลเดอร์'}ที่ตรงกับ "${target}" ค่ะ`,
            matches: []
        };
    }

    return {
        success: true,
        matches: matches.map(({ path: matchPath, type }) => ({ path: matchPath, type }))
    };
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
    
    if (!fs.existsSync(resolvedPath)) {
        console.error(`[OpenFile] File not found: ${resolvedPath}`);
        return `ไม่พบไฟล์หรือโฟลเดอร์: ${target} ค่ะ`;
    }

    if (shell) {
        const result = await shell.openPath(resolvedPath);
        if (result) {
            console.error('openFile error:', result);
            return `เกิดข้อผิดพลาดในการเปิดไฟล์: ${result}`;
        }
        return true;
    } else {
        return new Promise((resolve) => {
            // บน Linux ลอง xdg-open แล้วค่อย gio open ถ้าอันแรกไม่ทำงาน
            const { exec } = require('child_process');
            const platformCmd = process.platform === 'darwin' ? 'open' : (process.platform === 'win32' ? 'start' : 'xdg-open');
            
            // ใช้ exec เพื่อให้รันผ่าน shell และรองรับการทำ fallback
            let cmd = `${platformCmd} "${resolvedPath}"`;
            if (process.platform === 'linux') {
                cmd = `xdg-open "${resolvedPath}" || gio open "${resolvedPath}" || nautilus "${resolvedPath}"`;
            }

            exec(cmd, (err) => {
                if (err) {
                    console.error("Failed to open path:", err);
                    resolve(`ไม่สามารถเปิดได้ค่ะ: ${err.message}`);
                } else {
                    resolve(true);
                }
            });
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
            execFile('gio', ['trash', resolvedPath], (err) => {
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

module.exports = { createFolder, openFile, deleteFile, findPath };
