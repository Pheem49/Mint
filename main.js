const { app, BrowserWindow, ipcMain, shell, globalShortcut } = require('electron');
const path = require('path');
require('dotenv').config();

const { handleChat } = require('./src/AI_Brain/Gemini_API');
const { openApp } = require('./src/Automation_Layer/open_app');
const { openWebsite, openSearch } = require('./src/Automation_Layer/open_website');
const { performWebAutomation } = require('./src/Automation_Layer/browser_automation');
const { parseCommand } = require('./src/Command_Parser/parser');

let mainWindow;
let tray = null;

function createWindow() {
    mainWindow = new BrowserWindow({
        width: 400,
        height: 600,
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



app.whenReady().then(() => {
    createWindow();

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

// IPC Handlers
ipcMain.handle('chat-message', async (event, message) => {
    try {
        const rawResponse = await handleChat(message);
        const aiResponse = parseCommand(rawResponse);

        // Execute action if present
        if (aiResponse.action && aiResponse.action.type !== 'none') {
            try {
                executeAction(aiResponse.action);
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
        event.preventDefault(); // Ensure it doesn't kill the process
        mainWindow.hide();
    }
});

function executeAction(action) {
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
            performWebAutomation(action.target);
            break;
    }
}
