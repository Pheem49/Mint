const { app, BrowserWindow, Tray, Menu, nativeImage, screen } = require('electron');
const path = require('path');

function createWindowManager(projectRoot) {
    let mainWindow = null;
    let settingsWindow = null;
    let spotlightWindow = null;
    let widgetWindow = null;
    let tray = null;

    function createMainWindow() {
        mainWindow = new BrowserWindow({
            width: 600,
            height: 800,
            icon: path.join(projectRoot, 'assets', 'icon.png'),
            webPreferences: {
                preload: path.join(projectRoot, 'preload.js'),
                nodeIntegration: false,
                contextIsolation: true,
            },
            frame: false,
            transparent: true,
            resizable: true,
            show: false
        });

        mainWindow.loadFile(path.join(projectRoot, 'src/UI/index.html'));
        mainWindow.on('ready-to-show', () => mainWindow.show());
        mainWindow.on('close', (event) => {
            if (!app.isQuiting) {
                event.preventDefault();
                mainWindow.hide();
            }
            return false;
        });
        mainWindow.on('focus', () => {
            // clearFloatingUnread(); // Disabled
        });
        mainWindow.on('closed', () => {
            mainWindow = null;
        });

        return mainWindow;
    }

    function createTray() {
        const iconPath = path.join(projectRoot, 'assets', 'icon.png');
        let icon = nativeImage.createFromPath(iconPath);
        icon = icon.resize({ width: 16, height: 16 });

        tray = new Tray(icon);
        tray.setToolTip('Mint AI Assistant');
        tray.setContextMenu(Menu.buildFromTemplate([
            { label: 'Show App', click: () => { if (mainWindow) mainWindow.show(); } },
            { label: 'Settings', click: () => { createSettingsWindow(); } },
            { type: 'separator' },
            {
                label: 'Quit',
                click: () => {
                    app.isQuiting = true;
                    app.quit();
                }
            }
        ]));

        tray.on('click', toggleMainWindow);
        return tray;
    }

    function createSettingsWindow() {
        if (settingsWindow) {
            settingsWindow.focus();
            return settingsWindow;
        }

        settingsWindow = new BrowserWindow({
            width: 720,
            height: 620,
            minWidth: 640,
            minHeight: 560,
            icon: path.join(projectRoot, 'assets', 'icon.png'),
            webPreferences: {
                preload: path.join(projectRoot, 'preload-settings.js'),
                nodeIntegration: false,
                contextIsolation: true,
            },
            frame: false,
            transparent: true,
            resizable: true,
            parent: mainWindow,
        });
        settingsWindow.loadFile(path.join(projectRoot, 'src/UI/settings.html'));
        settingsWindow.on('closed', () => { settingsWindow = null; });
        return settingsWindow;
    }

    function createSpotlightWindow() {
        if (spotlightWindow) {
            spotlightWindow.show();
            return spotlightWindow;
        }

        const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize;
        const windowWidth = 600;
        const windowHeight = 80;

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
                preload: path.join(projectRoot, 'src/UI/preload-spotlight.js'),
                nodeIntegration: false,
                contextIsolation: true,
            }
        });

        spotlightWindow.loadFile(path.join(projectRoot, 'src/UI/spotlight.html'));
        spotlightWindow.on('blur', () => spotlightWindow.hide());
        spotlightWindow.on('closed', () => { spotlightWindow = null; });
        return spotlightWindow;
    }

    function createWidgetWindow() {
        if (widgetWindow) return widgetWindow;

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
                preload: path.join(projectRoot, 'src/UI/preload-widget.js'),
                nodeIntegration: false,
                contextIsolation: true
            }
        });

        widgetWindow.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
        widgetWindow.setAlwaysOnTop(true, 'floating');

        try {
            const primaryDisplay = screen.getPrimaryDisplay();
            const { width, x, y } = primaryDisplay.workArea;
            widgetWindow.setPosition(x + width - 150 - 40, y + 40);
        } catch (_) {}

        widgetWindow.loadFile(path.join(projectRoot, 'src/UI/widget.html'));
        widgetWindow.on('closed', () => { widgetWindow = null; });
        return widgetWindow;
    }

    function toggleMainWindow() {
        if (!mainWindow) return;
        if (mainWindow.isVisible()) {
            mainWindow.hide();
        } else {
            mainWindow.show();
        }
    }

    function toggleSpotlightWindow() {
        if (spotlightWindow && spotlightWindow.isVisible()) {
            spotlightWindow.hide();
            return;
        }
        createSpotlightWindow().show();
    }

    function closeWidgetWindow() {
        if (widgetWindow && !widgetWindow.isDestroyed()) {
            widgetWindow.close();
            widgetWindow = null;
        }
    }

    function ensureWidgetWindow() {
        if (!widgetWindow || widgetWindow.isDestroyed()) {
            createWidgetWindow();
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
    };
}

module.exports = { createWindowManager };
