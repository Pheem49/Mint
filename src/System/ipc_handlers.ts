import * as safetyManager from './safety_manager'
import { getSmartContext  } from './smart_context'

export function buildApprovalRequest(action: any) {
    const classification = safetyManager.classifyAction(action)
    if (
        classification.tier !== safetyManager.TIERS.APPROVAL &&
        classification.tier !== safetyManager.TIERS.DANGEROUS
    ) {
        return null
    }

    return {
        required: true,
        tier: classification.tier,
        reason: classification.reason,
        action
    }
}

export async function executeApprovedAction(executeAction: any, action: any, clipboard: any) {
    const classification = safetyManager.classifyAction(action)
    const options = {
        clipboard,
        source: 'user_approved_action',
        allowApproval: classification.tier === safetyManager.TIERS.APPROVAL,
        allowDangerous: classification.tier === safetyManager.TIERS.DANGEROUS
    }
    const result = await executeAction(action, options)
    return {
        success: true,
        action,
        tier: classification.tier,
        result,
        message: result && typeof result === 'string'
            ? result
            : 'Action completed.'
    }
}

export function registerIpcHandlers({
    app,
    ipcMain,
    shell,
    clipboard,
    windowManager,
    proactiveLoop,
    screenCapture,
    services
}: any) {
    const {
        handleChat,
        resetChat,
        getChatTranscript,
        refreshApiKeyFromConfig,
        getSystemInfo,
        getWeather,
        readConfig,
        writeConfig,
        saveChatImages,
        listSavedPictures,
        parseCommand,
        executeAction,
        getGoogleTtsUrls,
        customWorkflows
    } = services

    ipcMain.handle('chat-message', async (event: any, message: string, base64Image: string | null = null, base64Audio: string | null = null) => {
        try {
            if (base64Image && saveChatImages) {
                saveChatImages(base64Image, { source: 'chat', message })
            }

            const rawResponse = await handleChat(message, base64Image, base64Audio)
            const aiResponse = parseCommand(rawResponse)

            if (aiResponse.action && aiResponse.action.type !== 'none') {
                try {
                    const approval = buildApprovalRequest(aiResponse.action)
                    if (approval) {
                        aiResponse.approval = approval
                        return aiResponse
                    }

                    const actionResult = await executeAction(aiResponse.action, { clipboard })
                    if (actionResult && typeof actionResult === 'string') {
                        aiResponse.response += `\n\n${actionResult}`
                    }
                } catch (err) {
                    console.error("Action execution error:", err)
                    aiResponse.response += "\n\n(Note: I tried to execute the action, but an error occurred.)"
                }
            }

            return aiResponse
        } catch (error) {
            console.error('Chat error:', error)
            return { response: 'Error communicating with Gemini API. Check your console and API key.', action: { type: 'none' } }
        }
    })

    ipcMain.handle('execute-approved-action', async (event: any, action: any) => {
        try {
            if (!action || action.type === 'none') {
                return { success: false, message: 'No action to execute.' }
            }
            return await executeApprovedAction(executeAction, action, clipboard)
        } catch (err: any) {
            console.error('[ApprovedAction] Error:', err)
            return { success: false, message: err.message || 'Action failed.' }
        }
    })

    ipcMain.on('close-window', () => {
        const mainWindow = windowManager.getMainWindow()
        if (mainWindow) mainWindow.hide()
    })

    ipcMain.on('minimize-window', () => {
        const mainWindow = windowManager.getMainWindow()
        if (mainWindow) mainWindow.minimize()
    })

    ipcMain.on('quit-app', () => {
        app.isQuiting = true
        app.quit()
    })

    ipcMain.on('maximize-window', () => {
        const mainWindow = windowManager.getMainWindow()
        if (!mainWindow) return
        if (mainWindow.isMaximized()) {
            mainWindow.unmaximize()
        } else {
            mainWindow.maximize()
        }
    })

    ipcMain.handle('reset-chat', () => {
        resetChat()
        return { success: true }
    })

    ipcMain.handle('get-chat-history', () => getChatTranscript())

    ipcMain.handle('list-saved-pictures', () => {
        return listSavedPictures ? listSavedPictures() : []
    })

    ipcMain.handle('open-settings', () => {
        windowManager.createSettingsWindow()
    })

    ipcMain.handle('get-settings', () => readConfig())

    ipcMain.handle('save-settings', (event: any, config: any) => {
        console.log('[Settings] Saving new config. MCP Servers count:', Object.keys(config.mcpServers || {}).length)
        const result = writeConfig(config)
        refreshApiKeyFromConfig()

        const mainWindow = windowManager.getMainWindow()
        if (mainWindow && !mainWindow.isDestroyed()) {
            mainWindow.webContents.send('settings-changed', config)
        }

        if (proactiveLoop.isRunning()) {
            proactiveLoop.start(config.proactiveInterval)
        }

        if (config.enableCustomWorkflows !== false) {
            customWorkflows.startMonitoring(mainWindow.webContents)
        } else {
            customWorkflows.stopMonitoring()
        }

        if (config.showDesktopWidget === false) {
            windowManager.closeWidgetWindow()
        } else {
            windowManager.ensureWidgetWindow()
        }

        return result
    })

    ipcMain.on('set-ai-state', (event: any, state: string) => {
        const widgetWindow = windowManager.getWidgetWindow()
        if (widgetWindow && !widgetWindow.isDestroyed()) {
            widgetWindow.webContents.send('widget-state', state)
        }
    })

    ipcMain.on('close-settings', () => {
        const settingsWindow = windowManager.getSettingsWindow()
        if (settingsWindow) settingsWindow.close()
    })

    ipcMain.handle('open-custom-workflows', () => {
        customWorkflows.openConfigFile()
    })

    ipcMain.handle('reload-custom-workflows', () => {
        customWorkflows.loadWorkflows()
        return { success: true }
    })

    ipcMain.on('spotlight-close', () => {
        const spotlightWindow = windowManager.getSpotlightWindow()
        if (spotlightWindow) spotlightWindow.close()
    })

    ipcMain.on('spotlight-hide', () => {
        const spotlightWindow = windowManager.getSpotlightWindow()
        if (spotlightWindow) spotlightWindow.hide()
    })

    ipcMain.on('spotlight-submit', async (event: any, query: string) => {
        console.log('[Spotlight] Submit:', query)
        const spotlightWindow = windowManager.getSpotlightWindow()
        if (spotlightWindow) spotlightWindow.hide()

        const mainWindow = windowManager.getMainWindow()
        if (mainWindow) {
            mainWindow.show()
            mainWindow.webContents.send('spotlight-to-chat', query)
        }
    })

    ipcMain.handle('spotlight-action', async (event: any, action: any) => {
        const spotlightWindow = windowManager.getSpotlightWindow()
        if (spotlightWindow) spotlightWindow.hide()

        if (!action || action.type === 'none') {
            return { success: false, message: 'No Spotlight action to execute.' }
        }

        try {
            const result = await executeAction(action, {
                clipboard,
                source: 'spotlight'
            })
            return {
                success: true,
                action,
                message: result && typeof result === 'string' ? result : 'Spotlight action completed.'
            }
        } catch (err: any) {
            console.error('[SpotlightAction] Error:', err)
            return { success: false, message: err.message || 'Spotlight action failed.' }
        }
    })

    ipcMain.on('spotlight-resize', (event: any, width: number, height: number) => {
        const spotlightWindow = windowManager.getSpotlightWindow()
        if (spotlightWindow) spotlightWindow.setSize(width, height)
    })

    ipcMain.handle('open-external', (event: any, url: string) => {
        shell.openExternal(url)
    })

    ipcMain.handle('clipboard-read', () => clipboard.readText())

    ipcMain.handle('clipboard-write', (event: any, text: string) => {
        clipboard.writeText(text)
        return { success: true }
    })

    ipcMain.handle('get-tts-urls', async (event: any, text: string) => {
        try {
            const isThai = /[\u0E00-\u0E7F]/.test(text)
            return getGoogleTtsUrls(text, {
                lang: isThai ? 'th' : 'en',
                host: 'https://translate.google.com',
            })
        } catch (e) {
            console.error("TTS Error:", e)
            return []
        }
    })

    ipcMain.handle('get-system-info', async () => getSystemInfo())
    ipcMain.handle('get-weather', async (event: any, city: string) => getWeather(city))

    ipcMain.handle('start-screen-capture', () => screenCapture.startScreenCapture())
    ipcMain.on('vision-selection', (event: any, base64Image: string) => screenCapture.handleSelection(base64Image))
    ipcMain.on('vision-translate-start', (event: any, rect: any) => screenCapture.startLiveTranslate(rect))
    ipcMain.on('vision-translate-stop', () => screenCapture.stopLiveTranslate())
    ipcMain.on('vision-overlay-interactable', (event: any, isInteractable: boolean) => screenCapture.setOverlayInteractable(isInteractable))
    ipcMain.on('vision-cancel', () => screenCapture.cancel())
    ipcMain.handle('capture-silent-screen', () => screenCapture.captureSilentScreen())

    ipcMain.handle('get-smart-context', () => getSmartContext({ clipboard }))

    ipcMain.on('toggle-proactive', (event: any, isOn: boolean) => {
        if (isOn) {
            proactiveLoop.start()
        } else {
            proactiveLoop.stop()
        }
    })

    ipcMain.on('record-behavior', (event: any, contextDescription: any) => {
        proactiveLoop.recordBehavior(contextDescription)
    })

    ipcMain.handle('execute-proactive-action', async (event: any, action: any) => {
        if (!action || action.type === 'none') {
            return { success: false, message: 'ไม่มี action ที่จะดำเนินการค่ะ' }
        }
        try {
            const result = await executeAction(action, { clipboard })
            const messages: Record<string, string> = {
                open_url: `เปิดเว็บไซต์ให้แล้วค่ะ 🌐`,
                open_app: `เปิดแอป ${action.target} ให้แล้วค่ะ 🚀`,
                search: `ค้นหา "${action.target}" ให้แล้วค่ะ 🔍`,
                web_automation: result || 'ดำเนินการเสร็จแล้วค่ะ ✅',
                create_folder: `สร้างโฟลเดอร์ "${action.target}" แล้วค่ะ 📁`,
                clipboard_write: `คัดลอกข้อความแล้วค่ะ 📋`,
                learn_file: result || `เรียนรู้เอกสารเรียบร้อยค่ะ 📚`,
            }
            return {
                success: true,
                message: messages[action.type] || 'ดำเนินการเสร็จแล้วค่ะ ✅'
            }
        } catch (err: any) {
            console.error('[ProactiveAction] Error:', err)
            return { success: false, message: `เกิดข้อผิดพลาด: ${err.message}` }
        }
    })
}
