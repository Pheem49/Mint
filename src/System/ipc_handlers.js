function registerIpcHandlers({
    app,
    ipcMain,
    shell,
    clipboard,
    windowManager,
    proactiveLoop,
    screenCapture,
    services
}) {
    const {
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
    } = services;

    ipcMain.handle('chat-message', async (event, message, base64Image = null, base64Audio = null) => {
        try {
            const rawResponse = await handleChat(message, base64Image, base64Audio);
            const aiResponse = parseCommand(rawResponse);

            if (aiResponse.action && aiResponse.action.type !== 'none') {
                try {
                    const actionResult = await executeAction(aiResponse.action, { clipboard });
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

    ipcMain.on('close-window', () => {
        const mainWindow = windowManager.getMainWindow();
        if (mainWindow) mainWindow.hide();
    });

    ipcMain.on('minimize-window', () => {
        const mainWindow = windowManager.getMainWindow();
        if (mainWindow) mainWindow.minimize();
    });

    ipcMain.on('quit-app', () => {
        app.isQuiting = true;
        app.quit();
    });

    ipcMain.on('maximize-window', () => {
        const mainWindow = windowManager.getMainWindow();
        if (!mainWindow) return;
        if (mainWindow.isMaximized()) {
            mainWindow.unmaximize();
        } else {
            mainWindow.maximize();
        }
    });

    ipcMain.handle('reset-chat', () => {
        resetChat();
        return { success: true };
    });

    ipcMain.handle('get-chat-history', () => getChatTranscript());

    ipcMain.handle('open-settings', () => {
        windowManager.createSettingsWindow();
    });

    ipcMain.handle('get-settings', () => readConfig());

    ipcMain.handle('save-settings', (event, config) => {
        console.log('[Settings] Saving new config. MCP Servers count:', Object.keys(config.mcpServers || {}).length);
        const result = writeConfig(config);
        refreshApiKeyFromConfig();

        const mainWindow = windowManager.getMainWindow();
        if (mainWindow && !mainWindow.isDestroyed()) {
            mainWindow.webContents.send('settings-changed', config);
        }

        if (proactiveLoop.isRunning()) {
            proactiveLoop.start(config.proactiveInterval);
        }

        if (config.enableCustomWorkflows !== false) {
            customWorkflows.startMonitoring(mainWindow.webContents);
        } else {
            customWorkflows.stopMonitoring();
        }

        if (config.showDesktopWidget === false) {
            windowManager.closeWidgetWindow();
        } else {
            windowManager.ensureWidgetWindow();
        }

        return result;
    });

    ipcMain.on('set-ai-state', (event, state) => {
        const widgetWindow = windowManager.getWidgetWindow();
        if (widgetWindow && !widgetWindow.isDestroyed()) {
            widgetWindow.webContents.send('widget-state', state);
        }
    });

    ipcMain.on('close-settings', () => {
        const settingsWindow = windowManager.getSettingsWindow();
        if (settingsWindow) settingsWindow.close();
    });

    ipcMain.handle('open-custom-workflows', () => {
        customWorkflows.openConfigFile();
    });

    ipcMain.handle('reload-custom-workflows', () => {
        customWorkflows.loadWorkflows();
        return { success: true };
    });

    ipcMain.on('spotlight-close', () => {
        const spotlightWindow = windowManager.getSpotlightWindow();
        if (spotlightWindow) spotlightWindow.close();
    });

    ipcMain.on('spotlight-hide', () => {
        const spotlightWindow = windowManager.getSpotlightWindow();
        if (spotlightWindow) spotlightWindow.hide();
    });

    ipcMain.on('spotlight-submit', async (event, query) => {
        console.log('[Spotlight] Submit:', query);
        const spotlightWindow = windowManager.getSpotlightWindow();
        if (spotlightWindow) spotlightWindow.hide();

        const mainWindow = windowManager.getMainWindow();
        if (mainWindow) {
            mainWindow.show();
            mainWindow.webContents.send('spotlight-to-chat', query);
        }
    });

    ipcMain.on('spotlight-resize', (event, width, height) => {
        const spotlightWindow = windowManager.getSpotlightWindow();
        if (spotlightWindow) spotlightWindow.setSize(width, height);
    });

    ipcMain.handle('open-external', (event, url) => {
        shell.openExternal(url);
    });

    ipcMain.handle('clipboard-read', () => clipboard.readText());

    ipcMain.handle('clipboard-write', (event, text) => {
        clipboard.writeText(text);
        return { success: true };
    });

    ipcMain.handle('get-tts-urls', async (event, text) => {
        try {
            const isThai = /[\u0E00-\u0E7F]/.test(text);
            return getGoogleTtsUrls(text, {
                lang: isThai ? 'th' : 'en',
                host: 'https://translate.google.com',
            });
        } catch (e) {
            console.error("TTS Error:", e);
            return [];
        }
    });

    ipcMain.handle('get-system-info', async () => getSystemInfo());
    ipcMain.handle('get-weather', async (event, city) => getWeather(city));

    ipcMain.handle('start-screen-capture', () => screenCapture.startScreenCapture());
    ipcMain.on('vision-selection', (event, base64Image) => screenCapture.handleSelection(base64Image));
    ipcMain.on('vision-translate-start', (event, rect) => screenCapture.startLiveTranslate(rect));
    ipcMain.on('vision-translate-stop', () => screenCapture.stopLiveTranslate());
    ipcMain.on('vision-overlay-interactable', (event, isInteractable) => screenCapture.setOverlayInteractable(isInteractable));
    ipcMain.on('vision-cancel', () => screenCapture.cancel());
    ipcMain.handle('capture-silent-screen', () => screenCapture.captureSilentScreen());

    ipcMain.on('toggle-proactive', (event, isOn) => {
        if (isOn) {
            proactiveLoop.start();
        } else {
            proactiveLoop.stop();
        }
    });

    ipcMain.on('record-behavior', (event, contextDescription) => {
        proactiveLoop.recordBehavior(contextDescription);
    });

    ipcMain.handle('execute-proactive-action', async (event, action) => {
        if (!action || action.type === 'none') {
            return { success: false, message: 'ไม่มี action ที่จะดำเนินการค่ะ' };
        }
        try {
            const result = await executeAction(action, { clipboard });
            const messages = {
                open_url: `เปิดเว็บไซต์ให้แล้วค่ะ 🌐`,
                open_app: `เปิดแอป ${action.target} ให้แล้วค่ะ 🚀`,
                search: `ค้นหา "${action.target}" ให้แล้วค่ะ 🔍`,
                web_automation: result || 'ดำเนินการเสร็จแล้วค่ะ ✅',
                create_folder: `สร้างโฟลเดอร์ "${action.target}" แล้วค่ะ 📁`,
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
}

module.exports = { registerIpcHandlers };
