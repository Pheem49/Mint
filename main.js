const { app, BrowserWindow, ipcMain, shell, globalShortcut, clipboard, Tray, Menu, nativeImage, desktopCapturer, screen } = require('electron');
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

let mainWindow;
let settingsWindow = null;
let screenPickerWindow = null;
let tray = null;

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
        resizable: false,
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
    }
}
