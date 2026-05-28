const chatContainer = document.getElementById('chat-container');
const chatForm = document.getElementById('chat-form');
const chatInput = document.getElementById('chat-input');
const closeBtn = document.getElementById('close-btn');
const maximizeBtn = document.getElementById('maximize-btn');
const minimizeBtn = document.getElementById('minimize-btn');
const clearBtn = document.getElementById('clear-btn');
const settingsBtn = document.getElementById('settings-btn');
const sidebarNewChatBtn = document.getElementById('sidebar-new-chat');
const sidebarSettingsBtn = document.getElementById('sidebar-settings');
const sidebarToggleBtn = document.getElementById('sidebar-toggle');
const appBody = document.querySelector('.app-body');
const sidebarChatBtn = document.getElementById('sidebar-chat-btn');
const sidebarPicturesBtn = document.getElementById('sidebar-pictures-btn');
const picturesLibrary = document.getElementById('pictures-library');
const picturesGrid = document.getElementById('pictures-grid');
const picturesEmpty = document.getElementById('pictures-empty');
const picturesCloseBtn = document.getElementById('pictures-close-btn');
const micBtn = document.getElementById('mic-btn');
const visionBtn = document.getElementById('vision-btn');
const chatProviderSelect = document.getElementById('chat-provider-select');
const imagePreviewContainer = document.getElementById('image-preview-container');
const imagePreview = document.getElementById('image-preview');
const removeImageBtn = document.getElementById('remove-image-btn');
const agentModeToggle = document.getElementById('agent-mode-toggle');
const modelMount = document.getElementById('model-mount');
const modelShell = document.getElementById('model-shell');
const modelStatus = document.getElementById('model-status');
const mintStatus = document.getElementById('mint-status');
const mintStatusLabel = document.getElementById('mint-status-label');
const modelActivityBadge = document.getElementById('model-activity-badge');
const startupLoading = document.getElementById('startup-loading');
const appContainer = document.querySelector('.app-container');

if (startupLoading) {
    startupLoading.style.background = 'var(--bg-gradient)';
    startupLoading.style.color = 'var(--text-muted)';
}

// Proactive Assistant elements
const proactiveBar = document.getElementById('proactive-bar');
const proactiveMessage = document.getElementById('proactive-message');
const proactiveChips = document.getElementById('proactive-chips');
const proactiveDismissBtn = document.getElementById('proactive-dismiss-btn');

let currentBase64Image = null;
let enableVoiceReply = true;
let ttsProvider = 'google';
let ttsVolume = 1.0;
let ttsSpeed = 1.0;
let ttsPitch = 1.0;
let lastConversationLanguage = 'auto';
let mintActivityResetTimer = null;
let currentSettings = {};

const PROVIDER_PICKER_OPTIONS = [
    ['gemini', 'Gemini'],
    ['anthropic', 'Claude'],
    ['openai', 'OpenAI'],
    ['ollama', 'Ollama'],
    ['huggingface', 'Hugging Face'],
    ['local_openai', 'Local']
];

function buildProviderPicker(settings = currentSettings) {
    if (!chatProviderSelect) return;
    chatProviderSelect.textContent = '';
    PROVIDER_PICKER_OPTIONS.forEach(([value, label]) => {
        const option = document.createElement('option');
        option.value = value;
        option.textContent = label;
        chatProviderSelect.appendChild(option);
    });
    chatProviderSelect.value = settings.aiProvider || 'gemini';
}

function syncAgentModeToggle(settings = currentSettings) {
    if (!agentModeToggle) return;
    agentModeToggle.checked = settings.assistantMode === 'agent';
    agentModeToggle.closest('.smart-context-control')?.classList.toggle('is-active', agentModeToggle.checked);
}

async function changeChatProvider(provider) {
    if (!PROVIDER_PICKER_OPTIONS.some(([value]) => value === provider)) return;
    const nextSettings = { ...currentSettings, aiProvider: provider };
    chatProviderSelect.disabled = true;
    try {
        const result = await window.api.saveSettings(nextSettings);
        if (!result || result.success !== false) {
            currentSettings = nextSettings;
            buildProviderPicker(currentSettings);
        } else {
            throw new Error(result.message || 'Unable to save provider setting');
        }
    } catch (error) {
        console.error('Failed to change provider:', error);
        buildProviderPicker(currentSettings);
        setMintActivity('error');
    } finally {
        chatProviderSelect.disabled = false;
    }
}

const MINT_ACTIVITY_STATES = {
    idle: { label: 'Idle', title: 'Mint is idle' },
    listening: { label: 'Listening', title: 'Mint is listening' },
    thinking: { label: 'Thinking', title: 'Mint is thinking' },
    speaking: { label: 'Speaking', title: 'Mint is speaking' },
    error: { label: 'Error', title: 'Mint needs attention' }
};

function setMintActivity(state, options = {}) {
    const normalizedState = MINT_ACTIVITY_STATES[state] ? state : 'idle';
    const meta = MINT_ACTIVITY_STATES[normalizedState];
    if (mintActivityResetTimer) {
        clearTimeout(mintActivityResetTimer);
        mintActivityResetTimer = null;
    }

    [mintStatus, modelActivityBadge].forEach((element) => {
        if (!element) return;
        element.dataset.state = normalizedState;
        element.title = meta.title;
        const label = element.querySelector('.mint-status-label');
        if (label) label.textContent = meta.label;
    });
    if (mintStatusLabel) mintStatusLabel.textContent = meta.label;

    if (window.api && window.api.setAiState) {
        window.api.setAiState(normalizedState);
    }

    if (normalizedState === 'error' || options.resetAfter) {
        mintActivityResetTimer = setTimeout(() => {
            setMintActivity('idle');
        }, options.resetAfter || 3500);
    }
}

function detectConversationLanguage(text) {
    const value = String(text || '');
    if (/[\u0E00-\u0E7F]/.test(value)) return 'thai';
    if (/[A-Za-z]/.test(value)) return 'english';
    return 'auto';
}

function rememberConversationLanguage(text) {
    const detected = detectConversationLanguage(text);
    if (detected !== 'auto') {
        lastConversationLanguage = detected;
    }
}

function buildInteractionLanguageInstruction() {
    if (lastConversationLanguage === 'thai') {
        return 'Current conversation language: Thai. Reply in Thai. Do not reply in English just because this interaction instruction is written in English.';
    }
    if (lastConversationLanguage === 'english') {
        return 'Current conversation language: English. Reply in English. Do not switch to Thai.';
    }
    return 'Infer the reply language from the recent conversation before this interaction instruction, not from the language of this instruction.';
}

// --- Theme Loading ---
function applyTheme(theme, accentColor, systemTextColor, config = {}) {
    document.documentElement.setAttribute('data-theme', theme || 'dark');
    const accent = accentColor || '#8f6cf5';
    const defaultTextColor = theme === 'light' ? '#0f172a' : '#e8e8ea';
    const textColor = (!systemTextColor || (theme === 'light' && systemTextColor === '#f8fafc'))
        ? defaultTextColor
        : systemTextColor;
    document.documentElement.style.setProperty('--accent', accent);
    document.documentElement.style.setProperty('--accent-hover', lightenColor(accent, 20));
    document.documentElement.style.setProperty('--text-main', textColor);

    // Dynamic UI Customizations
    document.documentElement.style.setProperty('--glass-blur', config.glassBlur || 'blur(16px)');
    document.body.style.fontFamily = config.fontFamily || "'Outfit', sans-serif";
    document.documentElement.style.fontSize = config.fontSize || '15px';

    if (theme === 'custom') {
        if (config.customBgStart && config.customBgEnd) {
            const gradient = `linear-gradient(135deg, ${config.customBgStart} 0%, ${config.customBgEnd} 100%)`;
            document.documentElement.style.setProperty('--bg-color', config.customBgStart);
            document.documentElement.style.setProperty('--bg-gradient', gradient);
        }
        if (config.customPanelBg) {
            const rgb = hexToRgb(config.customPanelBg);
            document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`);
            document.documentElement.style.setProperty('--panel-raised', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.82)`);
            document.documentElement.style.setProperty('--panel-soft', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.46)`);
            document.documentElement.style.setProperty('--chrome-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.88)`);
            document.documentElement.style.setProperty('--surface-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.62)`);
            document.documentElement.style.setProperty('--surface-strong', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.86)`);
            document.documentElement.style.setProperty('--input-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.72)`);
        }
    } else {
        [
            '--bg-color',
            '--bg-gradient',
            '--panel-bg',
            '--panel-raised',
            '--panel-soft',
            '--chrome-bg',
            '--surface-bg',
            '--surface-strong',
            '--input-bg'
        ].forEach(name => document.documentElement.style.removeProperty(name));
    }
}

function hexToRgb(hex) {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : { r: 15, g: 23, b: 42 };
}

async function loadTheme() {
    try {
        const config = await window.api.getSettings();
        currentSettings = config || {};
        applyTheme(config.theme, config.accentColor, config.systemTextColor, config);
        enableVoiceReply = config.enableVoiceReply !== false;
        ttsProvider = config.ttsProvider || 'google';
        ttsVolume = config.ttsVolume !== undefined ? config.ttsVolume : 1.0;
        ttsSpeed = config.ttsSpeed !== undefined ? config.ttsSpeed : 1.0;
        ttsPitch = config.ttsPitch !== undefined ? config.ttsPitch : 1.0;
        buildProviderPicker(currentSettings);
        syncAgentModeToggle(currentSettings);
    } catch (e) {
        applyTheme('dark', '#8b5cf6', '#f8fafc');
        buildProviderPicker(currentSettings);
        syncAgentModeToggle(currentSettings);
    }
}

function lightenColor(hex, amount) {
    const clean = hex.replace('#', '');
    if (clean.length !== 6) return hex;
    const num = parseInt(clean, 16);
    const r = Math.min(255, (num >> 16) + amount);
    const g = Math.min(255, ((num >> 8) & 0x00FF) + amount);
    const b = Math.min(255, (num & 0x0000FF) + amount);
    return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

// 🔔 Real-time theme sync from Settings window
window.api.onSettingsChanged((config) => {
    currentSettings = config || currentSettings;
    applyTheme(config.theme, config.accentColor, config.systemTextColor, config);
    enableVoiceReply = config.enableVoiceReply !== false;
    ttsProvider = config.ttsProvider || 'google';
    ttsVolume = config.ttsVolume !== undefined ? config.ttsVolume : 1.0;
    ttsSpeed = config.ttsSpeed !== undefined ? config.ttsSpeed : 1.0;
    ttsPitch = config.ttsPitch !== undefined ? config.ttsPitch : 1.0;
    buildProviderPicker(currentSettings);
    syncAgentModeToggle(currentSettings);
});

chatProviderSelect?.addEventListener('change', (event) => {
    changeChatProvider(event.target.value);
});

agentModeToggle?.addEventListener('change', async () => {
    const nextSettings = {
        ...currentSettings,
        assistantMode: agentModeToggle.checked ? 'agent' : 'chat'
    };
    agentModeToggle.disabled = true;
    try {
        const result = await window.api.saveSettings(nextSettings);
        if (!result || result.success !== false) {
            currentSettings = nextSettings;
        } else {
            throw new Error(result.message || 'Unable to save assistant mode');
        }
    } catch (error) {
        console.error('Failed to change assistant mode:', error);
        setMintActivity('error');
    } finally {
        syncAgentModeToggle(currentSettings);
        agentModeToggle.disabled = false;
    }
});

// --- Voice Input Setup ---
let mediaRecorder = null;
let audioChunks = [];
let speechRecognition = null;
let isSpeechStreaming = false;
let speechInterim = '';
let speechHadResult = false;
let speechFallbackTimer = null;
let voiceMode = null; // 'speech' | 'recorder' | null
let voiceSendQueue = Promise.resolve();
let speechPausedForReply = false;
let resumeSpeechAfterResponse = false;
const DEFAULT_PLACEHOLDER = "Type or speak a command...";
const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;

function notifyAiIfNeeded() {
    if (!window.api.notifyAiResponse) return;
    if (!document.hasFocus() || document.hidden) {
        window.api.notifyAiResponse();
    } else if (window.api.clearAiNotifications) {
        window.api.clearAiNotifications();
    }
}

function queueVoiceTextSend(text) {
    const clean = (text || '').trim();
    if (!clean) return;
    voiceSendQueue = voiceSendQueue.then(() => sendTextMessage(clean, { allowSmartContext: false }));
}

function pauseSpeechForReply() {
    if (!speechRecognition || !isSpeechStreaming) return;
    resumeSpeechAfterResponse = true;
    speechPausedForReply = true;
    try {
        speechRecognition.stop();
    } catch (_) {}
}

function resumeSpeechIfNeeded() {
    if (!speechRecognition || !isSpeechStreaming) {
        resumeSpeechAfterResponse = false;
        speechPausedForReply = false;
        return;
    }
    if (!resumeSpeechAfterResponse) return;
    resumeSpeechAfterResponse = false;
    speechPausedForReply = false;
    try {
        speechRecognition.start();
    } catch (e) {
        console.error("Speech recognition resume error:", e);
    }
}

function setupSpeechRecognition() {
    if (!SpeechRecognitionCtor) return;
    speechRecognition = new SpeechRecognitionCtor();
    speechRecognition.lang = 'th-TH';
    speechRecognition.interimResults = true;
    // Let the engine auto-stop on silence, then we restart if streaming is enabled.
    speechRecognition.continuous = false;

    speechRecognition.onstart = () => {
        micBtn.classList.add('listening');
        chatInput.placeholder = "Listening... (Click to stop)";
        setMintActivity('listening');
        speechHadResult = false;
        if (speechFallbackTimer) clearTimeout(speechFallbackTimer);
        speechFallbackTimer = setTimeout(() => {
            if (isSpeechStreaming && !speechHadResult) {
                fallbackToMediaRecorder();
            }
        }, 1500);
    };

    speechRecognition.onresult = (event) => {
        speechHadResult = true;
        let interimTranscript = '';
        let finalTranscript = '';

        for (let i = event.resultIndex; i < event.results.length; i++) {
            const result = event.results[i];
            const transcript = result[0]?.transcript || '';
            if (result.isFinal) {
                finalTranscript += transcript;
            } else {
                interimTranscript += transcript;
            }
        }

        if (finalTranscript.trim()) {
            const textToSend = finalTranscript.trim();
            speechInterim = '';
            chatInput.value = '';
            pauseSpeechForReply();
            queueVoiceTextSend(textToSend);
        } else {
            speechInterim = interimTranscript;
            chatInput.value = speechInterim.trimStart();
        }
    };

    speechRecognition.onerror = (err) => {
        console.error("Speech recognition error:", err);
        setMintActivity('error');
        fallbackToMediaRecorder();
        isSpeechStreaming = false;
        resetMicUI();
    };

    speechRecognition.onend = () => {
        if (speechFallbackTimer) {
            clearTimeout(speechFallbackTimer);
            speechFallbackTimer = null;
        }
        if (speechPausedForReply) {
            return;
        }
        if (isSpeechStreaming && !speechHadResult) {
            fallbackToMediaRecorder();
            return;
        }
        if (isSpeechStreaming) {
            try {
                speechRecognition.start();
            } catch (e) {
                console.error("Speech recognition restart error:", e);
                isSpeechStreaming = false;
                resetMicUI();
            }
        } else {
            resetMicUI();
        }
    };
}

async function setupMediaRecorder() {
    try {
        // Improved audio constraints for better quality and noise reduction
        const stream = await navigator.mediaDevices.getUserMedia({ 
            audio: {
                echoCancellation: true,
                noiseSuppression: true,
                autoGainControl: true
            } 
        });

        // Check for supported MIME types
        const mimeType = MediaRecorder.isTypeSupported('audio/webm') ? 'audio/webm' : 'audio/mp4';
        mediaRecorder = new MediaRecorder(stream, { mimeType });

        mediaRecorder.ondataavailable = (event) => {
            if (event.data.size > 0) audioChunks.push(event.data);
        };

        mediaRecorder.onstop = async () => {
            if (audioChunks.length === 0) {
                resetMicUI();
                return;
            }

            const audioBlob = new Blob(audioChunks, { type: mimeType });
            audioChunks = [];
            
            // Convert Blob to Base64
            const reader = new FileReader();
            reader.readAsDataURL(audioBlob);
            reader.onloadend = async () => {
                const base64Audio = reader.result;
                // Send to Gemini
                await sendVoiceMessage(base64Audio);
            };
        };

        mediaRecorder.onstart = () => {
            micBtn.classList.add('listening');
            chatInput.placeholder = "Listening... (Click to stop)";
            setMintActivity('listening');
        };

    } catch (err) {
        console.error("Microphone access error:", err);
        setMintActivity('error');
        micBtn.style.display = 'none';
        appendMessage("❌ ไม่สามารถเข้าถึงไมโครโฟนได้ค่ะ กรุณาตรวจสอบการตั้งค่าระดับระบบ", 'ai');
    }
}

function resetMicUI() {
    micBtn.classList.remove('listening');
    chatInput.placeholder = DEFAULT_PLACEHOLDER;
    if (voiceMode !== 'speech' && (!mediaRecorder || mediaRecorder.state === 'inactive')) {
        setMintActivity('idle');
    }
}

async function sendVoiceMessage(base64Audio) {
    showTyping();
    chatInput.placeholder = "Processing voice...";
    setMintActivity('thinking');
    try {
        // Send empty text, but include the audio
        const response = await window.api.sendMessage("", null, base64Audio);
        removeTyping();
        
        // Show AI response
        const msgDiv = await appendAiMessages(response.response, { 
            allowDelay: true, 
            timestamp: new Date().toISOString() 
        });
        await speakText(normalizeAiText(response.response), { onEnd: resumeSpeechIfNeeded });
        notifyAiIfNeeded();

        if (response.approval?.required) {
            appendApprovalCard(msgDiv, response.approval);
        } else if (response.action && response.action.type !== 'none') {
            appendActionCard(msgDiv, response.action);
        }
    } catch (error) {
        removeTyping();
        setMintActivity('error');
        appendMessage("ขออภัยค่ะ เกิดข้อผิดพลาดในการประมวลผลเสียง", 'ai');
        console.error(error);
        resumeSpeechIfNeeded();
    } finally {
        resetMicUI();
    }
}

function fallbackToMediaRecorder() {
    if (voiceMode === 'recorder') return;
    isSpeechStreaming = false;
    speechPausedForReply = false;
    resumeSpeechAfterResponse = false;
    voiceMode = 'recorder';
    try {
        if (speechRecognition) {
            speechRecognition.stop();
        }
    } catch (_) {}
    if (mediaRecorder && mediaRecorder.state === 'inactive') {
        audioChunks = [];
        mediaRecorder.start();
    }
}

// Initialize voice input
setupMediaRecorder();
if (SpeechRecognitionCtor) {
    setupSpeechRecognition();
}

micBtn.addEventListener('click', (e) => {
    e.preventDefault();
    if (voiceMode === 'recorder') {
        if (!mediaRecorder) return;
        if (mediaRecorder.state === 'inactive') {
            audioChunks = [];
            mediaRecorder.start();
            setMintActivity('listening');
        } else {
            mediaRecorder.stop();
            setMintActivity('thinking');
            voiceMode = null;
        }
        return;
    }

    if (speechRecognition) {
        if (!isSpeechStreaming) {
            isSpeechStreaming = true;
            voiceMode = 'speech';
            speechInterim = '';
            chatInput.value = '';
            try {
                speechRecognition.start();
            } catch (err) {
                console.error("Speech recognition start error:", err);
                isSpeechStreaming = false;
                resetMicUI();
            }
        } else {
            isSpeechStreaming = false;
            speechRecognition.stop();
            voiceMode = null;
        }
        return;
    }

    if (!mediaRecorder) return;

    if (mediaRecorder.state === 'inactive') {
        audioChunks = [];
        mediaRecorder.start();
        setMintActivity('listening');
    } else {
        mediaRecorder.stop();
        setMintActivity('thinking');
    }
});

// --- Speech Synthesis Setup ---
let currentAudioPlayer = null;

function speakText(text, options = {}) {
    setMintActivity('speaking');
    const onEnd = typeof options.onEnd === 'function' ? options.onEnd : () => {};
    
    const wrappedOnEnd = () => {
        if (window.Live2DManager) Live2DManager.stopLipSync();
        onEnd();
    };

    return new Promise(async (resolve) => {
        if (!enableVoiceReply) {
            setMintActivity('idle');
            wrappedOnEnd();
            return resolve();
        }

        // Stop any currently playing audio
        if (currentAudioPlayer) {
            currentAudioPlayer.pause();
            currentAudioPlayer.currentTime = 0;
            currentAudioPlayer = null;
        }
        if (window.Live2DManager) Live2DManager.stopLipSync();

        if ('speechSynthesis' in window) {
            window.speechSynthesis.cancel();
        }

        if (!text || !text.trim()) {
            setMintActivity('idle');
            wrappedOnEnd();
            return resolve();
        }

        if (window.Live2DManager) Live2DManager.startLipSync();

        try {
            if (ttsProvider !== 'native') {
                const urls = await window.api.getTtsUrls(text);
                if (urls && urls.length > 0) {
                    let i = 0;
                    const playNext = () => {
                        if (i >= urls.length) {
                            setMintActivity('idle');
                            wrappedOnEnd();
                            return resolve();
                        }
                        const audio = new Audio(urls[i].url);
                        audio.volume = ttsVolume;
                        audio.playbackRate = ttsSpeed;

                        currentAudioPlayer = audio;
                        audio.onended = () => {
                            i++;
                            playNext();
                        };
                        audio.onerror = () => {
                            console.error("TTS Audio error", urls[i]);
                            i++;
                            playNext();
                        };
                        audio.play().catch(e => {
                            console.error("Audio playback prevented:", e);
                            fallbackSpeak(text, wrappedOnEnd, resolve);
                        });
                    };
                    playNext();
                    return;
                }
            }
        } catch (err) {
            console.error("Cloud TTS Error, falling back to local:", err);
        }

        // Fallback
        fallbackSpeak(text, wrappedOnEnd, resolve);
    });
}

function fallbackSpeak(text, onEnd, resolve) {
    if (!('speechSynthesis' in window)) {
        setMintActivity('idle');
        if (onEnd) onEnd();
        resolve();
        return;
    }

    window.speechSynthesis.cancel();
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.lang = 'th-TH'; 
    utterance.volume = ttsVolume;
    utterance.rate = ttsSpeed;
    utterance.pitch = ttsPitch;
    
    let finished = false;
    const done = () => {
        if (finished) return;
        finished = true;
        setMintActivity('idle');
        if (onEnd) onEnd();
        resolve();
    };

    utterance.onend = done;
    utterance.onerror = done;
    window.speechSynthesis.speak(utterance);
}

// Minimize window handler (hides to tray)
minimizeBtn.addEventListener('click', () => {
    window.api.minimizeWindow();
});

// Close window handler (quits app)
closeBtn.addEventListener('click', () => {
    window.api.quitApp();
});

maximizeBtn.addEventListener('click', () => {
    window.api.maximizeWindow();
});

// Settings button
function openSettings() {
    window.api.openSettings();
}

settingsBtn.addEventListener('click', openSettings);
sidebarSettingsBtn?.addEventListener('click', openSettings);

async function renderPicturesLibrary() {
    if (!picturesGrid || !picturesEmpty) return;
    picturesGrid.innerHTML = '';

    const pictures = await window.api.listSavedPictures();
    picturesEmpty.classList.toggle('is-hidden', pictures.length > 0);

    for (const picture of pictures) {
        const card = document.createElement('article');
        card.className = 'picture-card';

        const img = document.createElement('img');
        img.src = picture.url;
        img.alt = picture.filename || 'Saved picture';
        img.loading = 'lazy';

        const meta = document.createElement('div');
        meta.className = 'picture-card-meta';
        const date = picture.createdAt ? new Date(picture.createdAt).toLocaleString() : '';
        meta.textContent = picture.message || date || picture.filename || 'Saved picture';
        meta.title = [picture.filename, picture.message, date].filter(Boolean).join('\n');

        card.appendChild(img);
        card.appendChild(meta);
        picturesGrid.appendChild(card);
    }
}

async function openPicturesLibrary() {
    if (!appBody || !picturesLibrary) return;
    picturesLibrary.hidden = false;
    requestAnimationFrame(() => {
        appBody.classList.add('pictures-open');
    });
    sidebarChatBtn?.classList.remove('is-active');
    sidebarPicturesBtn?.classList.add('is-active');
    await renderPicturesLibrary();
}

function closePicturesLibrary() {
    if (!appBody || !picturesLibrary) return;
    appBody.classList.remove('pictures-open');
    setTimeout(() => {
        if (!appBody.classList.contains('pictures-open')) {
            picturesLibrary.hidden = true;
        }
    }, 240);
    sidebarChatBtn?.classList.add('is-active');
    sidebarPicturesBtn?.classList.remove('is-active');
}

sidebarChatBtn?.addEventListener('click', closePicturesLibrary);
sidebarPicturesBtn?.addEventListener('click', openPicturesLibrary);
picturesCloseBtn?.addEventListener('click', closePicturesLibrary);

function setSidebarCollapsed(isCollapsed) {
    if (!appBody || !sidebarToggleBtn) return;
    appBody.classList.toggle('sidebar-collapsed', isCollapsed);
    sidebarToggleBtn.setAttribute('aria-expanded', String(!isCollapsed));
    sidebarToggleBtn.setAttribute('aria-label', isCollapsed ? 'Expand sidebar' : 'Collapse sidebar');
    sidebarToggleBtn.setAttribute('title', isCollapsed ? 'Expand sidebar' : 'Collapse sidebar');
}

if (appBody && sidebarToggleBtn) {
    setSidebarCollapsed(true);
    sidebarToggleBtn.addEventListener('click', () => {
        setSidebarCollapsed(!appBody.classList.contains('sidebar-collapsed'));
    });
}

// Throttle utility to prevent UI spam
function throttle(func, limit) {
    let inThrottle;
    return function() {
        const args = arguments;
        const context = this;
        if (!inThrottle) {
            func.apply(context, args);
            inThrottle = true;
            setTimeout(() => inThrottle = false, limit);
        }
    }
}

// Vision system
visionBtn.addEventListener('click', throttle(async () => {
    await window.api.startVision();
}, 1000));

window.api.onVisionReady((base64Image) => {
    currentBase64Image = base64Image;
    imagePreview.src = base64Image;
    imagePreviewContainer.style.display = 'block';
    chatInput.focus();
});

removeImageBtn.addEventListener('click', () => {
    currentBase64Image = null;
    imagePreview.src = '';
    imagePreviewContainer.style.display = 'none';
});

function formatTime(isoString) {
    if (!isoString) return '';
    try {
        const date = new Date(isoString);
        return date.toLocaleTimeString('th-TH', { hour: '2-digit', minute: '2-digit', hour12: false });
    } catch (e) {
        return '';
    }
}

function compactSmartContext(context) {
    if (!context || typeof context !== 'object') return null;
    const activeWindow = context.activeWindow || {};
    const currentApp = context.currentApp || {};
    const browser = context.browser || null;
    return {
        capturedAt: context.capturedAt,
        platform: context.platform,
        currentApp: currentApp.name || activeWindow.appName || activeWindow.processName || '',
        processName: currentApp.processName || activeWindow.processName || '',
        pid: currentApp.pid || activeWindow.pid || null,
        activeWindowTitle: activeWindow.title || '',
        browser: browser ? {
            title: browser.title || '',
            url: browser.url || '',
            urlUnavailableReason: browser.urlUnavailableReason || ''
        } : null,
        selectedText: context.selectedText || '',
        clipboardText: context.clipboardText || ''
    };
}

function appendSmartContextToMessage(message, context) {
    const compact = compactSmartContext(context);
    if (!compact) return message;
    return [
        message,
        '',
        '[SMART_CONTEXT]',
        'Use this structured desktop context together with the attached screenshot. Do not mention it unless it helps answer the user.',
        JSON.stringify(compact, null, 2),
        '[/SMART_CONTEXT]'
    ].join('\n');
}

function shouldShowAgentActivity(options = {}) {
    return options.showAgentActivity !== false && currentSettings.assistantMode === 'agent';
}

function createAgentActivityCard() {
    const messageDiv = document.createElement('div');
    messageDiv.classList.add('message', 'ai-message', 'agent-activity-message');

    const bubble = document.createElement('div');
    bubble.classList.add('message-bubble', 'agent-activity-card');

    const header = document.createElement('div');
    header.className = 'agent-activity-header';
    const title = document.createElement('span');
    title.textContent = 'Agent Activity';
    const status = document.createElement('span');
    status.className = 'agent-activity-status';
    status.textContent = 'Running';
    header.appendChild(title);
    header.appendChild(status);

    const list = document.createElement('div');
    list.className = 'agent-activity-list';

    bubble.appendChild(header);
    bubble.appendChild(list);
    messageDiv.appendChild(bubble);
    chatContainer.appendChild(messageDiv);
    scrollToBottom();

    return {
        element: messageDiv,
        list,
        status,
        add(label, state = 'running', detail = '') {
            const item = document.createElement('div');
            item.className = 'agent-activity-item';
            item.dataset.state = state;

            const dot = document.createElement('span');
            dot.className = 'agent-activity-dot';

            const content = document.createElement('span');
            content.className = 'agent-activity-text';
            content.textContent = detail ? `${label}: ${detail}` : label;

            item.appendChild(dot);
            item.appendChild(content);
            list.appendChild(item);
            scrollToBottom();
            return item;
        },
        update(item, state, label, detail = '') {
            if (!item) return;
            item.dataset.state = state;
            const content = item.querySelector('.agent-activity-text');
            if (content && label) {
                content.textContent = detail ? `${label}: ${detail}` : label;
            }
        },
        finish(state = 'done', label = 'Done') {
            status.textContent = label;
            status.dataset.state = state;
        }
    };
}

function describeSmartContextActivity(context, hasScreenshot) {
    const compact = compactSmartContext(context) || {};
    const parts = [];
    if (hasScreenshot) parts.push('screen');
    if (compact.currentApp) parts.push(compact.currentApp);
    if (compact.activeWindowTitle) parts.push(compact.activeWindowTitle);
    if (compact.selectedText) parts.push('selected text');
    if (compact.clipboardText) parts.push('clipboard');
    return parts.slice(0, 3).join(' · ') || 'desktop context';
}

function describeActionActivity(action) {
    if (!action || action.type === 'none') return 'No desktop action';
    const meta = getActionCardMeta(action);
    return meta.detail ? `${meta.title} · ${meta.detail}` : meta.title;
}

// Clear chat history
async function clearChatHistory(confirmMessage = 'Clear current chat history?') {
    const shouldClear = window.confirm(confirmMessage);
    if (!shouldClear) return;

    closePicturesLibrary();
    await window.api.resetChat();
    // Remove all messages except the initial greeting
    const messages = chatContainer.querySelectorAll('.message:not(.initial)');
    messages.forEach(m => m.remove());
    // Append a clear confirmation
    appendMessage('Chat history cleared. Starting fresh! 🌿', 'ai', null, new Date().toISOString());
}

clearBtn.addEventListener('click', () => clearChatHistory('Clear current chat history?'));
sidebarNewChatBtn?.addEventListener('click', () => clearChatHistory('Start a new chat and clear current history?'));

function formatProviderInfo(providerInfo) {
    if (!providerInfo || typeof providerInfo !== 'object') return '';
    const provider = String(providerInfo.provider || '').trim();
    const model = String(providerInfo.model || '').trim();
    if (!provider && !model) return '';
    return model ? `${provider || 'AI'} • ${model}` : provider;
}

function formatNumber(value) {
    const number = Number(value) || 0;
    return number.toLocaleString('en-US');
}

function summarizeProviderUsage(providerInfo) {
    const usage = Array.isArray(providerInfo?.usage) ? providerInfo.usage : [];
    const selectedProvider = String(providerInfo?.provider || '').trim();
    const selectedModel = String(providerInfo?.model || '').trim();
    const row = usage.find(item =>
        String(item.provider || '') === selectedProvider &&
        String(item.model || '') === selectedModel
    ) || usage[0] || {};

    return {
        requests: Number(row.requests) || 0,
        inputTokens: Number(row.inputTokens) || 0,
        outputTokens: Number(row.outputTokens) || 0,
        reasoningTokens: Number(row.reasoningTokens) || 0,
        cacheReads: Number(row.cacheReads) || 0,
        totalTokens: Number(row.totalTokens) || 0
    };
}

function closeProviderPopover() {
    document.querySelectorAll('.provider-popover').forEach(popover => popover.remove());
    document.querySelectorAll('.provider-badge.is-open').forEach(badge => badge.classList.remove('is-open'));
}

function createProviderRow(label, value) {
    const row = document.createElement('div');
    row.className = 'provider-popover-row';
    const labelEl = document.createElement('span');
    labelEl.textContent = label;
    const valueEl = document.createElement('strong');
    valueEl.textContent = value;
    row.appendChild(labelEl);
    row.appendChild(valueEl);
    return row;
}

function showProviderPopover(anchor, providerInfo) {
    closeProviderPopover();
    anchor.classList.add('is-open');

    const provider = String(providerInfo?.provider || 'AI').trim();
    const model = String(providerInfo?.model || 'Unknown model').trim();
    const usage = summarizeProviderUsage(providerInfo);
    const popover = document.createElement('div');
    popover.className = 'provider-popover';

    const title = document.createElement('div');
    title.className = 'provider-popover-title';
    title.textContent = 'Model details';
    popover.appendChild(title);

    popover.appendChild(createProviderRow('Provider', provider));
    popover.appendChild(createProviderRow('Model', model));
    popover.appendChild(createProviderRow('Context tokens', formatNumber(usage.inputTokens)));
    popover.appendChild(createProviderRow('Output tokens', formatNumber(usage.outputTokens)));
    if (usage.reasoningTokens) {
        popover.appendChild(createProviderRow('Reasoning tokens', formatNumber(usage.reasoningTokens)));
    }
    popover.appendChild(createProviderRow('Total tokens', formatNumber(usage.totalTokens)));

    const action = document.createElement('button');
    action.type = 'button';
    action.className = 'provider-popover-action';
    action.textContent = 'Change model in Settings';
    action.addEventListener('click', (event) => {
        event.stopPropagation();
        closeProviderPopover();
        if (window.api?.openSettings) window.api.openSettings();
    });
    popover.appendChild(action);

    anchor.after(popover);
}

function splitListOutro(text) {
    const value = String(text || '').trim();
    const markers = [
        ' คุณภีมอยาก',
        ' อยากให้',
        ' อยากดู',
        ' บอกมิ้นท์',
        ' Would you',
        ' Do you want',
        ' Tell me'
    ];

    for (const marker of markers) {
        const index = value.indexOf(marker);
        if (index > 60) {
            return {
                main: value.slice(0, index).trim(),
                outro: value.slice(index).trim()
            };
        }
    }

    return { main: value, outro: '' };
}

function buildAiTextBlocks(text) {
    const normalized = normalizeAiText(text).replace(/\r\n/g, '\n').trim();
    if (!normalized) return [];

    const readable = normalized
        .replace(/\s+(\d+)[.)]\s+/g, '\n$1. ')
        .replace(/\n{3,}/g, '\n\n');

    const blocks = [];
    const lines = readable.split(/\n+/).map(line => line.trim()).filter(Boolean);

    for (const line of lines) {
        const numbered = line.match(/^\d+[.)]\s+(.+)$/);
        const bullet = line.match(/^[-*•]\s+(.+)$/);

        if (numbered || bullet) {
            const content = numbered ? numbered[1] : bullet[1];
            const { main, outro } = splitListOutro(content);
            blocks.push({ type: 'bullet', text: main });
            if (outro) blocks.push({ type: 'paragraph', text: outro });
        } else {
            blocks.push({ type: 'paragraph', text: line });
        }
    }

    return blocks;
}

function appendFormattedMessageText(bubble, text, sender) {
    if (sender !== 'ai') {
        const textSpan = document.createElement('span');
        textSpan.textContent = text;
        bubble.appendChild(textSpan);
        return;
    }

    const blocks = buildAiTextBlocks(text);
    if (blocks.length === 0) return;

    const wrapper = document.createElement('div');
    wrapper.classList.add('formatted-ai-text');

    for (const block of blocks) {
        const item = document.createElement(block.type === 'bullet' ? 'div' : 'p');
        item.classList.add(block.type === 'bullet' ? 'ai-list-item' : 'ai-paragraph');

        if (block.type === 'bullet') {
            const bullet = document.createElement('span');
            bullet.classList.add('ai-list-bullet');
            bullet.textContent = '•';
            const content = document.createElement('span');
            content.textContent = block.text;
            item.appendChild(bullet);
            item.appendChild(content);
        } else {
            item.textContent = block.text;
        }

        wrapper.appendChild(item);
    }

    bubble.appendChild(wrapper);
}

function appendMessage(text, sender, base64Image = null, timestamp = null, options = {}) {
    const messageDiv = document.createElement('div');
    messageDiv.classList.add('message', `${sender}-message`);

    const bubbleWrapper = document.createElement('div');
    bubbleWrapper.classList.add('bubble-wrapper');

    const bubble = document.createElement('div');
    bubble.classList.add('message-bubble');

    if (base64Image && sender === 'user') {
        const img = document.createElement('img');
        img.src = base64Image;
        img.style.maxWidth = '100%';
        img.style.borderRadius = '4px';
        img.style.marginBottom = '8px';
        img.style.display = 'block';
        bubble.appendChild(img);
    }

    if (text) {
        appendFormattedMessageText(bubble, text, sender);
    }

    bubbleWrapper.appendChild(bubble);

    const providerLabel = sender === 'ai' ? formatProviderInfo(options.providerInfo) : '';

    // Add metadata
    if (timestamp || providerLabel) {
        const timeDiv = document.createElement('div');
        timeDiv.classList.add('message-time');
        if (providerLabel) {
            const providerButton = document.createElement('button');
            providerButton.type = 'button';
            providerButton.classList.add('provider-badge');
            providerButton.textContent = providerLabel;
            providerButton.title = 'View model details';
            providerButton.addEventListener('click', (event) => {
                event.stopPropagation();
                if (providerButton.classList.contains('is-open')) {
                    closeProviderPopover();
                    return;
                }
                showProviderPopover(providerButton, options.providerInfo);
            });
            timeDiv.appendChild(providerButton);
        }
        if (timestamp) {
            const timeSpan = document.createElement('span');
            timeSpan.textContent = formatTime(timestamp);
            timeDiv.appendChild(timeSpan);
        }
        bubbleWrapper.appendChild(timeDiv);
    }

    messageDiv.appendChild(bubbleWrapper);
    chatContainer.appendChild(messageDiv);
    scrollToBottom();

    return messageDiv; // Return it so we can append action cards if needed
}

function normalizeAiText(input) {
    if (Array.isArray(input)) {
        return input
            .map((item) => (item == null ? '' : String(item).trim()))
            .filter(Boolean)
            .join('\n\n');
    }
    if (input == null) return '';
    return String(input);
}

function splitAiMessages(text) {
    const normalized = normalizeAiText(text).trim();
    if (!normalized) return [];
    if (/(^|\s)\d+[.)]\s+/.test(normalized) || /(^|\n)\s*[-*•]\s+/.test(normalized)) {
        return [normalized];
    }
    const byBlankLine = normalized
        .split(/\n\s*\n/)
        .map((part) => part.trim())
        .filter(Boolean);
    if (byBlankLine.length > 1) return byBlankLine;
    return autoChunkAiText(normalized);
}

function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

function estimateMessageDelay(text) {
    const base = 260;
    const perChar = 12;
    const jitter = Math.floor(Math.random() * 120);
    const scaled = base + Math.min(1200, text.length * perChar) + jitter;
    return Math.min(1600, scaled);
}

async function appendAiMessages(text, options = {}) {
    const allowDelay = options.allowDelay !== false;
    const timestamp = options.timestamp || new Date().toISOString();
    const providerInfo = options.providerInfo || null;
    const parts = splitAiMessages(text);
    let lastDiv = null;

    for (let index = 0; index < parts.length; index += 1) {
        if (allowDelay && index > 0) {
            showTyping();
            await sleep(estimateMessageDelay(parts[index]));
            removeTyping();
        }
        // Only show timestamp for the last bubble in a group if multiple
        const partTimestamp = (index === parts.length - 1) ? timestamp : null;
        const partProviderInfo = (index === parts.length - 1) ? providerInfo : null;
        lastDiv = appendMessage(parts[index], 'ai', null, partTimestamp, { providerInfo: partProviderInfo });
    }

    return lastDiv;
}

function autoChunkAiText(text) {
    const trimmed = text.trim();
    if (trimmed.length <= 120) return [trimmed];

    const sentenceMatches = trimmed.match(/[^.!?…\n]+[.!?…]+|[^.!?…\n]+$/g);
    if (!sentenceMatches || sentenceMatches.length <= 1) return [trimmed];

    const bubbles = [];
    let current = '';
    for (const sentence of sentenceMatches) {
        const next = current ? `${current} ${sentence}` : sentence;
        if (next.length > 180 && current) {
            bubbles.push(current.trim());
            current = sentence;
        } else {
            current = next;
        }
    }
    if (current.trim()) bubbles.push(current.trim());

    if (bubbles.length > 3) {
        const merged = [bubbles[0], bubbles[1], bubbles.slice(2).join(' ').trim()];
        return merged.filter(Boolean);
    }

    return bubbles.length > 0 ? bubbles : [trimmed];
}

function appendActionCard(messageDiv, action) {
    if (!messageDiv || !action || action.type === 'none') return;

    const meta = getActionCardMeta(action);
    const card = document.createElement('div');
    card.classList.add('action-card');
    card.dataset.actionType = action.type || 'unknown';

    const icon = document.createElement('span');
    icon.className = 'action-card-icon';
    icon.textContent = meta.icon;

    const content = document.createElement('div');
    content.className = 'action-card-content';

    const title = document.createElement('div');
    title.className = 'action-card-title';
    title.textContent = meta.title;
    content.appendChild(title);

    if (meta.detail) {
        const detail = document.createElement('div');
        detail.className = 'action-card-detail';
        detail.textContent = meta.detail;
        content.appendChild(detail);
    }

    card.appendChild(icon);
    card.appendChild(content);
    messageDiv.querySelector('.message-bubble')?.appendChild(card);
}

function getActionCardMeta(action) {
    const target = formatActionTarget(action);
    const type = action?.type || 'unknown';
    const targetOrFallback = target || 'No target';

    const map = {
        open_url: ['🌐', 'Opened URL', target],
        search: ['🔍', 'Searched the web', target],
        open_app: ['🚀', 'Launched app', target],
        web_automation: ['🧭', 'Ran browser automation', target],
        create_folder: ['📁', 'Created folder', target],
        open_file: ['📄', 'Opened file', target],
        open_folder: ['📂', 'Opened folder', target],
        delete_file: ['🗑️', 'Deleted file', target],
        find_path: ['🔎', action.openAfter ? 'Found and opened path' : 'Found path', buildFindPathDetail(action)],
        clipboard_write: ['📋', 'Updated clipboard', target],
        learn_file: ['📚', 'Indexed file', target],
        learn_folder: ['📚', 'Indexed folder', target],
        system_info: ['💻', target ? 'Checked weather' : 'Checked system info', target],
        plugin: ['🔌', 'Ran plugin', target],
        mcp_tool: ['🧩', 'Called MCP tool', target],
        mouse_move: ['↗', 'Moved pointer', target],
        mouse_click: ['☝', 'Clicked screen', buildMouseDetail(action)],
        type_text: ['⌨', 'Typed text', target],
        key_tap: ['⌨', 'Pressed key', target],
        system_automation: ['⚙', 'Changed system setting', target]
    };

    const [icon, title, detail] = map[type] || ['⚡', `Ran action: ${type}`, targetOrFallback];
    return { icon, title, detail };
}

function buildFindPathDetail(action) {
    const target = formatActionTarget(action);
    const typeLabel = action.pathType && action.pathType !== 'any' ? ` (${action.pathType})` : '';
    return target ? `${target}${typeLabel}` : typeLabel.trim();
}

function buildMouseDetail(action) {
    const point = formatActionTarget(action);
    const button = action.button ? `button ${action.button}` : 'left button';
    return point ? `${point} · ${button}` : button;
}

function formatActionTarget(action) {
    if (!action || typeof action !== 'object') return '';
    if (action.server && action.target) return `${action.server}:${action.target}`;
    if (action.pluginName) return `${action.pluginName} ${action.target || ''}`.trim();
    if (action.target) return String(action.target);
    if (Number.isFinite(action.x) && Number.isFinite(action.y)) return `${action.x}, ${action.y}`;
    return '';
}

function getApprovalCopy(approval) {
    const action = approval?.action || {};
    const actionType = action.type || 'unknown';
    const target = formatActionTarget(action);
    const isDangerous = approval?.tier === 'dangerous';
    return {
        title: isDangerous ? 'Dangerous action requires approval' : 'Action requires approval',
        body: target ? `${actionType}: ${target}` : actionType,
        reason: approval?.reason || 'This action needs your permission before Mint can run it.',
        approveLabel: isDangerous ? 'Allow Dangerous Action' : 'Allow Action'
    };
}

function appendApprovalCard(messageDiv, approval, activity = null) {
    if (!messageDiv || !approval?.action || !window.api?.executeApprovedAction) return;

    const copy = getApprovalCopy(approval);
    const card = document.createElement('div');
    card.classList.add('action-card', 'approval-card');
    card.dataset.tier = approval.tier || 'approval';

    const content = document.createElement('div');
    content.className = 'approval-card-content';

    const title = document.createElement('div');
    title.className = 'approval-card-title';
    title.textContent = copy.title;

    const body = document.createElement('div');
    body.className = 'approval-card-body';
    body.textContent = copy.body;

    const reason = document.createElement('div');
    reason.className = 'approval-card-reason';
    reason.textContent = copy.reason;

    content.appendChild(title);
    content.appendChild(body);
    content.appendChild(reason);

    const actions = document.createElement('div');
    actions.className = 'approval-card-actions';

    const approveBtn = document.createElement('button');
    approveBtn.type = 'button';
    approveBtn.className = 'approval-btn approval-btn-approve';
    approveBtn.textContent = copy.approveLabel;

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'approval-btn approval-btn-cancel';
    cancelBtn.textContent = 'Cancel';

    const setDone = (message, state) => {
        approveBtn.disabled = true;
        cancelBtn.disabled = true;
        card.dataset.state = state;
        reason.textContent = message;
    };

    approveBtn.addEventListener('click', async () => {
        approveBtn.disabled = true;
        cancelBtn.disabled = true;
        reason.textContent = 'Running approved action...';
        const runStep = activity?.add('Running approved action', 'running', describeActionActivity(approval.action));
        setMintActivity('thinking');

        try {
            const result = await window.api.executeApprovedAction(approval.action);
            if (!result || result.success === false) {
                setDone(result?.message || 'Action failed.', 'error');
                activity?.update(runStep, 'error', 'Action failed', result?.message || '');
                activity?.finish('error', 'Failed');
                setMintActivity('error');
                return;
            }

            setDone(result.message || 'Action completed.', 'approved');
            activity?.update(runStep, 'done', 'Action completed', result.message || describeActionActivity(approval.action));
            activity?.finish('done', 'Completed');
            setMintActivity('idle');
        } catch (error) {
            console.error('[Approval] Failed to execute action:', error);
            setDone(error.message || 'Action failed.', 'error');
            activity?.update(runStep, 'error', 'Action failed', error.message || '');
            activity?.finish('error', 'Failed');
            setMintActivity('error');
        }
    });

    cancelBtn.addEventListener('click', () => {
        setDone('Cancelled by user.', 'cancelled');
        activity?.add('Approval cancelled', 'cancelled');
        activity?.finish('cancelled', 'Cancelled');
        setMintActivity('idle');
    });

    actions.appendChild(approveBtn);
    actions.appendChild(cancelBtn);
    card.appendChild(content);
    card.appendChild(actions);

    messageDiv.querySelector('.message-bubble')?.appendChild(card);
}

function showTyping() {
    const typingDiv = document.createElement('div');
    typingDiv.classList.add('message', 'ai-message', 'typing-message');
    typingDiv.id = 'typing-indicator';

    const indicator = document.createElement('div');
    indicator.classList.add('typing-indicator');
    indicator.innerHTML = '<div class="dot"></div><div class="dot"></div><div class="dot"></div>';

    typingDiv.appendChild(indicator);
    chatContainer.appendChild(typingDiv);
    scrollToBottom();
}

function removeTyping() {
    const typingDiv = document.getElementById('typing-indicator');
    if (typingDiv) {
        typingDiv.remove();
    }
}

function scrollToBottom() {
    chatContainer.scrollTop = chatContainer.scrollHeight;
}

function loadScript(src) {
    return new Promise((resolve, reject) => {
        if (document.querySelector(`script[src="${src}"]`)) {
            resolve();
            return;
        }
        const script = document.createElement('script');
        script.src = src;
        script.onload = resolve;
        script.onerror = () => reject(new Error(`Failed to load script: ${src}`));
        document.body.appendChild(script);
    });
}

function hideStartupLoading() {
    appContainer?.classList.remove('is-loading');
    if (!startupLoading) return;
    startupLoading.classList.add('is-hidden');
    setTimeout(() => startupLoading.remove(), 400);
}

async function loadLive2DWhenIdle() {
    if (!modelMount || window.Live2DManager) {
        hideStartupLoading();
        return;
    }
    try {
        await loadScript('../../node_modules/@hazart-pkg/live2d-core/live2dcubismcore.min.js');
        await loadScript('../../node_modules/pixi.js/dist/browser/pixi.min.js');
        await loadScript('../../node_modules/pixi-live2d-display/dist/cubism4.min.js');
        await loadScript('live2d_manager.js');
        if (window.Live2DManager) {
            await Live2DManager.loadModel(modelMount, modelStatus, modelShell);
            applyModelPanelControlState();
        }
    } catch (err) {
        console.error('[Live2D] Deferred load failed:', err);
        if (modelStatus) {
            modelStatus.classList.add('is-error');
            modelStatus.textContent = 'Live2D model unavailable.';
        }
    } finally {
        hideStartupLoading();
    }
}

async function loadChatHistory() {
    try {
        const history = await window.api.getChatHistory();
        const initial = chatContainer.querySelector('.message.initial');
        
        if (!Array.isArray(history) || history.length === 0) {
            if (initial) {
                initial.style.display = 'flex';
                initial.style.opacity = '1';
            }
            return;
        }

        if (initial) {
            initial.remove();
        }

        for (const item of history) {
            if (!item || typeof item.text !== 'string' || !item.text.trim()) continue;
            const sender = item.sender === 'user' ? 'user' : 'ai';
            if (sender === 'user' && !String(item.text).startsWith('Model interaction:')) {
                rememberConversationLanguage(item.text);
            }
            appendMessage(item.text, sender, null, item.timestamp, {
                providerInfo: sender === 'ai' ? item.providerInfo : null
            });
        }
    } catch (error) {
        console.error('Failed to load chat history:', error);
    }
}

async function sendTextMessage(text, options = {}) {
    const cleanText = (text || '').trim();
    const allowSmartContext = options.allowSmartContext !== false;
    const includePendingImage = options.includePendingImage !== false;
    const displayText = options.displayText !== undefined ? options.displayText : cleanText;
    const trackLanguage = options.trackLanguage !== false;

    // We can send either a text message, an image, or both.
    if (!cleanText && (!includePendingImage || !currentBase64Image)) return;

    // Cache the image for sending and UI, then clear
    let imageToSend = includePendingImage ? currentBase64Image : null;

    // Clear input & UI for explicit images
    chatInput.value = '';
    if (includePendingImage) {
        currentBase64Image = null;
        imagePreviewContainer.style.display = 'none';
        imagePreview.src = '';
    }

    const now = new Date().toISOString();

    // Show user message (with explicit image if available)
    appendMessage(displayText, 'user', imageToSend, now);
    if (trackLanguage) {
        rememberConversationLanguage(displayText || cleanText);
    }

    const activity = shouldShowAgentActivity(options) ? createAgentActivityCard() : null;
    const contextStep = activity?.add('Preparing desktop context', 'running');

    // Show typing early so user knows we are processing
    showTyping();
    setMintActivity('thinking');

    let messageToSend = cleanText;

    // Check Smart Context Toggle
    const smartToggle = document.getElementById('smart-context-toggle');
    if (allowSmartContext && smartToggle && smartToggle.checked && !imageToSend) {
        try {
            const [silentCapture, smartContext] = await Promise.all([
                window.api.captureSilentScreen(),
                window.api.getSmartContext ? window.api.getSmartContext() : Promise.resolve(null)
            ]);
            if (silentCapture) {
                // Set imageToSend so it gets sent to the API, but we already appended the chat bubble
                imageToSend = silentCapture;
            }
            if (smartContext) {
                messageToSend = appendSmartContextToMessage(cleanText, smartContext);
            }
            if (activity && contextStep) {
                activity.update(
                    contextStep,
                    'done',
                    'Read Smart Context',
                    describeSmartContextActivity(smartContext, Boolean(silentCapture))
                );
            }
        } catch (err) {
            console.error("Smart Context capture failed:", err);
            activity?.update(contextStep, 'error', 'Smart Context unavailable', err.message || '');
        }
    } else if (activity && contextStep) {
        activity.update(contextStep, 'skipped', 'Smart Context skipped', imageToSend ? 'image already attached' : 'toggle is off');
    }

    // Hide proactive bar if user is actively typing a message
    hideProactiveBar();
    const modelStep = activity?.add('Waiting for model response', 'running');

    try {
        // Send to main process (text, image, audio=null)
        const response = await window.api.sendMessage(messageToSend, imageToSend, null);
        removeTyping();
        activity?.update(modelStep, 'done', 'Model response received');

        if (typeof response.response !== 'string') {
            response.response = normalizeAiText(response.response);
        }

        // Handle system_info action: fetch data and append to AI message
        if (response.action && response.action.type === 'system_info') {
            const infoStep = activity?.add('Running local info action', 'running', describeActionActivity(response.action));
            const city = (response.action.target || '').trim();
            // Only treat as weather if city looks like a real location name (not blank, not 'date', not 'time')
            const weatherKeywords = ['date', 'time', 'วัน', 'เวลา', 'today', 'now'];
            const isWeather = city && !weatherKeywords.some(k => city.toLowerCase().includes(k));
            
            if (isWeather) {
                // Weather query
                const weather = await window.api.getWeather(city);
                response.response += `\n\n🌡️ ${weather.data}`;
                activity?.update(infoStep, 'done', 'Weather info added', city);
            } else {
                // General system info (date, time, RAM, CPU)
                const info = await window.api.getSystemInfo();
                const machine = info.machine && info.machine.display ? `\n🖥️ รุ่นเครื่อง: ${info.machine.display}` : '';
                const distro = info.distro ? `\nระบบ: ${info.distro}` : '';
                response.response += `\n\n📅 วันนี้: ${info.date}\n⏰ เวลา: ${info.time}${machine}${distro}\n💻 CPU: ${info.cpu.model} (${info.cpu.cores} คอร์)\n💻 RAM: ${info.ram.used} / ${info.ram.total} (${info.ram.percent})`;
                activity?.update(infoStep, 'done', 'System info added');
            }
        }

        // Show AI response
        const msgDiv = await appendAiMessages(response.response, {
            allowDelay: true,
            timestamp: response.timestamp,
            providerInfo: response.providerInfo
        });

        // Speak AI response
        await speakText(normalizeAiText(response.response), { onEnd: resumeSpeechIfNeeded });
        notifyAiIfNeeded();

        // Append action card if applicable
        if (response.approval?.required) {
            activity?.add('Selected action', 'approval', describeActionActivity(response.approval.action));
            activity?.add('Waiting for approval', 'running', response.approval.reason || '');
            activity?.finish('waiting', 'Waiting');
            appendApprovalCard(msgDiv, response.approval, activity);
        } else if (response.action && response.action.type !== 'none' && response.action.type !== 'system_info') {
            activity?.add('Selected action', 'done', describeActionActivity(response.action));
            appendActionCard(msgDiv, response.action);
            activity?.finish('done', 'Completed');
        } else if (response.action && response.action.type === 'system_info') {
            activity?.add('Selected action', 'done', describeActionActivity(response.action));
            activity?.finish('done', 'Completed');
        } else {
            activity?.add('No desktop action selected', 'done');
            activity?.finish('done', 'Completed');
        }
    } catch (error) {
        removeTyping();
        setMintActivity('error');
        activity?.update(modelStep, 'error', 'Model request failed', error.message || '');
        activity?.finish('error', 'Failed');
        appendMessage("Sorry, I encountered an error communicating with the main process.", 'ai');
        console.error(error);
        resumeSpeechIfNeeded();
    }
}

chatForm.addEventListener('submit', throttle(async (e) => {
    e.preventDefault();
    const text = chatInput.value.trim();
    await sendTextMessage(text);
}, 500));

window.addEventListener('live2d-model-interaction', async (event) => {
    const prompt = event?.detail?.prompt;
    if (!prompt) return;
    setMintActivity('thinking');
    const interactionPrompt = `${prompt}\n\n${buildInteractionLanguageInstruction()}`;
    const displayPrefix = lastConversationLanguage === 'thai' ? 'แตะโมเดล' : 'Model interaction';
    await sendTextMessage(interactionPrompt, {
        allowSmartContext: false,
        includePendingImage: false,
        trackLanguage: false,
        displayText: `${displayPrefix}: ${event.detail.label || event.detail.region || 'Interaction'}`
    });
});

// --- Image Paste and Drag-n-Drop Support ---
function handleImageFile(file) {
    if (!file || !file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = (e) => {
        currentBase64Image = e.target.result;
        imagePreview.src = currentBase64Image;
        imagePreviewContainer.style.display = 'block';
        chatInput.focus();
    };
    reader.readAsDataURL(file);
}

// Paste Event
chatInput.addEventListener('paste', (e) => {
    const items = (e.clipboardData || e.originalEvent.clipboardData).items;
    for (let index in items) {
        const item = items[index];
        if (item.kind === 'file' && item.type.startsWith('image/')) {
            const blob = item.getAsFile();
            handleImageFile(blob);
            break; // Handle only the first image
        }
    }
});

// Drag and Drop Events (on the whole chat form/input area)
const inputArea = document.querySelector('.input-area');

inputArea.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    inputArea.style.opacity = '0.7'; // Visual feedback
});

inputArea.addEventListener('dragleave', (e) => {
    e.preventDefault();
    e.stopPropagation();
    inputArea.style.opacity = '1';
});

inputArea.addEventListener('drop', (e) => {
    e.preventDefault();
    e.stopPropagation();
    inputArea.style.opacity = '1';

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
        handleImageFile(e.dataTransfer.files[0]);
    }
});

// Focus input on load + init theme
window.addEventListener('DOMContentLoaded', async () => {
    chatInput.focus();
    await loadTheme();
    setMintActivity('idle');
    await loadChatHistory();
    loadLive2DWhenIdle();
});

// Proactive OS Notifications (Battery, Network, etc.)
window.api.onProactiveNotification((data) => {
    if (!data || !data.message) return;
    appendMessage(data.message, 'ai');
    // Also speak the notification automatically
    speakText(data.message);
});

window.addEventListener('focus', () => {
    if (window.api.clearAiNotifications) window.api.clearAiNotifications();
});

document.addEventListener('click', closeProviderPopover);
document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closeProviderPopover();
});

// =====================
// Proactive Smart Suggestion Engine
// =====================

function showProactiveBar(data) {
    // Clear old chips
    proactiveChips.innerHTML = '';

    // Set message
    proactiveMessage.textContent = data.message || '';

    // Render each suggestion as a chip
    data.suggestions.forEach((item, index) => {
        const chip = document.createElement('button');
        chip.className = 'suggestion-chip';
        chip.textContent = item.label;
        chip.style.animationDelay = `${index * 60}ms`;

        chip.addEventListener('click', async () => {
            hideProactiveBar();

            if (window.api.recordBehavior) {
                window.api.recordBehavior(`User picked: ${item.label}`);
            }

            showTyping();
            try {
                const result = await window.api.executeProactiveAction(item.action);
                removeTyping();
                const confirmText = result?.message || `เปิด ${item.label} แล้วค่ะ ✅`;
                const msgDiv = appendMessage(confirmText, 'ai');
                speakText(confirmText);
                if (item.action && item.action.type !== 'none') {
                    appendActionCard(msgDiv, item.action);
                }
            } catch (err) {
                removeTyping();
                appendMessage('ขออภัยค่ะ เกิดข้อผิดพลาด', 'ai');
                console.error('[Chip] Error:', err);
            }
        });

        proactiveChips.appendChild(chip);
    });

    // Show bar with animation reset
    proactiveBar.style.display = 'none';
    requestAnimationFrame(() => {
        proactiveBar.style.display = 'block';
    });
}

function hideProactiveBar() {
    proactiveBar.style.display = 'none';
    proactiveChips.innerHTML = '';
}

// Receive multi-suggestion data from main process
window.api.onProactiveSuggestion((data) => {
    if (data && data.message && Array.isArray(data.suggestions) && data.suggestions.length > 0) {
        showProactiveBar(data);
        notifyAiIfNeeded();
    }
});

// Dismiss button
proactiveDismissBtn.addEventListener('click', () => {
    hideProactiveBar();
});

// Sync Smart Context toggle → start/stop proactive loop
const smartContextToggle = document.getElementById('smart-context-toggle');
if (smartContextToggle) {
    smartContextToggle.addEventListener('change', () => {
        window.api.toggleProactive(smartContextToggle.checked);
    });
}

// Toggle Live2D Model visibility
const toggleModelBtn = document.getElementById('toggle-model-btn');
const assistantWorkspace = document.querySelector('.assistant-workspace');
const modelLockBtn = document.getElementById('model-lock-btn');
const modelScaleSlider = document.getElementById('model-scale-slider');
const modelScaleValue = document.getElementById('model-scale-value');
const modelScaleResetBtn = document.getElementById('model-scale-reset-btn');
const modelBgBtn = document.getElementById('model-bg-btn');
const layoutPresetBtns = document.querySelectorAll('.layout-preset-btn');

const modelBgStorageKey = 'mint-model-background';
const modelScaleStorageKey = 'mint-model-scale';
const modelPositionLockStorageKey = 'mint-model-position-locked';
const workspaceLayoutStorageKey = 'mint-workspace-layout';
const modelBgClasses = ['model-bg-default', 'model-bg-clear', 'model-bg-grid', 'model-bg-stage'];
const modelBgLabels = ['Default background', 'Clear background', 'Grid background', 'Stage background'];
const workspaceLayoutClasses = ['layout-chat'];
const workspaceLayoutPresets = ['companion', 'chat'];

function setModelHidden(isHidden) {
    if (!assistantWorkspace || !toggleModelBtn) return;
    assistantWorkspace.classList.toggle('model-hidden', Boolean(isHidden));
    toggleModelBtn.classList.toggle('active', Boolean(isHidden));
    toggleModelBtn.setAttribute('aria-pressed', String(Boolean(isHidden)));
    localStorage.setItem('mint-model-hidden', String(Boolean(isHidden)));

    if (!isHidden && window.Live2DManager && Live2DManager.model) {
        setTimeout(() => {
            window.dispatchEvent(new Event('resize'));
            if (typeof Live2DManager.fitModelToMount === 'function') {
                Live2DManager.fitModelToMount();
            }
        }, 450);
    }
}

function setModelPositionLocked(isLocked) {
    const locked = Boolean(isLocked);
    localStorage.setItem(modelPositionLockStorageKey, String(locked));
    modelLockBtn?.classList.toggle('is-active', locked);
    modelLockBtn?.setAttribute('aria-pressed', String(locked));
    modelLockBtn?.setAttribute('title', locked ? 'Unlock model position' : 'Lock model position');
    if (window.Live2DManager) {
        Live2DManager.setPointerTrackingEnabled(!locked);
    }
}

function setModelBackground(index) {
    if (!modelShell) return;
    const normalized = ((Number(index) || 0) + modelBgClasses.length) % modelBgClasses.length;
    modelBgClasses.forEach(className => modelShell.classList.remove(className));
    if (normalized > 0) {
        modelShell.classList.add(modelBgClasses[normalized]);
    }
    localStorage.setItem(modelBgStorageKey, String(normalized));
    modelBgBtn?.setAttribute('title', modelBgLabels[normalized]);
}

function setModelScale(value) {
    const next = Math.max(78, Math.min(128, Number(value) || 100));
    localStorage.setItem(modelScaleStorageKey, String(next));
    if (modelScaleSlider) modelScaleSlider.value = String(next);
    if (modelScaleValue) modelScaleValue.textContent = `${(next / 100).toFixed(2)}x`;
    if (window.Live2DManager) {
        Live2DManager.setZoomMultiplier(next / 100);
    }
}

function applyModelPanelControlState() {
    setModelPositionLocked(localStorage.getItem(modelPositionLockStorageKey) === 'true');
    setModelBackground(Number(localStorage.getItem(modelBgStorageKey) || 0));
    setModelScale(Number(localStorage.getItem(modelScaleStorageKey) || 100));
    setWorkspaceLayout(localStorage.getItem(workspaceLayoutStorageKey) || 'companion');
}

function setWorkspaceLayout(layout) {
    if (!assistantWorkspace) return;
    const normalized = workspaceLayoutPresets.includes(layout) ? layout : 'companion';
    workspaceLayoutClasses.forEach(className => assistantWorkspace.classList.remove(className));
    if (normalized !== 'companion') {
        assistantWorkspace.classList.add(`layout-${normalized}`);
    }
    localStorage.setItem(workspaceLayoutStorageKey, normalized);
    layoutPresetBtns.forEach((button) => {
        const isActive = button.dataset.layoutPreset === normalized;
        button.classList.toggle('is-active', isActive);
        button.setAttribute('aria-pressed', String(isActive));
    });
}

if (toggleModelBtn && assistantWorkspace) {
    toggleModelBtn.addEventListener('click', () => {
        setModelHidden(!assistantWorkspace.classList.contains('model-hidden'));
    });

    // Restore preference on load
    const savedModelHidden = localStorage.getItem('mint-model-hidden');
    const savedHidden = savedModelHidden === null || savedModelHidden === 'true';
    if (savedHidden) {
        setModelHidden(true);
    }
}

modelLockBtn?.addEventListener('click', () => {
    setModelPositionLocked(localStorage.getItem(modelPositionLockStorageKey) !== 'true');
});
modelScaleSlider?.addEventListener('input', (event) => setModelScale(event.target.value));
modelScaleResetBtn?.addEventListener('click', () => setModelScale(100));
modelBgBtn?.addEventListener('click', () => {
    const current = Number(localStorage.getItem(modelBgStorageKey) || 0);
    setModelBackground(current + 1);
});
layoutPresetBtns.forEach((button) => {
    button.addEventListener('click', () => setWorkspaceLayout(button.dataset.layoutPreset));
});

applyModelPanelControlState();

// Cycle Shiroko's Expression
const changeExpressionBtn = document.getElementById('change-expression-btn');
if (changeExpressionBtn) {
    changeExpressionBtn.addEventListener('click', () => {
        if (window.Live2DManager) {
            Live2DManager.cycleExpression();
        }
    });
}

// Cycle Live2D accessories
const accessoryStorageKey = 'mint-live2d-accessories';
const accessoryCycleBtn = document.getElementById('accessory-cycle-btn');
const accessoryCycleLabel = document.getElementById('accessory-cycle-label');
const accessoryCycleOrder = [null, 'glasses', 'pen', 'cat'];
const accessoryLabels = {
    glasses: 'Glasses',
    pen: 'Pen',
    cat: 'Cat'
};
let savedAccessories = {};
try {
    savedAccessories = JSON.parse(localStorage.getItem(accessoryStorageKey) || '{}') || {};
} catch (_) {
    savedAccessories = {};
}

const getSavedAccessoryId = () => accessoryCycleOrder.find(id => id && savedAccessories[id] === true) || null;

function updateAccessoryCycleButton(accessoryId) {
    if (!accessoryCycleBtn) return;
    const isActive = Boolean(accessoryId);
    const label = accessoryId ? accessoryLabels[accessoryId] : 'Accessory';
    accessoryCycleBtn.classList.toggle('active', isActive);
    accessoryCycleBtn.setAttribute('aria-pressed', String(isActive));
    accessoryCycleBtn.title = `Accessory: ${label}`;
    if (accessoryCycleLabel) accessoryCycleLabel.textContent = label;
}

let currentAccessoryId = getSavedAccessoryId();
updateAccessoryCycleButton(currentAccessoryId);

if (accessoryCycleBtn) {
    accessoryCycleBtn.addEventListener('click', () => {
        const currentIndex = accessoryCycleOrder.indexOf(currentAccessoryId);
        currentAccessoryId = accessoryCycleOrder[(currentIndex + 1) % accessoryCycleOrder.length];
        updateAccessoryCycleButton(currentAccessoryId);

        if (window.Live2DManager) {
            Live2DManager.setExclusiveAccessory(currentAccessoryId, true);
        } else {
            savedAccessories = {};
            if (currentAccessoryId) savedAccessories[currentAccessoryId] = true;
            localStorage.setItem(accessoryStorageKey, JSON.stringify(savedAccessories));
        }
    });
}

// Toggle Live2D model interaction
const toggleInteractionBtn = document.getElementById('toggle-interaction-btn');
if (toggleInteractionBtn) {
    const savedInteractionEnabled = localStorage.getItem('mint-model-interaction-enabled') !== 'false';
    toggleInteractionBtn.classList.toggle('active', savedInteractionEnabled);
    toggleInteractionBtn.setAttribute('aria-pressed', String(savedInteractionEnabled));
    if (window.Live2DManager) {
        Live2DManager.setInteractionEnabled(savedInteractionEnabled);
    }

    toggleInteractionBtn.addEventListener('click', () => {
        const isEnabled = !toggleInteractionBtn.classList.contains('active');
        toggleInteractionBtn.classList.toggle('active', isEnabled);
        toggleInteractionBtn.setAttribute('aria-pressed', String(isEnabled));
        if (window.Live2DManager) {
            Live2DManager.setInteractionEnabled(isEnabled, true);
        } else {
            localStorage.setItem('mint-model-interaction-enabled', String(isEnabled));
        }
    });
}

// Toggle Live2D interaction area guide
const interactionGuideBtn = document.getElementById('interaction-guide-btn');
if (interactionGuideBtn && modelShell) {
    const savedGuideVisible = localStorage.getItem('mint-interaction-guide-visible') === 'true';
    modelShell.classList.toggle('show-interaction-guide', savedGuideVisible);
    interactionGuideBtn.classList.toggle('active', savedGuideVisible);

    interactionGuideBtn.addEventListener('click', () => {
        const isVisible = modelShell.classList.toggle('show-interaction-guide');
        interactionGuideBtn.classList.toggle('active', isVisible);
        localStorage.setItem('mint-interaction-guide-visible', String(isVisible));
    });
}

// Spotlight integration
window.api.onSpotlightToChat((query) => {
    chatInput.value = query;
    chatForm.dispatchEvent(new Event('submit'));
});
