import { app, BrowserWindow, ipcMain, shell, globalShortcut, clipboard, nativeImage } from 'electron'
import dotenv from 'dotenv'
import path from 'path'

dotenv.config()

// Linux docks match running windows to a .desktop file through WM_CLASS.
app.setName('Mint')
if (process.platform === 'linux') {
    app.commandLine.appendSwitch('class', 'Mint')
    if (typeof (app as any).setDesktopName === 'function') {
        (app as any).setDesktopName('mint-ai.desktop')
    }
}

import { getSystemInfo, getWeather } from '../System/system_info'
import { readConfig, writeConfig } from '../System/config_manager'
import { parseCommand } from '../Command_Parser/parser'
import { getGoogleTtsUrls } from '../System/google_tts_urls'
import { createWindowManager } from '../System/window_manager'
import { createProactiveLoop } from '../System/proactive_loop'
import { createScreenCaptureController } from '../System/screen_capture'
import { registerIpcHandlers } from '../System/ipc_handlers'
import { saveChatImages, listSavedPictures } from '../System/picture_store'

import systemEvents from '../System/system_events'
import customWorkflows from '../System/custom_workflows'
import mcpManager from '../Plugins/mcp_manager'

let geminiServices: any = null
function getGeminiServices() {
    if (!geminiServices) {
        geminiServices = require('../AI_Brain/Gemini_API')
    }
    return geminiServices
}

function getActionExecutor() {
    return require('../System/action_executor')
}

// In compiled electron-vite main bundle, __dirname is out/main/
// The project root is one level up from out/main/
const projectRoot = path.resolve(__dirname, '../..')
const windowManager = createWindowManager(projectRoot)

const proactiveLoop = createProactiveLoop({
    app,
    projectRoot,
    readConfig,
    getMainWindow: windowManager.getMainWindow
})

const screenCapture = createScreenCaptureController({
    projectRoot,
    translateImageContent: (...args: any[]) => getGeminiServices().translateImageContent(...args),
    getMainWindow: windowManager.getMainWindow
})

registerIpcHandlers({
    app,
    ipcMain,
    shell,
    clipboard,
    windowManager,
    proactiveLoop,
    screenCapture,
    services: {
        handleChat: (...args: any[]) => getGeminiServices().handleChat(...args),
        resetChat: (...args: any[]) => getGeminiServices().resetChat(...args),
        getChatTranscript: (...args: any[]) => getGeminiServices().getChatTranscript(...args),
        refreshApiKeyFromConfig: (...args: any[]) => getGeminiServices().refreshApiKeyFromConfig(...args),
        getSystemInfo,
        getWeather,
        readConfig,
        writeConfig,
        saveChatImages,
        listSavedPictures,
        parseCommand,
        executeAction: (...args: any[]) => getActionExecutor().executeAction(...args),
        getGoogleTtsUrls,
        customWorkflows
    }
})

app.whenReady().then(() => {
    const config = readConfig()
    const mainWindow = windowManager.createMainWindow()
    windowManager.createTray()

    mainWindow.once('ready-to-show', () => {
        if (config.showDesktopWidget !== false) {
            setTimeout(() => windowManager.createWidgetWindow(), 300)
        }

        setTimeout(() => {
            mcpManager.init().catch((err: any) => console.error('[MCP] Init Error:', err))

            const bridgeManager = require('../System/bridge_manager').default
            bridgeManager.init().catch((err: any) => console.error('[BridgeManager] Init Error:', err))
        }, 1000)
    })

    systemEvents.startMonitoring()
    if (config.enableCustomWorkflows !== false) {
        customWorkflows.startMonitoring(mainWindow.webContents)
    }

    systemEvents.on('low-battery', (level) => {
        const currentMainWindow = windowManager.getMainWindow()
        if (currentMainWindow && !currentMainWindow.isDestroyed()) {
            currentMainWindow.webContents.send('proactive-notification', {
                message: `⚠️ Battery is low (${level}%). Please plug in your charger. ✨`,
                type: 'warning'
            })
        }
    })

    systemEvents.on('connection-change', (isOnline) => {
        const currentMainWindow = windowManager.getMainWindow()
        if (currentMainWindow && !currentMainWindow.isDestroyed()) {
            const msg = isOnline ? '✅ Internet connection restored. ✨' : '❌ Internet connection lost.'
            currentMainWindow.webContents.send('proactive-notification', { message: msg, type: 'info' })
        }
    })

    globalShortcut.register('CommandOrControl+Shift+Space', windowManager.toggleMainWindow)
    globalShortcut.register('Alt+Space', windowManager.toggleSpotlightWindow)

    proactiveLoop.startIdleWatcher()

    app.on('activate', () => {
        if (BrowserWindow.getAllWindows().length === 0) {
            windowManager.createMainWindow()
        }
    })
})

app.on('window-all-closed', () => {
    // Keep app running by not calling app.quit()
})

app.on('will-quit', () => {
    globalShortcut.unregisterAll()
    mcpManager.shutdown()
})
