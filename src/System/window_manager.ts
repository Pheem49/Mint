import { app, BrowserWindow, Tray, Menu, nativeImage, screen } from 'electron'
import path from 'path'

export function createWindowManager(projectRoot: string) {
    let mainWindow: BrowserWindow | null = null
    let settingsWindow: BrowserWindow | null = null
    let spotlightWindow: BrowserWindow | null = null
    let widgetWindow: BrowserWindow | null = null
    let tray: Tray | null = null

    const isDev = !app.isPackaged

    function createMainWindow() {
        const iconPath = path.join(projectRoot, 'assets', 'icon.png')
        const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize
        const windowWidth = Math.min(1600, Math.max(1280, screenWidth - 40))
        const windowHeight = Math.min(980, Math.max(800, screenHeight - 40))

        mainWindow = new BrowserWindow({
            width: windowWidth,
            height: windowHeight,
            minWidth: 1280,
            minHeight: 720,
            x: Math.floor((screenWidth - windowWidth) / 2),
            y: Math.floor((screenHeight - windowHeight) / 2),
            icon: nativeImage.createFromPath(iconPath),
            webPreferences: {
                preload: path.join(__dirname, '../preload/index.js'),
                nodeIntegration: false,
                contextIsolation: true,
            },
            frame: false,
            transparent: true,
            resizable: true,
            show: false
        })

        mainWindow.loadFile(path.join(projectRoot, 'src', 'UI', 'index.html'))

        mainWindow.on('ready-to-show', () => mainWindow?.show())
        mainWindow.on('close', (event) => {
            if (!(app as any).isQuiting) {
                event.preventDefault()
                mainWindow?.hide()
            }
            return false
        })
        mainWindow.on('closed', () => {
            mainWindow = null
        })

        return mainWindow
    }

    function createTray() {
        const iconPath = path.join(projectRoot, 'assets', 'icon.png')
        let icon = nativeImage.createFromPath(iconPath)
        icon = icon.resize({ width: 16, height: 16 })

        tray = new Tray(icon)
        tray.setToolTip('Mint AI Assistant')
        tray.setContextMenu(Menu.buildFromTemplate([
            { label: 'Show App', click: () => { if (mainWindow) mainWindow.show(); } },
            { label: 'Settings', click: () => { createSettingsWindow(); } },
            { type: 'separator' },
            {
                label: 'Quit',
                click: () => {
                    (app as any).isQuiting = true
                    app.quit()
                }
            }
        ]))

        tray.on('click', toggleMainWindow)
        return tray
    }

    function createSettingsWindow() {
        if (settingsWindow) {
            settingsWindow.focus()
            return settingsWindow
        }

        const iconPath = path.join(projectRoot, 'assets', 'icon.png')
        settingsWindow = new BrowserWindow({
            width: 1020,
            height: 720,
            minWidth: 860,
            minHeight: 620,
            icon: nativeImage.createFromPath(iconPath),
            webPreferences: {
                preload: path.join(__dirname, '../preload/settings.js'),
                nodeIntegration: false,
                contextIsolation: true,
            },
            frame: false,
            transparent: true,
            resizable: true,
            parent: mainWindow || undefined,
        })

        const settingsUrl = isDev && process.env['ELECTRON_RENDERER_URL']
            ? `${process.env['ELECTRON_RENDERER_URL']}#/settings`
            : `file://${path.join(__dirname, '../renderer/index.html')}#/settings`

        settingsWindow.loadURL(settingsUrl)
        settingsWindow.on('closed', () => { settingsWindow = null; })
        return settingsWindow
    }

    function createSpotlightWindow() {
        if (spotlightWindow) {
            spotlightWindow.show()
            return spotlightWindow
        }

        const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize
        const windowWidth = 600
        const windowHeight = 80

        spotlightWindow = new BrowserWindow({
            width: windowWidth,
            height: windowHeight,
            x: Math.floor((screenWidth - windowWidth) / 2),
            y: Math.floor(screenHeight * 0.25),
            frame: false,
            transparent: true,
            alwaysOnTop: true,
            skipTaskbar: true,
            show: false,
            webPreferences: {
                preload: path.join(__dirname, '../preload/spotlight.js'),
                nodeIntegration: false,
                contextIsolation: true,
            }
        })

        const spotlightUrl = isDev && process.env['ELECTRON_RENDERER_URL']
            ? `${process.env['ELECTRON_RENDERER_URL']}#/spotlight`
            : `file://${path.join(__dirname, '../renderer/index.html')}#/spotlight`

        spotlightWindow.loadURL(spotlightUrl)
        spotlightWindow.on('blur', () => spotlightWindow?.hide())
        spotlightWindow.on('closed', () => { spotlightWindow = null; })
        return spotlightWindow
    }

    function createWidgetWindow() {
        if (widgetWindow) return widgetWindow

        widgetWindow = new BrowserWindow({
            width: 150,
            height: 150,
            frame: false,
            transparent: true,
            resizable: false,
            alwaysOnTop: true,
            skipTaskbar: true,
            show: true,
            webPreferences: {
                preload: path.join(__dirname, '../preload/widget.js'),
                nodeIntegration: false,
                contextIsolation: true
            }
        })

        widgetWindow.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true })
        widgetWindow.setAlwaysOnTop(true, 'floating')

        try {
            const primaryDisplay = screen.getPrimaryDisplay()
            const { width, x, y } = primaryDisplay.workArea
            widgetWindow.setPosition(x + width - 150 - 40, y + 40)
        } catch (_) {}

        const widgetUrl = isDev && process.env['ELECTRON_RENDERER_URL']
            ? `${process.env['ELECTRON_RENDERER_URL']}#/widget`
            : `file://${path.join(__dirname, '../renderer/index.html')}#/widget`

        widgetWindow.loadURL(widgetUrl)
        widgetWindow.on('closed', () => { widgetWindow = null; })
        return widgetWindow
    }

    function toggleMainWindow() {
        if (!mainWindow) return
        if (mainWindow.isVisible()) {
            mainWindow.hide()
        } else {
            mainWindow.show()
        }
    }

    function toggleSpotlightWindow() {
        if (spotlightWindow && spotlightWindow.isVisible()) {
            spotlightWindow.hide()
            return
        }
        createSpotlightWindow().show()
    }

    function closeWidgetWindow() {
        if (widgetWindow && !widgetWindow.isDestroyed()) {
            widgetWindow.close()
            widgetWindow = null
        }
    }

    function ensureWidgetWindow() {
        if (!widgetWindow || widgetWindow.isDestroyed()) {
            createWidgetWindow()
        }
    }

    return {
        createMainWindow,
        createTray,
        createSettingsWindow,
        createSpotlightWindow,
        createWidgetWindow,
        toggleMainWindow,
        toggleSpotlightWindow,
        closeWidgetWindow,
        ensureWidgetWindow,
        getMainWindow: () => mainWindow,
        getSettingsWindow: () => settingsWindow,
        getSpotlightWindow: () => spotlightWindow,
        getWidgetWindow: () => widgetWindow
    }
}
