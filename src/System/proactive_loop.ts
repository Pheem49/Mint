import { BrowserWindow, desktopCapturer, screen, powerMonitor } from 'electron'
import path from 'path'

const IDLE_THRESHOLD_SEC = 300

let proactiveEngine: any = null
function getProactiveEngine() {
    if (!proactiveEngine) {
        proactiveEngine = require('../AI_Brain/proactive_engine')
    }
    return proactiveEngine
}

let behaviorMemory: any = null
function getBehaviorMemory() {
    if (!behaviorMemory) {
        behaviorMemory = require('../AI_Brain/behavior_memory')
    }
    return behaviorMemory
}

export function createProactiveLoop({ app, projectRoot, readConfig, getMainWindow }: any) {
    let proactiveGlowWindow: BrowserWindow | null = null
    let proactiveIntervalHandle: NodeJS.Timeout | null = null
    let idleWatcherHandle: NodeJS.Timeout | null = null

    const isDev = !app.isPackaged

    async function runProactiveCycle() {
        const mainWindow = getMainWindow()
        if (!mainWindow || mainWindow.isDestroyed()) return

        try {
            showProactiveGlow()

            const primaryDisplay = screen.getPrimaryDisplay()
            const width = Math.floor(primaryDisplay.size.width * 0.5)
            const height = Math.floor(primaryDisplay.size.height * 0.5)
            const sources = await desktopCapturer.getSources({
                types: ['screen'],
                thumbnailSize: { width, height }
            })

            const primarySource = sources[0]
            if (!primarySource || !primarySource.thumbnail) return

            const base64Image = primarySource.thumbnail.toJPEG(60).toString('base64')
            const { analyzeAndSuggest } = getProactiveEngine()
            const { recordBehavior, getBehaviorSummary } = getBehaviorMemory()
            const result = await analyzeAndSuggest(base64Image, getBehaviorSummary())

            if (result && result.message && Array.isArray(result.suggestions)) {
                if (result.context) recordBehavior(result.context)

                const currentMainWindow = getMainWindow()
                if (currentMainWindow && !currentMainWindow.isDestroyed()) {
                    currentMainWindow.webContents.send('proactive-suggestion', result)
                }
            }

            hideProactiveGlow()
        } catch (err: any) {
            console.error('[Proactive] Cycle error:', err.message)
            hideProactiveGlow()
        }
    }

    function createProactiveGlowWindow() {
        if (proactiveGlowWindow) return proactiveGlowWindow
        const { width, height } = screen.getPrimaryDisplay().bounds

        proactiveGlowWindow = new BrowserWindow({
            width,
            height,
            x: 0,
            y: 0,
            frame: false,
            transparent: true,
            alwaysOnTop: true,
            skipTaskbar: true,
            focusable: false,
            show: false,
            webPreferences: {
                nodeIntegration: false,
                contextIsolation: true
            }
        })

        proactiveGlowWindow.setIgnoreMouseEvents(true)
        proactiveGlowWindow.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true })
        proactiveGlowWindow.setAlwaysOnTop(true, 'screen-saver')

        const glowUrl = isDev && process.env['ELECTRON_RENDERER_URL']
            ? `${process.env['ELECTRON_RENDERER_URL']}#/proactive-glow`
            : `file://${path.join(__dirname, '../renderer/index.html')}#/proactive-glow`

        proactiveGlowWindow.loadURL(glowUrl)
        proactiveGlowWindow.on('closed', () => { proactiveGlowWindow = null; })
        return proactiveGlowWindow
    }

    function showProactiveGlow() {
        if (!proactiveGlowWindow) createProactiveGlowWindow()
        if (proactiveGlowWindow) proactiveGlowWindow.showInactive()
    }

    function hideProactiveGlow() {
        if (proactiveGlowWindow && !proactiveGlowWindow.isDestroyed()) {
            proactiveGlowWindow.hide()
        }
    }

    function start(intervalSec?: number) {
        stop()
        const cfg = readConfig()
        const ms = (intervalSec || cfg.proactiveInterval || 60) * 1000
        console.log(`[Proactive] Starting loop — interval: ${ms / 1000}s`)
        proactiveIntervalHandle = setInterval(runProactiveCycle, ms)
    }

    function stop() {
        if (proactiveIntervalHandle) {
            clearInterval(proactiveIntervalHandle)
            proactiveIntervalHandle = null
            console.log('[Proactive] Stopped proactive loop.')
        }
    }

    function startIdleWatcher() {
        if (idleWatcherHandle) return
        idleWatcherHandle = setInterval(() => {
            if (!proactiveIntervalHandle) return
            if (!app.isReady()) return

            const idleSec = powerMonitor.getSystemIdleTime()
            if (idleSec < IDLE_THRESHOLD_SEC) return

            console.log(`[System Idle] User idle for ${idleSec}s. Pausing Proactive loop to save resources.`)
            stop()

            const resumeChecker = setInterval(() => {
                if (powerMonitor.getSystemIdleTime() < 10) {
                    console.log('[System Idle] User returned. Resuming Proactive loop.')
                    clearInterval(resumeChecker)
                    start()
                }
            }, 5000)
        }, 60000)
    }

    return {
        start,
        stop,
        startIdleWatcher,
        isRunning: () => Boolean(proactiveIntervalHandle),
        recordBehavior: (...args: any[]) => getBehaviorMemory().recordBehavior(...args)
    }
}
