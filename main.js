const { app, BrowserWindow, ipcMain, shell, globalShortcut, clipboard, Tray, Menu, nativeImage, desktopCapturer, screen, powerMonitor } = require('electron');
const path = require('path');
require('dotenv').config();

const { handleChat, resetChat, getChatTranscript } = require('./src/AI_Brain/Gemini_API');
const { openApp } = require('./src/Automation_Layer/open_app');
const { openWebsite, openSearch } = require('./src/Automation_Layer/open_website');
const { performWebAutomation } = require('./src/Automation_Layer/browser_automation');
const { createFolder, openFile, deleteFile } = require('./src/Automation_Layer/file_operations');
const { getSystemInfo, getWeather } = require('./src/System/system_info');
const { readConfig, writeConfig } = require('./src/System/config_manager');
const { parseCommand } = require('./src/Command_Parser/parser');
const pluginManager = require('./src/Plugins/plugin_manager');
const { analyzeAndSuggest } = require('./src/AI_Brain/proactive_engine');
const { recordBehavior, getBehaviorSummary } = require('./src/AI_Brain/behavior_memory');
const { indexFile } = require('./src/AI_Brain/knowledge_base');

let mainWindow;
let settingsWindow = null;
let screenPickerWindow = null;
let tray = null;

// =====================
// Proactive Loop
// =====================
let proactiveIntervalHandle = null;

async function runProactiveCycle() {
    if (!mainWindow || mainWindow.isDestroyed()) return;
    try {
        // Silent screen capture
        const primaryDisplay = screen.getPrimaryDisplay();
        // Downscale to 50% for performance
        const width = Math.floor(primaryDisplay.size.width * 0.5);
        const height = Math.floor(primaryDisplay.size.height * 0.5);
        
        const sources = await desktopCapturer.getSources({
            types: ['screen'],
            thumbnailSize: { width, height }
        });
        const primarySource = sources[0];
        if (!primarySource || !primarySource.thumbnail) return;

        // Compress image to JPEG to save huge base64 payload size and memory
        // .toJPEG(quality) where quality is 0-100
        const base64Image = primarySource.thumbnail.toJPEG(60).toString('base64');
        const behaviorSummary = getBehaviorSummary();

        const result = await analyzeAndSuggest(base64Image, behaviorSummary);

        if (result && result.message && Array.isArray(result.suggestions)) {
            // Record the observed context into behavior memory
            if (result.context) recordBehavior(result.context);

            // Push suggestion to the renderer
            if (mainWindow && !mainWindow.isDestroyed()) {
                mainWindow.webContents.send('proactive-suggestion', result);
            }
        }
    } catch (err) {
        console.error('[Proactive] Cycle error:', err.message);
    }
}

function startProactiveLoop(intervalSec) {
    // Stop existing loop first
    if (proactiveIntervalHandle) {
        clearInterval(proactiveIntervalHandle);
        proactiveIntervalHandle = null;
    }
    // Read from config if not provided
    const cfg = readConfig();
    const ms = (intervalSec || cfg.proactiveInterval || 60) * 1000;
    console.log(`[Proactive] Starting loop — interval: ${ms / 1000}s`);
    proactiveIntervalHandle = setInterval(runProactiveCycle, ms);
}

function stopProactiveLoop() {
    if (proactiveIntervalHandle) {
        clearInterval(proactiveIntervalHandle);
        proactiveIntervalHandle = null;
        console.log('[Proactive] Stopped proactive loop.');
    }
}

// Check idle state every minute to pause/resume background tasks
const IDLE_THRESHOLD_SEC = 300; // 5 minutes
setInterval(() => {
    if (!proactiveIntervalHandle) return; // Only manage if it's supposed to be active (Smart Context is ON)
    
    // powerMonitor.getSystemIdleTime() is available after app is ready
    if (app.isReady()) {
        const idleSec = powerMonitor.getSystemIdleTime();
        if (idleSec >= IDLE_THRESHOLD_SEC) {
            console.log(`[System Idle] User idle for ${idleSec}s. Pausing Proactive loop to save resources.`);
            stopProactiveLoop();
            
            // Wait for user to come back
            const resumeChecker = setInterval(() => {
                if (powerMonitor.getSystemIdleTime() < 10) {
                    console.log('[System Idle] User returned. Resuming Proactive loop.');
                    clearInterval(resumeChecker);
                    startProactiveLoop();
                }
            }, 5000);
        }
    }
}, 60000);

function createWindow() {
    mainWindow = new BrowserWindow({
        width: 400,
        height: 600,
        icon: path.join(__dirname, 'assets', 'icon.png'),
        webPreferences: {
            preload: path.join(__dirname, 'preload.js'),
            nodeIntegration: false,
            contextIsolation: true,
        },
        frame: false,
        transparent: true,
        resizable: true,
        show: false
    });

    mainWindow.loadFile('src/UI/index.html');

    mainWindow.on('ready-to-show', () => {
        mainWindow.show();
    });

    mainWindow.on('close', function (event) {
        if (!app.isQuiting) {
            event.preventDefault();
            mainWindow.hide();
        }
        return false;
    });
}

function createTray() {
    const iconPath = path.join(__dirname, 'assets', 'icon.png');
    let icon = nativeImage.createFromPath(iconPath);
    // Optional: resize it for the tray if needed, but Electron handles this natively on many OSs
    icon = icon.resize({ width: 16, height: 16 });
    
    tray = new Tray(icon);
    tray.setToolTip('Mint AI Assistant');
    
    const contextMenu = Menu.buildFromTemplate([
        { label: 'Show App', click: () => { if (mainWindow) mainWindow.show(); } },
        { label: 'Settings', click: () => { createSettingsWindow(); } },
        { type: 'separator' },
        { label: 'Quit', click: () => {
            app.isQuiting = true;
            app.quit();
        }}
    ]);
    
    tray.setContextMenu(contextMenu);
    
    tray.on('click', () => {
        if (mainWindow) {
            if (mainWindow.isVisible()) {
                mainWindow.hide();
            } else {
                mainWindow.show();
            }
        }
    });
}

function createSettingsWindow() {
    if (settingsWindow) {
        settingsWindow.focus();
        return;
    }
    settingsWindow = new BrowserWindow({
        width: 440,
        height: 560,
        icon: path.join(__dirname, 'assets', 'icon.png'),
        webPreferences: {
            preload: path.join(__dirname, 'preload-settings.js'),
            nodeIntegration: false,
            contextIsolation: true,
        },
        frame: false,
        transparent: true,
        resizable: false,
        parent: mainWindow,
    });
    settingsWindow.loadFile('src/UI/settings.html');
    settingsWindow.on('closed', () => { settingsWindow = null; });
}

app.whenReady().then(() => {
    createWindow();
    createTray();

    globalShortcut.register('CommandOrControl+Shift+Space', () => {
        if (mainWindow) {
            if (mainWindow.isVisible()) {
                mainWindow.hide();
            } else {
                mainWindow.show();
            }
        }
    });

    app.on('activate', function () {
        if (BrowserWindow.getAllWindows().length === 0) createWindow();
    });
});

app.on('window-all-closed', (e) => {
    e.preventDefault();
});

app.on('will-quit', () => {
    globalShortcut.unregisterAll();
});

// =====================
// IPC Handlers — Chat
// =====================
ipcMain.handle('chat-message', async (event, message, base64Image = null) => {
    try {
        const rawResponse = await handleChat(message, base64Image);
        const aiResponse = parseCommand(rawResponse);

        if (aiResponse.action && aiResponse.action.type !== 'none') {
            try {
                const actionResult = await executeAction(aiResponse.action);
                // If the action returned a string result (e.g. from Web Automation), append it.
                if (actionResult && typeof actionResult === 'string') {
                    aiResponse.response += `\n\n${actionResult}`;
                }
            } catch (err) {
                console.error("Action execution error:", err);
                aiResponse.response += "\n\n(Note: I tried to execute the action, but an error occurred.)";
            }
        }

        return aiResponse;
    } catch (error) {
        console.error('Chat error:', error);
        return { response: 'Error communicating with Gemini API. Check your console and API key.', action: { type: 'none' } };
    }
});

ipcMain.on('close-window', (event) => {
    if (mainWindow) {
        event.preventDefault();
        mainWindow.hide();
    }
});

ipcMain.on('maximize-window', (event) => {
    if (mainWindow) {
        if (mainWindow.isMaximized()) {
            mainWindow.unmaximize();
        } else {
            mainWindow.maximize();
        }
    }
});

ipcMain.handle('reset-chat', () => {
    resetChat();
    return { success: true };
});

ipcMain.handle('get-chat-history', () => {
    return getChatTranscript();
});

// =====================
// IPC Handlers — Settings
// =====================
ipcMain.handle('open-settings', () => {
    createSettingsWindow();
});

ipcMain.handle('get-settings', () => {
    return readConfig();
});

ipcMain.handle('save-settings', (event, config) => {
    const result = writeConfig(config);
    // 🔔 Notify main chat window immediately
    if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send('settings-changed', config);
    }
    // Restart proactive loop with new interval if it's running
    if (proactiveIntervalHandle) {
        startProactiveLoop(config.proactiveInterval);
    }
    return result;
});

ipcMain.on('close-settings', () => {
    if (settingsWindow) settingsWindow.close();
});

ipcMain.handle('open-external', (event, url) => {
    shell.openExternal(url);
});

// =====================
// IPC Handlers — Clipboard
// =====================
ipcMain.handle('clipboard-read', () => {
    return clipboard.readText();
});

ipcMain.handle('clipboard-write', (event, text) => {
    clipboard.writeText(text);
    return { success: true };
});

// =====================
// IPC Handlers — System Info
// =====================
ipcMain.handle('get-system-info', async () => {
    return getSystemInfo();
});

ipcMain.handle('get-weather', async (event, city) => {
    return getWeather(city);
});

// =====================
// IPC Handlers — Screen Vision
// =====================
ipcMain.handle('start-screen-capture', async () => {
    if (screenPickerWindow) return; // Prevent multiple windows

    try {
        // Capture primary display
        const primaryDisplay = screen.getPrimaryDisplay();
        const { width, height } = primaryDisplay.size;
        
        const sources = await desktopCapturer.getSources({
            types: ['screen'],
            thumbnailSize: { width, height }
        });
        
        // Find the full primary screen (usually "Screen 1" or "Entire screen")
        const primarySource = sources[0]; // Assuming single/primary monitor is the first

        // Create transparent, borderless screen picker window
        screenPickerWindow = new BrowserWindow({
            width, height,
            x: primaryDisplay.bounds.x, y: primaryDisplay.bounds.y,
            fullscreen: true,
            transparent: true,
            frame: false,
            alwaysOnTop: true,
            skipTaskbar: true,
            webPreferences: {
                preload: path.join(__dirname, 'preload-picker.js'),
                nodeIntegration: false,
                contextIsolation: true
            }
        });

        await screenPickerWindow.loadFile('src/UI/screenPicker.html');
        
        // Ensure image data isn't missing
        if(primarySource && primarySource.thumbnail) {
            screenPickerWindow.webContents.send('screenshot-data', primarySource.thumbnail.toDataURL());
        }

        screenPickerWindow.on('closed', () => { screenPickerWindow = null; });
    } catch (err) {
        console.error("Error starting screen capture:", err);
    }
});

// Received selection from the picker
ipcMain.on('vision-selection', (event, base64Image) => {
    if (screenPickerWindow) screenPickerWindow.close();
    
    // Relay image back to main window interface
    if (mainWindow) {
        mainWindow.webContents.send('vision-ready', base64Image);
        mainWindow.show(); // Bring chat back to focus
    }
});

ipcMain.on('vision-cancel', () => {
    if (screenPickerWindow) screenPickerWindow.close();
    if (mainWindow) mainWindow.show();
});

// =====================
// IPC Handlers — Proactive Assistant
// =====================
ipcMain.on('toggle-proactive', (event, isOn) => {
    if (isOn) {
        startProactiveLoop(); // reads interval from config automatically
    } else {
        stopProactiveLoop();
    }
});

ipcMain.on('record-behavior', (event, contextDescription) => {
    recordBehavior(contextDescription);
});

// Direct action executor for proactive Accept (no Gemini involved)
ipcMain.handle('execute-proactive-action', async (event, action) => {
    if (!action || action.type === 'none') {
        return { success: false, message: 'ไม่มี action ที่จะดำเนินการค่ะ' };
    }
    try {
        const result = await executeAction(action);
        // Build a friendly confirmation message
        const messages = {
            open_url:  `เปิดเว็บไซต์ให้แล้วค่ะ 🌐`,
            open_app:  `เปิดแอป ${action.target} ให้แล้วค่ะ 🚀`,
            search:    `ค้นหา "${action.target}" ให้แล้วค่ะ 🔍`,
            web_automation: result || 'ดำเนินการเสร็จแล้วค่ะ ✅',
            create_folder:  `สร้างโฟลเดอร์ "${action.target}" แล้วค่ะ 📁`,
            clipboard_write: `คัดลอกข้อความแล้วค่ะ 📋`,
            learn_file: result || `เรียนรู้เอกสารเรียบร้อยค่ะ 📚`,
        };
        return {
            success: true,
            message: messages[action.type] || 'ดำเนินการเสร็จแล้วค่ะ ✅'
        };
    } catch (err) {
        console.error('[ProactiveAction] Error:', err);
        return { success: false, message: `เกิดข้อผิดพลาด: ${err.message}` };
    }
});

ipcMain.handle('capture-silent-screen', async () => {
    try {
        const primaryDisplay = screen.getPrimaryDisplay();
        const { width, height } = primaryDisplay.size;
        
        const sources = await desktopCapturer.getSources({
            types: ['screen'],
            thumbnailSize: { width, height }
        });
        
        const primarySource = sources[0];
        if (primarySource && primarySource.thumbnail) {
            return primarySource.thumbnail.toDataURL();
        }
        return null;
    } catch (err) {
        console.error("Error silently capturing screen:", err);
        return null;
    }
});

// =====================
// Action Executor
// =====================
async function executeAction(action) {
    console.log("Executing action:", action);

    switch (action.type) {
        case 'open_url':
            openWebsite(action.target);
            break;
        case 'search':
            openSearch(action.target);
            break;
        case 'open_app':
            openApp(action.target);
            break;
        case 'web_automation':
            return await performWebAutomation(action.target);
        case 'create_folder':
            createFolder(action.target);
            break;
        case 'open_file':
            await openFile(action.target);
            break;
        case 'delete_file':
            await deleteFile(action.target);
            break;
        case 'clipboard_write':
            clipboard.writeText(action.target);
            break;
        case 'learn_file':
            return await indexFile(action.target);
        case 'plugin':
            return await pluginManager.executePlugin(action.pluginName, action.target);
    }
}
