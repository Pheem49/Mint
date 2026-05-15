const { app, BrowserWindow, ipcMain, shell, globalShortcut, clipboard } = require('electron');
require('dotenv').config();

const { handleChat, resetChat, getChatTranscript, translateImageContent, refreshApiKeyFromConfig } = require('./src/AI_Brain/Gemini_API');
const { getSystemInfo, getWeather } = require('./src/System/system_info');
const { readConfig, writeConfig } = require('./src/System/config_manager');
const { parseCommand } = require('./src/Command_Parser/parser');
const { executeAction } = require('./src/System/action_executor');
const { getGoogleTtsUrls } = require('./src/System/google_tts_urls');
const { createWindowManager } = require('./src/System/window_manager');
const { createProactiveLoop } = require('./src/System/proactive_loop');
const { createScreenCaptureController } = require('./src/System/screen_capture');
const { registerIpcHandlers } = require('./src/System/ipc_handlers');

const systemEvents = require('./src/System/system_events');
const customWorkflows = require('./src/System/custom_workflows');
const mcpManager = require('./src/Plugins/mcp_manager');

const projectRoot = __dirname;
const windowManager = createWindowManager(projectRoot);
const proactiveLoop = createProactiveLoop({
    app,
    projectRoot,
    readConfig,
    getMainWindow: windowManager.getMainWindow
});
const screenCapture = createScreenCaptureController({
    projectRoot,
    translateImageContent,
    getMainWindow: windowManager.getMainWindow
});

registerIpcHandlers({
    app,
    ipcMain,
    shell,
    clipboard,
    windowManager,
    proactiveLoop,
    screenCapture,
    services: {
        handleChat,
        resetChat,
        getChatTranscript,
        refreshApiKeyFromConfig,
        getSystemInfo,
        getWeather,
        readConfig,
        writeConfig,
        parseCommand,
        executeAction,
        getGoogleTtsUrls,
        customWorkflows
    }
});

app.whenReady().then(() => {
    const config = readConfig();
    const mainWindow = windowManager.createMainWindow();
    windowManager.createTray();

    if (config.showDesktopWidget !== false) {
        windowManager.createWidgetWindow();
    }

    mcpManager.init().catch(err => console.error('[MCP] Init Error:', err));

    const bridgeManager = require('./src/System/bridge_manager');
    bridgeManager.init().catch(err => console.error('[BridgeManager] Init Error:', err));

    systemEvents.startMonitoring();
    if (config.enableCustomWorkflows !== false) {
        customWorkflows.startMonitoring(mainWindow.webContents);
    }

    systemEvents.on('low-battery', (level) => {
        const currentMainWindow = windowManager.getMainWindow();
        if (currentMainWindow && !currentMainWindow.isDestroyed()) {
            currentMainWindow.webContents.send('proactive-notification', {
                message: `⚠️ Battery is low (${level}%). Please plug in your charger. ✨`,
                type: 'warning'
            });
        }
    });

    systemEvents.on('connection-change', (isOnline) => {
        const currentMainWindow = windowManager.getMainWindow();
        if (currentMainWindow && !currentMainWindow.isDestroyed()) {
            const msg = isOnline ? '✅ Internet connection restored. ✨' : '❌ Internet connection lost.';
            currentMainWindow.webContents.send('proactive-notification', { message: msg, type: 'info' });
        }
    });

    globalShortcut.register('CommandOrControl+Shift+Space', windowManager.toggleMainWindow);
    globalShortcut.register('Alt+Space', windowManager.toggleSpotlightWindow);

    proactiveLoop.startIdleWatcher();

    app.on('activate', () => {
        if (BrowserWindow.getAllWindows().length === 0) {
            windowManager.createMainWindow();
        }
    });
});

app.on('window-all-closed', (event) => {
    event.preventDefault();
});

app.on('will-quit', () => {
    globalShortcut.unregisterAll();
    mcpManager.shutdown();
});
