const bgCanvas = document.getElementById('bg-canvas');
const overlayCanvas = document.getElementById('overlay-canvas');
const bgCtx = bgCanvas.getContext('2d');
const overlayCtx = overlayCanvas.getContext('2d');

let isDrawing = false;
let startX = 0;
let startY = 0;
let currentX = 0;
let currentY = 0;
let baseImage = null;
let selectedRect = null;
let isOverlayInteractable = true;

let isTranslateMode = false;
let isContinuousTranslateActive = false;
const translateBtn = document.getElementById('btn-translate');
const hintText = document.getElementById('hint-text');
const translationBox = document.getElementById('translation-box');
const translationContent = document.getElementById('translation-content');
const btnCloseTranslate = document.getElementById('btn-close-translate');
const toolbar = document.getElementById('toolbar');

// Initialize canvases
function init() {
    bgCanvas.width = window.innerWidth;
    bgCanvas.height = window.innerHeight;
    overlayCanvas.width = window.innerWidth;
    overlayCanvas.height = window.innerHeight;

    // Wait for the main process to send the screenshot
    window.electronPicker.onScreenshot((base64Data) => {
        baseImage = new Image();
        baseImage.onload = () => {
            bgCtx.drawImage(baseImage, 0, 0, bgCanvas.width, bgCanvas.height);
            drawDarkOverlay();
        };
        baseImage.src = base64Data;
    });
}

function drawDarkOverlay() {
    overlayCtx.clearRect(0, 0, overlayCanvas.width, overlayCanvas.height);
    overlayCtx.fillStyle = 'rgba(0, 0, 0, 0.5)';
    overlayCtx.fillRect(0, 0, overlayCanvas.width, overlayCanvas.height);
}

function normalizeRect(rect) {
    return {
        x: Math.min(rect.startX, rect.currentX),
        y: Math.min(rect.startY, rect.currentY),
        width: Math.abs(rect.width),
        height: Math.abs(rect.height)
    };
}

function drawSelectionOutline(rect) {
    if (!rect || rect.width === 0 || rect.height === 0) return;

    overlayCtx.clearRect(0, 0, overlayCanvas.width, overlayCanvas.height);
    overlayCtx.strokeStyle = isTranslateMode ? '#8b5cf6' : '#00ff88';
    overlayCtx.lineWidth = 3;
    overlayCtx.strokeRect(rect.x, rect.y, rect.width, rect.height);
}

function drawSelection() {
    drawDarkOverlay();

    const rect = normalizeRect({
        startX,
        startY,
        currentX,
        currentY,
        width: currentX - startX,
        height: currentY - startY
    });

    overlayCtx.clearRect(rect.x, rect.y, rect.width, rect.height);
    overlayCtx.strokeStyle = isTranslateMode ? '#8b5cf6' : '#00ff88';
    overlayCtx.lineWidth = 2;
    overlayCtx.strokeRect(rect.x, rect.y, rect.width, rect.height);
}

function resetSelectionOverlay() {
    selectedRect = null;
    overlayCanvas.style.pointerEvents = 'auto';
    isOverlayInteractable = true;
    drawDarkOverlay();
}

function setOverlayInteractable(isInteractable) {
    if (isOverlayInteractable === isInteractable) return;
    isOverlayInteractable = isInteractable;
    window.electronPicker.setOverlayInteractable(isInteractable);
}

function stopTranslationMode({ keepTranslateMode = false } = {}) {
    isContinuousTranslateActive = false;
    isTranslateMode = keepTranslateMode;

    translationBox.style.display = 'none';
    toolbar.style.display = 'flex';
    overlayCanvas.style.pointerEvents = 'auto';
    translateBtn.classList.toggle('active', keepTranslateMode);
    translateBtn.innerText = keepTranslateMode ? 'Stop Translate' : '🌐 Live Translate';
    hintText.innerText = keepTranslateMode ? 'Drag over text to translate to Thai' : 'Click and drag to select a region';

    window.electronPicker.stopContinuousTranslation();
    setOverlayInteractable(true);

    if (baseImage) {
        bgCtx.clearRect(0, 0, bgCanvas.width, bgCanvas.height);
        bgCtx.drawImage(baseImage, 0, 0, bgCanvas.width, bgCanvas.height);
    }

    resetSelectionOverlay();
}

function setTranslationBoxPosition(rect) {
    const margin = 10;
    const boxWidth = Math.min(400, Math.max(240, rect.width));
    translationBox.style.maxWidth = `${boxWidth}px`;
    translationBox.style.left = `${Math.max(margin, Math.min(rect.x, window.innerWidth - boxWidth - margin))}px`;

    const preferredTop = rect.y + rect.height + margin;
    const fallbackTop = Math.max(margin, rect.y - margin - translationBox.offsetHeight);
    const top = preferredTop + translationBox.offsetHeight <= window.innerHeight
        ? preferredTop
        : fallbackTop;

    translationBox.style.top = `${top}px`;
}

function cropAndSend(rect) {
    if (rect.width === 0 || rect.height === 0) return;

    const { x, y, width: w, height: h } = normalizeRect(rect);

    const cropCanvas = document.createElement('canvas');
    cropCanvas.width = w;
    cropCanvas.height = h;
    const cropCtx = cropCanvas.getContext('2d');
    
    cropCtx.drawImage(
        baseImage, 
        x, y, w, h, 
        0, 0, w, h
    );

    const croppedBase64 = cropCanvas.toDataURL('image/png');
    
    if (isTranslateMode) {
        isContinuousTranslateActive = true;
        selectedRect = { x, y, width: w, height: h };

        toolbar.style.display = 'none';
        bgCtx.clearRect(0, 0, bgCanvas.width, bgCanvas.height);
        overlayCanvas.style.pointerEvents = 'none';
        drawSelectionOutline(selectedRect);

        translationBox.style.display = 'block';
        translationContent.innerHTML = '<span class="loading-spinner"></span> Auto-Translating...';
        setTranslationBoxPosition(selectedRect);
        setOverlayInteractable(false);

        window.electronPicker.startContinuousTranslation({ x, y, width: w, height: h });
    } else {
        window.electronPicker.sendSelection(croppedBase64);
    }
}

// Listen for continuous translation results from main process
window.electronPicker.onTranslationResult((thaiText) => {
    if (isContinuousTranslateActive && translationBox.style.display === 'block') {
        translationContent.innerHTML = thaiText;
    }
});

// Stop continuous translation
btnCloseTranslate.addEventListener('click', (e) => {
    e.stopPropagation();
    stopTranslationMode();
});

window.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || !isContinuousTranslateActive) return;
    e.preventDefault();
    stopTranslationMode();
});

window.addEventListener('mousemove', (e) => {
    if (!isContinuousTranslateActive || translationBox.style.display !== 'block') return;

    const rect = translationBox.getBoundingClientRect();
    const isInsideBox = e.clientX >= rect.left
        && e.clientX <= rect.right
        && e.clientY >= rect.top
        && e.clientY <= rect.bottom;

    setOverlayInteractable(isInsideBox);
});

// Mouse Events
overlayCanvas.addEventListener('mousedown', (e) => {
    if (isContinuousTranslateActive) return;
    isDrawing = true;
    startX = e.clientX;
    startY = e.clientY;
    currentX = e.clientX;
    currentY = e.clientY;
});

overlayCanvas.addEventListener('mousemove', (e) => {
    if (!isDrawing) return;
    currentX = e.clientX;
    currentY = e.clientY;
    drawSelection();
});

overlayCanvas.addEventListener('mouseup', (e) => {
    if (!isDrawing) return;
    currentX = e.clientX;
    currentY = e.clientY;
    isDrawing = false;
    cropAndSend({ startX, startY, currentX, currentY, width: currentX - startX, height: currentY - startY });
});

// UI Buttons
translateBtn.addEventListener('click', () => {
    isTranslateMode = !isTranslateMode;
    if (isTranslateMode) {
        translateBtn.classList.add('active');
        translateBtn.innerText = 'Stop Translate';
        hintText.innerText = 'Drag over text to translate to Thai';
        translationBox.style.display = 'none';
        resetSelectionOverlay();
    } else {
        if (isContinuousTranslateActive) {
            stopTranslationMode();
            return;
        }
        translateBtn.classList.remove('active');
        translateBtn.innerText = '🌐 Live Translate';
        hintText.innerText = 'Click and drag to select a region';
        translationBox.style.display = 'none';
        resetSelectionOverlay();
    }
});

document.getElementById('btn-fullscreen').addEventListener('click', () => {
    if (baseImage && !isTranslateMode) {
        window.electronPicker.sendSelection(baseImage.src);
    }
});

document.getElementById('btn-cancel').addEventListener('click', () => {
    window.electronPicker.closePicker();
});

// Setup
window.addEventListener('resize', init);
init();
