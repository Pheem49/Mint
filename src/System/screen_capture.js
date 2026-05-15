const { BrowserWindow, desktopCapturer, screen } = require('electron');
const path = require('path');

const TRANSLATE_REFRESH_MS = 3000;
const TRANSLATE_FAILURE_COOLDOWN_MS = 15000;

function createScreenCaptureController({ projectRoot, translateImageContent, getMainWindow }) {
    let screenPickerWindow = null;
    let translateIntervalHandle = null;
    let isTranslateRequestInFlight = false;
    let translateCooldownUntil = 0;

    async function startScreenCapture() {
        if (screenPickerWindow) return;

        try {
            const primaryDisplay = screen.getPrimaryDisplay();
            const { width, height } = primaryDisplay.size;
            const sources = await desktopCapturer.getSources({
                types: ['screen'],
                thumbnailSize: { width, height }
            });
            const primarySource = sources[0];

            screenPickerWindow = new BrowserWindow({
                width,
                height,
                x: primaryDisplay.bounds.x,
                y: primaryDisplay.bounds.y,
                fullscreen: true,
                transparent: true,
                frame: false,
                alwaysOnTop: true,
                skipTaskbar: true,
                webPreferences: {
                    preload: path.join(projectRoot, 'preload-picker.js'),
                    nodeIntegration: false,
                    contextIsolation: true
                }
            });

            await screenPickerWindow.loadFile(path.join(projectRoot, 'src/UI/screenPicker.html'));

            if (primarySource && primarySource.thumbnail) {
                screenPickerWindow.webContents.send('screenshot-data', primarySource.thumbnail.toDataURL());
            }

            screenPickerWindow.on('closed', () => { screenPickerWindow = null; });
        } catch (err) {
            console.error("Error starting screen capture:", err);
        }
    }

    function handleSelection(base64Image) {
        if (screenPickerWindow) screenPickerWindow.close();

        const mainWindow = getMainWindow();
        if (mainWindow) {
            mainWindow.webContents.send('vision-ready', base64Image);
            mainWindow.show();
        }
    }

    function startLiveTranslate(rect) {
        if (!screenPickerWindow) return;

        screenPickerWindow.setIgnoreMouseEvents(true, { forward: true });
        isTranslateRequestInFlight = false;
        translateCooldownUntil = 0;

        stopLiveTranslate(false);
        captureAndTranslate(rect);
        translateIntervalHandle = setInterval(() => {
            captureAndTranslate(rect);
        }, TRANSLATE_REFRESH_MS);
    }

    function stopLiveTranslate(resetMouseEvents = true) {
        if (translateIntervalHandle) {
            clearInterval(translateIntervalHandle);
            translateIntervalHandle = null;
        }
        if (resetMouseEvents && screenPickerWindow && !screenPickerWindow.isDestroyed()) {
            screenPickerWindow.setIgnoreMouseEvents(false);
        }
        isTranslateRequestInFlight = false;
        translateCooldownUntil = 0;
    }

    function setOverlayInteractable(isInteractable) {
        if (!screenPickerWindow || screenPickerWindow.isDestroyed()) return;
        screenPickerWindow.setIgnoreMouseEvents(!isInteractable, { forward: true });
    }

    async function captureAndTranslate(rect) {
        if (!screenPickerWindow || screenPickerWindow.isDestroyed()) return;
        if (isTranslateRequestInFlight) return;
        if (Date.now() < translateCooldownUntil) return;

        try {
            isTranslateRequestInFlight = true;
            const primaryDisplay = screen.getPrimaryDisplay();
            const sources = await desktopCapturer.getSources({
                types: ['screen'],
                thumbnailSize: {
                    width: primaryDisplay.size.width,
                    height: primaryDisplay.size.height
                }
            });

            if (sources.length > 0) {
                const croppedImage = sources[0].thumbnail.crop({
                    x: Math.round(rect.x),
                    y: Math.round(rect.y),
                    width: Math.round(rect.width),
                    height: Math.round(rect.height)
                });

                const base64Crop = croppedImage.toJPEG(70).toString('base64');
                const translationResult = await translateImageContent(`data:image/jpeg;base64,${base64Crop}`);
                if (translationResult.retryableFailure) {
                    translateCooldownUntil = Date.now() + TRANSLATE_FAILURE_COOLDOWN_MS;
                    console.warn(`Live translation cooldown active for ${TRANSLATE_FAILURE_COOLDOWN_MS / 1000}s after retryable API failure.`);
                } else {
                    translateCooldownUntil = 0;
                }

                if (screenPickerWindow && !screenPickerWindow.isDestroyed()) {
                    screenPickerWindow.webContents.send('vision-translate-result', translationResult.text);
                }
            }
        } catch (err) {
            console.error("Continuous translation loop failed:", err);
        } finally {
            isTranslateRequestInFlight = false;
        }
    }

    function cancel() {
        stopLiveTranslate(false);
        if (screenPickerWindow) screenPickerWindow.close();

        const mainWindow = getMainWindow();
        if (mainWindow) mainWindow.show();
    }

    async function captureSilentScreen() {
        try {
            const primaryDisplay = screen.getPrimaryDisplay();
            const { width, height } = primaryDisplay.size;
            const sources = await desktopCapturer.getSources({
                types: ['screen'],
                thumbnailSize: { width, height }
            });

            const primarySource = sources[0];
            return primarySource && primarySource.thumbnail ? primarySource.thumbnail.toDataURL() : null;
        } catch (err) {
            console.error("Error silently capturing screen:", err);
            return null;
        }
    }

    return {
        startScreenCapture,
        handleSelection,
        startLiveTranslate,
        stopLiveTranslate,
        setOverlayInteractable,
        cancel,
        captureSilentScreen
    };
}

module.exports = { createScreenCaptureController };
