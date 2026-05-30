/// <reference path="env.d.ts" />

// ─── DOM Element References ───────────────────────────────────────────────────

const chatContainer = document.getElementById('chat-container') as HTMLDivElement;
const chatForm = document.getElementById('chat-form') as HTMLFormElement;
const chatInput = document.getElementById('chat-input') as HTMLTextAreaElement;
const closeBtn = document.getElementById('close-btn') as HTMLButtonElement;
const maximizeBtn = document.getElementById('maximize-btn') as HTMLButtonElement;
const minimizeBtn = document.getElementById('minimize-btn') as HTMLButtonElement;
const clearBtn = document.getElementById('clear-btn') as HTMLButtonElement;
const settingsBtn = document.getElementById('settings-btn') as HTMLButtonElement;
const sidebarNewChatBtn = document.getElementById('sidebar-new-chat') as HTMLButtonElement | null;
const sidebarSettingsBtn = document.getElementById('sidebar-settings') as HTMLButtonElement | null;
const sidebarToggleBtn = document.getElementById('sidebar-toggle') as HTMLButtonElement | null;
const appBody = document.querySelector('.app-body') as HTMLElement | null;
const sidebarChatBtn = document.getElementById('sidebar-chat-btn') as HTMLButtonElement | null;
const sidebarPicturesBtn = document.getElementById('sidebar-pictures-btn') as HTMLButtonElement | null;
const picturesLibrary = document.getElementById('pictures-library') as HTMLElement | null;
const picturesGrid = document.getElementById('pictures-grid') as HTMLElement | null;
const picturesEmpty = document.getElementById('pictures-empty') as HTMLElement | null;
const picturesCloseBtn = document.getElementById('pictures-close-btn') as HTMLButtonElement | null;
const micBtn = document.getElementById('mic-btn') as HTMLButtonElement;
const visionBtn = document.getElementById('vision-btn') as HTMLButtonElement;
const chatProviderSelect = document.getElementById('chat-provider-select') as HTMLSelectElement | null;
const imagePreviewContainer = document.getElementById('image-preview-container') as HTMLElement;
const imagePreview = document.getElementById('image-preview') as HTMLImageElement;
const removeImageBtn = document.getElementById('remove-image-btn') as HTMLButtonElement;
const agentModeToggle = document.getElementById('agent-mode-toggle') as HTMLInputElement | null;
const modelMount = document.getElementById('model-mount') as HTMLElement | null;
const modelShell = document.getElementById('model-shell') as HTMLElement | null;
const modelStatus = document.getElementById('model-status') as HTMLElement | null;
const mintStatus = document.getElementById('mint-status') as HTMLElement | null;
const mintStatusLabel = document.getElementById('mint-status-label') as HTMLElement | null;
const modelActivityBadge = document.getElementById('model-activity-badge') as HTMLElement | null;
const startupLoading = document.getElementById('startup-loading') as HTMLElement | null;
const appContainer = document.querySelector('.app-container') as HTMLElement | null;
const proactiveBar = document.getElementById('proactive-bar') as HTMLElement;
const proactiveMessage = document.getElementById('proactive-message') as HTMLElement;
const proactiveChips = document.getElementById('proactive-chips') as HTMLElement;
const proactiveDismissBtn = document.getElementById('proactive-dismiss-btn') as HTMLButtonElement;

if (startupLoading) {
    startupLoading.style.background = 'var(--bg-gradient)';
    startupLoading.style.color = 'var(--text-muted)';
}

// ─── State Variables ───────────────────────────────────────────────────────────

let currentBase64Image: string | null = null;
let enableVoiceReply = true;
let ttsProvider = 'google';
let ttsVolume = 1.0;
let ttsSpeed = 1.0;
let ttsPitch = 1.0;
let lastConversationLanguage: 'thai' | 'english' | 'auto' = 'auto';
let mintActivityResetTimer: ReturnType<typeof setTimeout> | null = null;
let currentSettings: Record<string, any> = {};

// ─── Types ─────────────────────────────────────────────────────────────────────

type MintActivityState = 'idle' | 'listening' | 'thinking' | 'speaking' | 'error';

interface ActionCardMeta {
    icon: string;
    title: string;
    detail: string;
}

interface AgentActivity {
    element: HTMLElement;
    list: HTMLElement;
    status: HTMLElement;
    add: (label: string, state?: string, detail?: string) => HTMLElement;
    update: (item: HTMLElement | null, state: string, label?: string, detail?: string) => void;
    finish: (state?: string, label?: string) => void;
}

interface ProactiveChip {
    label: string;
    action: any;
}

interface ProactiveSuggestionData {
    message: string;
    suggestions: ProactiveChip[];
}

// ─── Provider Picker ──────────────────────────────────────────────────────────

const PROVIDER_PICKER_OPTIONS: [string, string][] = [
    ['gemini', 'Gemini'],
    ['anthropic', 'Claude'],
    ['openai', 'OpenAI'],
    ['ollama', 'Ollama'],
    ['huggingface', 'Hugging Face'],
    ['local_openai', 'Local'],
];

function buildProviderPicker(settings: Record<string, any> = currentSettings): void {
    if (!chatProviderSelect) return;
    chatProviderSelect.textContent = '';
    PROVIDER_PICKER_OPTIONS.forEach(([value, label]) => {
        const option = document.createElement('option');
        option.value = value;
        option.textContent = label;
        chatProviderSelect!.appendChild(option);
    });
    chatProviderSelect.value = settings.aiProvider || 'gemini';
}

function syncAgentModeToggle(settings: Record<string, any> = currentSettings): void {
    if (!agentModeToggle) return;
    agentModeToggle.checked = settings.assistantMode === 'agent';
    agentModeToggle.closest('.smart-context-control')?.classList.toggle('is-active', agentModeToggle.checked);
}

async function changeChatProvider(provider: string): Promise<void> {
    if (!PROVIDER_PICKER_OPTIONS.some(([value]) => value === provider)) return;
    const nextSettings = { ...currentSettings, aiProvider: provider };
    if (chatProviderSelect) chatProviderSelect.disabled = true;
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
        if (chatProviderSelect) chatProviderSelect.disabled = false;
    }
}

// ─── Activity State ───────────────────────────────────────────────────────────

const MINT_ACTIVITY_STATES: Record<MintActivityState, { label: string; title: string }> = {
    idle:      { label: 'Idle',      title: 'Mint is idle' },
    listening: { label: 'Listening', title: 'Mint is listening' },
    thinking:  { label: 'Thinking',  title: 'Mint is thinking' },
    speaking:  { label: 'Speaking',  title: 'Mint is speaking' },
    error:     { label: 'Error',     title: 'Mint needs attention' },
};

function setMintActivity(state: MintActivityState, options: { resetAfter?: number } = {}): void {
    const normalizedState: MintActivityState = MINT_ACTIVITY_STATES[state] ? state : 'idle';
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

    if (window.api?.setAiState) {
        window.api.setAiState(normalizedState);
    }

    if (normalizedState === 'error' || options.resetAfter) {
        mintActivityResetTimer = setTimeout(() => {
            setMintActivity('idle');
        }, options.resetAfter || 3500);
    }
}

// ─── Language Detection ───────────────────────────────────────────────────────

function detectConversationLanguage(text: string): 'thai' | 'english' | 'auto' {
    const value = String(text || '');
    if (/[\u0E00-\u0E7F]/.test(value)) return 'thai';
    if (/[A-Za-z]/.test(value)) return 'english';
    return 'auto';
}

function rememberConversationLanguage(text: string): void {
    const detected = detectConversationLanguage(text);
    if (detected !== 'auto') {
        lastConversationLanguage = detected;
    }
}

function buildInteractionLanguageInstruction(): string {
    if (lastConversationLanguage === 'thai') {
        return 'Current conversation language: Thai. Reply in Thai. Do not reply in English just because this interaction instruction is written in English.';
    }
    if (lastConversationLanguage === 'english') {
        return 'Current conversation language: English. Reply in English. Do not switch to Thai.';
    }
    return 'Infer the reply language from the recent conversation before this interaction instruction, not from the language of this instruction.';
}

// ─── Theme ────────────────────────────────────────────────────────────────────

function hexToRgb(hex: string): { r: number; g: number; b: number } {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result
        ? { r: parseInt(result[1], 16), g: parseInt(result[2], 16), b: parseInt(result[3], 16) }
        : { r: 15, g: 23, b: 42 };
}

function lightenColor(hex: string, amount: number): string {
    const clean = hex.replace('#', '');
    if (clean.length !== 6) return hex;
    const num = parseInt(clean, 16);
    const r = Math.min(255, (num >> 16) + amount);
    const g = Math.min(255, ((num >> 8) & 0x00ff) + amount);
    const b = Math.min(255, (num & 0x0000ff) + amount);
    return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

function applyTheme(
    theme: string,
    accentColor: string,
    systemTextColor: string,
    config: Record<string, any> = {}
): void {
    document.documentElement.setAttribute('data-theme', theme || 'dark');
    const accent = accentColor || '#8f6cf5';
    const defaultTextColor = theme === 'light' ? '#0f172a' : '#e8e8ea';
    const textColor =
        !systemTextColor || (theme === 'light' && systemTextColor === '#f8fafc')
            ? defaultTextColor
            : systemTextColor;

    document.documentElement.style.setProperty('--accent', accent);
    document.documentElement.style.setProperty('--accent-hover', lightenColor(accent, 20));
    document.documentElement.style.setProperty('--text-main', textColor);
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
            '--bg-color', '--bg-gradient', '--panel-bg', '--panel-raised',
            '--panel-soft', '--chrome-bg', '--surface-bg', '--surface-strong', '--input-bg',
        ].forEach((name) => document.documentElement.style.removeProperty(name));
    }
}

async function loadTheme(): Promise<void> {
    try {
        const config = await window.api.getSettings();
        currentSettings = config || {};
        applyTheme(config.theme, config.accentColor, config.systemTextColor, config);
        enableVoiceReply = config.enableVoiceReply !== false;
        ttsProvider = config.ttsProvider || 'google';
        ttsVolume  = config.ttsVolume  !== undefined ? config.ttsVolume  : 1.0;
        ttsSpeed   = config.ttsSpeed   !== undefined ? config.ttsSpeed   : 1.0;
        ttsPitch   = config.ttsPitch   !== undefined ? config.ttsPitch   : 1.0;
        buildProviderPicker(currentSettings);
        syncAgentModeToggle(currentSettings);
    } catch {
        applyTheme('dark', '#8b5cf6', '#f8fafc');
        buildProviderPicker(currentSettings);
        syncAgentModeToggle(currentSettings);
    }
}

window.api.onSettingsChanged((config: any) => {
    currentSettings = config || currentSettings;
    applyTheme(config.theme, config.accentColor, config.systemTextColor, config);
    enableVoiceReply = config.enableVoiceReply !== false;
    ttsProvider = config.ttsProvider || 'google';
    ttsVolume  = config.ttsVolume  !== undefined ? config.ttsVolume  : 1.0;
    ttsSpeed   = config.ttsSpeed   !== undefined ? config.ttsSpeed   : 1.0;
    ttsPitch   = config.ttsPitch   !== undefined ? config.ttsPitch   : 1.0;
    buildProviderPicker(currentSettings);
    syncAgentModeToggle(currentSettings);
});

chatProviderSelect?.addEventListener('change', (event: Event) => {
    changeChatProvider((event.target as HTMLSelectElement).value);
});

agentModeToggle?.addEventListener('change', async () => {
    const nextSettings = { ...currentSettings, assistantMode: agentModeToggle!.checked ? 'agent' : 'chat' };
    agentModeToggle!.disabled = true;
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
        agentModeToggle!.disabled = false;
    }
});

// ─── Voice Input ──────────────────────────────────────────────────────────────

let mediaRecorder: MediaRecorder | null = null;
let audioChunks: Blob[] = [];
let speechRecognition: SpeechRecognition | null = null;
let isSpeechStreaming = false;
let speechInterim = '';
let speechHadResult = false;
let speechFallbackTimer: ReturnType<typeof setTimeout> | null = null;
let voiceMode: 'speech' | 'recorder' | null = null;
let voiceSendQueue: Promise<void> = Promise.resolve();
let speechPausedForReply = false;
let resumeSpeechAfterResponse = false;
const DEFAULT_PLACEHOLDER = 'Type or speak a command...';
const SpeechRecognitionCtor = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;

function notifyAiIfNeeded(): void {
    if (!window.api.notifyAiResponse) return;
    if (!document.hasFocus() || document.hidden) {
        window.api.notifyAiResponse();
    } else if (window.api.clearAiNotifications) {
        window.api.clearAiNotifications();
    }
}

function queueVoiceTextSend(text: string): void {
    const clean = (text || '').trim();
    if (!clean) return;
    voiceSendQueue = voiceSendQueue.then(() => sendTextMessage(clean, { allowSmartContext: false }));
}

function pauseSpeechForReply(): void {
    if (!speechRecognition || !isSpeechStreaming) return;
    resumeSpeechAfterResponse = true;
    speechPausedForReply = true;
    try { speechRecognition.stop(); } catch (_) {}
}

function resumeSpeechIfNeeded(): void {
    if (!speechRecognition || !isSpeechStreaming) {
        resumeSpeechAfterResponse = false;
        speechPausedForReply = false;
        return;
    }
    if (!resumeSpeechAfterResponse) return;
    resumeSpeechAfterResponse = false;
    speechPausedForReply = false;
    try { speechRecognition.start(); } catch (e) { console.error('Speech recognition resume error:', e); }
}

function setupSpeechRecognition(): void {
    if (!SpeechRecognitionCtor) return;
    speechRecognition = new SpeechRecognitionCtor() as SpeechRecognition;
    speechRecognition.lang = 'th-TH';
    speechRecognition.interimResults = true;
    speechRecognition.continuous = false;

    speechRecognition.onstart = () => {
        micBtn.classList.add('listening');
        chatInput.placeholder = 'Listening... (Click to stop)';
        setMintActivity('listening');
        speechHadResult = false;
        if (speechFallbackTimer) clearTimeout(speechFallbackTimer);
        speechFallbackTimer = setTimeout(() => {
            if (isSpeechStreaming && !speechHadResult) fallbackToMediaRecorder();
        }, 1500);
    };

    speechRecognition.onresult = (event: SpeechRecognitionEvent) => {
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
            speechInterim = '';
            chatInput.value = '';
            pauseSpeechForReply();
            queueVoiceTextSend(finalTranscript.trim());
        } else {
            speechInterim = interimTranscript;
            chatInput.value = speechInterim.trimStart();
        }
    };

    speechRecognition.onerror = () => {
        setMintActivity('error');
        fallbackToMediaRecorder();
        isSpeechStreaming = false;
        resetMicUI();
    };

    speechRecognition.onend = () => {
        if (speechFallbackTimer) { clearTimeout(speechFallbackTimer); speechFallbackTimer = null; }
        if (speechPausedForReply) return;
        if (isSpeechStreaming && !speechHadResult) { fallbackToMediaRecorder(); return; }
        if (isSpeechStreaming) {
            try { speechRecognition!.start(); } catch (e) {
                console.error('Speech recognition restart error:', e);
                isSpeechStreaming = false;
                resetMicUI();
            }
        } else {
            resetMicUI();
        }
    };
}

async function setupMediaRecorder(): Promise<void> {
    try {
        const stream = await navigator.mediaDevices.getUserMedia({
            audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        });
        const mimeType = MediaRecorder.isTypeSupported('audio/webm') ? 'audio/webm' : 'audio/mp4';
        mediaRecorder = new MediaRecorder(stream, { mimeType });

        mediaRecorder.ondataavailable = (event: BlobEvent) => {
            if (event.data.size > 0) audioChunks.push(event.data);
        };

        mediaRecorder.onstop = async () => {
            if (audioChunks.length === 0) { resetMicUI(); return; }
            const audioBlob = new Blob(audioChunks, { type: mimeType });
            audioChunks = [];
            const reader = new FileReader();
            reader.readAsDataURL(audioBlob);
            reader.onloadend = async () => {
                await sendVoiceMessage(reader.result as string);
            };
        };

        mediaRecorder.onstart = () => {
            micBtn.classList.add('listening');
            chatInput.placeholder = 'Listening... (Click to stop)';
            setMintActivity('listening');
        };
    } catch (err) {
        console.error('Microphone access error:', err);
        setMintActivity('error');
        micBtn.style.display = 'none';
        appendMessage('❌ ไม่สามารถเข้าถึงไมโครโฟนได้ค่ะ กรุณาตรวจสอบการตั้งค่าระดับระบบ', 'ai');
    }
}

function resetMicUI(): void {
    micBtn.classList.remove('listening');
    chatInput.placeholder = DEFAULT_PLACEHOLDER;
    if (voiceMode !== 'speech' && (!mediaRecorder || mediaRecorder.state === 'inactive')) {
        setMintActivity('idle');
    }
}

async function sendVoiceMessage(base64Audio: string): Promise<void> {
    showTyping();
    chatInput.placeholder = 'Processing voice...';
    setMintActivity('thinking');
    try {
        const response = await window.api.sendMessage('', null, base64Audio);
        removeTyping();
        const msgDiv = await appendAiMessages(response.response, { allowDelay: true, timestamp: new Date().toISOString() });
        await speakText(normalizeAiText(response.response), { onEnd: resumeSpeechIfNeeded });
        notifyAiIfNeeded();
        if (response.approval?.required) {
            appendApprovalCard(msgDiv!, response.approval);
        } else if (response.action && response.action.type !== 'none') {
            appendActionCard(msgDiv!, response.action);
        }
    } catch (error) {
        removeTyping();
        setMintActivity('error');
        appendMessage('ขออภัยค่ะ เกิดข้อผิดพลาดในการประมวลผลเสียง', 'ai');
        console.error(error);
        resumeSpeechIfNeeded();
    } finally {
        resetMicUI();
    }
}

function fallbackToMediaRecorder(): void {
    if (voiceMode === 'recorder') return;
    isSpeechStreaming = false;
    speechPausedForReply = false;
    resumeSpeechAfterResponse = false;
    voiceMode = 'recorder';
    try { if (speechRecognition) speechRecognition.stop(); } catch (_) {}
    if (mediaRecorder && mediaRecorder.state === 'inactive') {
        audioChunks = [];
        mediaRecorder.start();
    }
}

setupMediaRecorder();
if (SpeechRecognitionCtor) setupSpeechRecognition();

micBtn.addEventListener('click', (e: MouseEvent) => {
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
            try { speechRecognition.start(); } catch (err) {
                console.error('Speech recognition start error:', err);
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

// ─── Speech Synthesis ─────────────────────────────────────────────────────────

let currentAudioPlayer: HTMLAudioElement | null = null;

function speakText(text: string, options: { onEnd?: () => void } = {}): Promise<void> {
    setMintActivity('speaking');
    const onEnd = typeof options.onEnd === 'function' ? options.onEnd : () => {};
    const wrappedOnEnd = () => {
        if (window.Live2DManager) window.Live2DManager.stopLipSync();
        onEnd();
    };

    return new Promise(async (resolve) => {
        if (!enableVoiceReply) { setMintActivity('idle'); wrappedOnEnd(); return resolve(); }
        if (currentAudioPlayer) {
            currentAudioPlayer.pause();
            currentAudioPlayer.currentTime = 0;
            currentAudioPlayer = null;
        }
        if (window.Live2DManager) window.Live2DManager.stopLipSync();
        if ('speechSynthesis' in window) window.speechSynthesis.cancel();
        if (!text || !text.trim()) { setMintActivity('idle'); wrappedOnEnd(); return resolve(); }
        if (window.Live2DManager) window.Live2DManager.startLipSync();

        try {
            if (ttsProvider !== 'native') {
                const urls = await window.api.getTtsUrls(text);
                if (urls && urls.length > 0) {
                    let i = 0;
                    const playNext = () => {
                        if (i >= urls.length) { setMintActivity('idle'); wrappedOnEnd(); return resolve(); }
                        const audio = new Audio((urls[i] as any).url ?? urls[i]);
                        audio.volume = ttsVolume;
                        audio.playbackRate = ttsSpeed;
                        currentAudioPlayer = audio;
                        audio.onended = () => { i++; playNext(); };
                        audio.onerror = () => { i++; playNext(); };
                        audio.play().catch(() => fallbackSpeak(text, wrappedOnEnd, resolve));
                    };
                    playNext();
                    return;
                }
            }
        } catch (err) {
            console.error('Cloud TTS Error, falling back to local:', err);
        }
        fallbackSpeak(text, wrappedOnEnd, resolve);
    });
}

function fallbackSpeak(text: string, onEnd: () => void, resolve: () => void): void {
    if (!('speechSynthesis' in window)) { setMintActivity('idle'); onEnd(); resolve(); return; }
    window.speechSynthesis.cancel();
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.lang = 'th-TH';
    utterance.volume = ttsVolume;
    utterance.rate = ttsSpeed;
    utterance.pitch = ttsPitch;
    let finished = false;
    const done = () => { if (finished) return; finished = true; setMintActivity('idle'); onEnd(); resolve(); };
    utterance.onend = done;
    utterance.onerror = done;
    window.speechSynthesis.speak(utterance);
}

// ─── Window Controls ──────────────────────────────────────────────────────────

minimizeBtn.addEventListener('click', () => window.api.minimizeWindow());
closeBtn.addEventListener('click', () => window.api.quitApp());
maximizeBtn.addEventListener('click', () => window.api.maximizeWindow());

function openSettings(): void { window.api.openSettings(); }
settingsBtn.addEventListener('click', openSettings);
sidebarSettingsBtn?.addEventListener('click', openSettings);

// ─── Pictures Library ─────────────────────────────────────────────────────────

async function renderPicturesLibrary(): Promise<void> {
    if (!picturesGrid || !picturesEmpty) return;
    picturesGrid.innerHTML = '';
    const pictures: any[] = await window.api.listSavedPictures();
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

async function openPicturesLibrary(): Promise<void> {
    if (!appBody || !picturesLibrary) return;
    picturesLibrary.hidden = false;
    requestAnimationFrame(() => appBody!.classList.add('pictures-open'));
    sidebarChatBtn?.classList.remove('is-active');
    sidebarPicturesBtn?.classList.add('is-active');
    await renderPicturesLibrary();
}

function closePicturesLibrary(): void {
    if (!appBody || !picturesLibrary) return;
    appBody.classList.remove('pictures-open');
    setTimeout(() => { if (!appBody!.classList.contains('pictures-open')) picturesLibrary!.hidden = true; }, 240);
    sidebarChatBtn?.classList.add('is-active');
    sidebarPicturesBtn?.classList.remove('is-active');
}

sidebarChatBtn?.addEventListener('click', closePicturesLibrary);
sidebarPicturesBtn?.addEventListener('click', openPicturesLibrary);
picturesCloseBtn?.addEventListener('click', closePicturesLibrary);

// ─── Sidebar Toggle ───────────────────────────────────────────────────────────

function setSidebarCollapsed(isCollapsed: boolean): void {
    if (!appBody || !sidebarToggleBtn) return;
    appBody.classList.toggle('sidebar-collapsed', isCollapsed);
    sidebarToggleBtn.setAttribute('aria-expanded', String(!isCollapsed));
    sidebarToggleBtn.setAttribute('aria-label', isCollapsed ? 'Expand sidebar' : 'Collapse sidebar');
    sidebarToggleBtn.setAttribute('title', isCollapsed ? 'Expand sidebar' : 'Collapse sidebar');
}

if (appBody && sidebarToggleBtn) {
    setSidebarCollapsed(true);
    sidebarToggleBtn.addEventListener('click', () => {
        setSidebarCollapsed(!appBody!.classList.contains('sidebar-collapsed'));
    });
}

// ─── Utilities ────────────────────────────────────────────────────────────────

function throttle<T extends (...args: any[]) => any>(func: T, limit: number): T {
    let inThrottle = false;
    return function (this: any, ...args: Parameters<T>) {
        if (!inThrottle) {
            func.apply(this, args);
            inThrottle = true;
            setTimeout(() => (inThrottle = false), limit);
        }
    } as T;
}

function formatTime(isoString: string | null | undefined): string {
    if (!isoString) return '';
    try {
        return new Date(isoString).toLocaleTimeString('th-TH', { hour: '2-digit', minute: '2-digit', hour12: false });
    } catch { return ''; }
}

// ─── Vision ───────────────────────────────────────────────────────────────────

visionBtn.addEventListener('click', throttle(async () => { await window.api.startVision(); }, 1000));

window.api.onVisionReady((base64Image: string) => {
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

// ─── Smart Context ────────────────────────────────────────────────────────────

function compactSmartContext(context: any): any {
    if (!context || typeof context !== 'object') return null;
    const activeWindow = context.activeWindow || {};
    const currentApp = context.currentApp || {};
    return {
        capturedAt: context.capturedAt,
        platform: context.platform,
        currentApp: currentApp.name || activeWindow.appName || activeWindow.processName || '',
        processName: currentApp.processName || activeWindow.processName || '',
        pid: currentApp.pid || activeWindow.pid || null,
        activeWindowTitle: activeWindow.title || '',
        browser: context.browser ? {
            title: context.browser.title || '',
            url: context.browser.url || '',
            urlUnavailableReason: context.browser.urlUnavailableReason || '',
        } : null,
        selectedText: context.selectedText || '',
        clipboardText: context.clipboardText || '',
    };
}

function appendSmartContextToMessage(message: string, context: any): string {
    const compact = compactSmartContext(context);
    if (!compact) return message;
    return [message, '', '[SMART_CONTEXT]',
        'Use this structured desktop context together with the attached screenshot. Do not mention it unless it helps answer the user.',
        JSON.stringify(compact, null, 2), '[/SMART_CONTEXT]'].join('\n');
}

function shouldShowAgentActivity(options: Record<string, any> = {}): boolean {
    return options.showAgentActivity !== false && currentSettings.assistantMode === 'agent';
}

function describeSmartContextActivity(context: any, hasScreenshot: boolean): string {
    const compact = compactSmartContext(context) || {};
    const parts: string[] = [];
    if (hasScreenshot) parts.push('screen');
    if (compact.currentApp) parts.push(compact.currentApp);
    if (compact.activeWindowTitle) parts.push(compact.activeWindowTitle);
    if (compact.selectedText) parts.push('selected text');
    if (compact.clipboardText) parts.push('clipboard');
    return parts.slice(0, 3).join(' · ') || 'desktop context';
}

// ─── Agent Activity Card ──────────────────────────────────────────────────────

function createAgentActivityCard(): AgentActivity {
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
        element: messageDiv, list, status,
        add(label: string, state = 'running', detail = ''): HTMLElement {
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
        update(item: HTMLElement | null, state: string, label?: string, detail = ''): void {
            if (!item) return;
            item.dataset.state = state;
            const content = item.querySelector('.agent-activity-text');
            if (content && label) content.textContent = detail ? `${label}: ${detail}` : label;
        },
        finish(state = 'done', label = 'Done'): void {
            status.textContent = label;
            status.dataset.state = state;
        },
    };
}

// ─── Chat History Clear ───────────────────────────────────────────────────────

async function clearChatHistory(confirmMessage = 'Clear current chat history?'): Promise<void> {
    if (!window.confirm(confirmMessage)) return;
    closePicturesLibrary();
    await window.api.resetChat();
    chatContainer.querySelectorAll<HTMLElement>('.message:not(.initial)').forEach((m) => m.remove());
    appendMessage('Chat history cleared. Starting fresh! 🌿', 'ai', null, new Date().toISOString());
}

clearBtn.addEventListener('click', () => clearChatHistory('Clear current chat history?'));
sidebarNewChatBtn?.addEventListener('click', () => clearChatHistory('Start a new chat and clear current history?'));

// ─── Provider Popover ─────────────────────────────────────────────────────────

function formatProviderInfo(providerInfo: any): string {
    if (!providerInfo || typeof providerInfo !== 'object') return '';
    const provider = String(providerInfo.provider || '').trim();
    const model = String(providerInfo.model || '').trim();
    if (!provider && !model) return '';
    return model ? `${provider || 'AI'} • ${model}` : provider;
}

function formatNumber(value: any): string {
    return (Number(value) || 0).toLocaleString('en-US');
}

function summarizeProviderUsage(providerInfo: any): Record<string, number> {
    const usage = Array.isArray(providerInfo?.usage) ? providerInfo.usage : [];
    const selectedProvider = String(providerInfo?.provider || '').trim();
    const selectedModel = String(providerInfo?.model || '').trim();
    const row = usage.find((item: any) => String(item.provider || '') === selectedProvider && String(item.model || '') === selectedModel) || usage[0] || {};
    return {
        requests: Number(row.requests) || 0,
        inputTokens: Number(row.inputTokens) || 0,
        outputTokens: Number(row.outputTokens) || 0,
        reasoningTokens: Number(row.reasoningTokens) || 0,
        cacheReads: Number(row.cacheReads) || 0,
        totalTokens: Number(row.totalTokens) || 0,
    };
}

function closeProviderPopover(): void {
    document.querySelectorAll('.provider-popover').forEach((p) => p.remove());
    document.querySelectorAll('.provider-badge.is-open').forEach((b) => b.classList.remove('is-open'));
}

function createProviderRow(label: string, value: string): HTMLElement {
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

function showProviderPopover(anchor: HTMLElement, providerInfo: any): void {
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
    if (usage.reasoningTokens) popover.appendChild(createProviderRow('Reasoning tokens', formatNumber(usage.reasoningTokens)));
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

// ─── Message Rendering ────────────────────────────────────────────────────────

function splitListOutro(text: string): { main: string; outro: string } {
    const value = String(text || '').trim();
    const markers = [' คุณภีมอยาก', ' อยากให้', ' อยากดู', ' บอกมิ้นท์', ' Would you', ' Do you want', ' Tell me'];
    for (const marker of markers) {
        const index = value.indexOf(marker);
        if (index > 60) return { main: value.slice(0, index).trim(), outro: value.slice(index).trim() };
    }
    return { main: value, outro: '' };
}

function buildAiTextBlocks(text: string): { type: 'bullet' | 'paragraph'; text: string }[] {
    const normalized = normalizeAiText(text).replace(/\r\n/g, '\n').trim();
    if (!normalized) return [];
    const readable = normalized.replace(/\s+(\d+)[.)]\s+/g, '\n$1. ').replace(/\n{3,}/g, '\n\n');
    const blocks: { type: 'bullet' | 'paragraph'; text: string }[] = [];
    const lines = readable.split(/\n+/).map((l) => l.trim()).filter(Boolean);
    for (const line of lines) {
        const numbered = line.match(/^\d+[.)]\s+(.+)$/);
        const bullet = line.match(/^[-*•]\s+(.+)$/);
        if (numbered || bullet) {
            const content = numbered ? numbered[1] : bullet![1];
            const { main, outro } = splitListOutro(content);
            blocks.push({ type: 'bullet', text: main });
            if (outro) blocks.push({ type: 'paragraph', text: outro });
        } else {
            blocks.push({ type: 'paragraph', text: line });
        }
    }
    return blocks;
}

function appendFormattedMessageText(bubble: HTMLElement, text: string, sender: string): void {
    if (sender !== 'ai') {
        const span = document.createElement('span');
        span.textContent = text;
        bubble.appendChild(span);
        return;
    }
    const blocks = buildAiTextBlocks(text);
    if (!blocks.length) return;
    const wrapper = document.createElement('div');
    wrapper.classList.add('formatted-ai-text');
    for (const block of blocks) {
        const item = document.createElement(block.type === 'bullet' ? 'div' : 'p');
        item.classList.add(block.type === 'bullet' ? 'ai-list-item' : 'ai-paragraph');
        if (block.type === 'bullet') {
            const dot = document.createElement('span');
            dot.classList.add('ai-list-bullet');
            dot.textContent = '•';
            const content = document.createElement('span');
            content.textContent = block.text;
            item.appendChild(dot);
            item.appendChild(content);
        } else {
            item.textContent = block.text;
        }
        wrapper.appendChild(item);
    }
    bubble.appendChild(wrapper);
}

function appendMessage(
    text: string,
    sender: 'user' | 'ai',
    base64Image: string | null = null,
    timestamp: string | null = null,
    options: { providerInfo?: any } = {}
): HTMLElement {
    const messageDiv = document.createElement('div');
    messageDiv.classList.add('message', `${sender}-message`);
    const bubbleWrapper = document.createElement('div');
    bubbleWrapper.classList.add('bubble-wrapper');
    const bubble = document.createElement('div');
    bubble.classList.add('message-bubble');

    if (base64Image && sender === 'user') {
        const img = document.createElement('img');
        img.src = base64Image;
        img.style.cssText = 'max-width:100%;border-radius:4px;margin-bottom:8px;display:block';
        bubble.appendChild(img);
    }
    if (text) appendFormattedMessageText(bubble, text, sender);
    bubbleWrapper.appendChild(bubble);

    const providerLabel = sender === 'ai' ? formatProviderInfo(options.providerInfo) : '';
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
                if (providerButton.classList.contains('is-open')) { closeProviderPopover(); return; }
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
    return messageDiv;
}

function normalizeAiText(input: any): string {
    if (Array.isArray(input)) return input.map((item) => (item == null ? '' : String(item).trim())).filter(Boolean).join('\n\n');
    if (input == null) return '';
    return String(input);
}

function splitAiMessages(text: string): string[] {
    const normalized = normalizeAiText(text).trim();
    if (!normalized) return [];
    if (/(^|\s)\d+[.)]\s+/.test(normalized) || /(^|\n)\s*[-*•]\s+/.test(normalized)) return [normalized];
    const byBlankLine = normalized.split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean);
    if (byBlankLine.length > 1) return byBlankLine;
    return autoChunkAiText(normalized);
}

function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

function estimateMessageDelay(text: string): number {
    return Math.min(1600, 260 + Math.min(1200, text.length * 12) + Math.floor(Math.random() * 120));
}

async function appendAiMessages(text: string, options: { allowDelay?: boolean; timestamp?: string; providerInfo?: any } = {}): Promise<HTMLElement | null> {
    const allowDelay = options.allowDelay !== false;
    const timestamp = options.timestamp || new Date().toISOString();
    const providerInfo = options.providerInfo || null;
    const parts = splitAiMessages(text);
    let lastDiv: HTMLElement | null = null;
    for (let index = 0; index < parts.length; index++) {
        if (allowDelay && index > 0) { showTyping(); await sleep(estimateMessageDelay(parts[index])); removeTyping(); }
        const partTimestamp = index === parts.length - 1 ? timestamp : null;
        const partProviderInfo = index === parts.length - 1 ? providerInfo : null;
        lastDiv = appendMessage(parts[index], 'ai', null, partTimestamp, { providerInfo: partProviderInfo });
    }
    return lastDiv;
}

function autoChunkAiText(text: string): string[] {
    const trimmed = text.trim();
    if (trimmed.length <= 120) return [trimmed];
    const sentenceMatches = trimmed.match(/[^.!?…\n]+[.!?…]+|[^.!?…\n]+$/g);
    if (!sentenceMatches || sentenceMatches.length <= 1) return [trimmed];
    const bubbles: string[] = [];
    let current = '';
    for (const sentence of sentenceMatches) {
        const next = current ? `${current} ${sentence}` : sentence;
        if (next.length > 180 && current) { bubbles.push(current.trim()); current = sentence; }
        else { current = next; }
    }
    if (current.trim()) bubbles.push(current.trim());
    if (bubbles.length > 3) return [bubbles[0], bubbles[1], bubbles.slice(2).join(' ').trim()].filter(Boolean);
    return bubbles.length > 0 ? bubbles : [trimmed];
}

// ─── Action Cards ─────────────────────────────────────────────────────────────

function formatActionTarget(action: any): string {
    if (!action || typeof action !== 'object') return '';
    if (action.server && action.target) return `${action.server}:${action.target}`;
    if (action.pluginName) return `${action.pluginName} ${action.target || ''}`.trim();
    if (action.target) return String(action.target);
    if (Number.isFinite(action.x) && Number.isFinite(action.y)) return `${action.x}, ${action.y}`;
    return '';
}

function buildFindPathDetail(action: any): string {
    const target = formatActionTarget(action);
    const typeLabel = action.pathType && action.pathType !== 'any' ? ` (${action.pathType})` : '';
    return target ? `${target}${typeLabel}` : typeLabel.trim();
}

function buildMouseDetail(action: any): string {
    const point = formatActionTarget(action);
    const button = action.button ? `button ${action.button}` : 'left button';
    return point ? `${point} · ${button}` : button;
}

function describeActionActivity(action: any): string {
    if (!action || action.type === 'none') return 'No desktop action';
    const meta = getActionCardMeta(action);
    return meta.detail ? `${meta.title} · ${meta.detail}` : meta.title;
}

function getActionCardMeta(action: any): ActionCardMeta {
    const target = formatActionTarget(action);
    const type = action?.type || 'unknown';
    const map: Record<string, [string, string, string]> = {
        open_url:         ['🌐', 'Opened URL', target],
        search:           ['🔍', 'Searched the web', target],
        open_app:         ['🚀', 'Launched app', target],
        web_automation:   ['🧭', 'Ran browser automation', target],
        create_folder:    ['📁', 'Created folder', target],
        open_file:        ['📄', 'Opened file', target],
        open_folder:      ['📂', 'Opened folder', target],
        delete_file:      ['🗑️', 'Deleted file', target],
        find_path:        ['🔎', action.openAfter ? 'Found and opened path' : 'Found path', buildFindPathDetail(action)],
        clipboard_write:  ['📋', 'Updated clipboard', target],
        learn_file:       ['📚', 'Indexed file', target],
        learn_folder:     ['📚', 'Indexed folder', target],
        system_info:      ['💻', target ? 'Checked weather' : 'Checked system info', target],
        plugin:           ['🔌', 'Ran plugin', target],
        mcp_tool:         ['🧩', 'Called MCP tool', target],
        mouse_move:       ['↗', 'Moved pointer', target],
        mouse_click:      ['☝', 'Clicked screen', buildMouseDetail(action)],
        type_text:        ['⌨', 'Typed text', target],
        key_tap:          ['⌨', 'Pressed key', target],
        system_automation:['⚙', 'Changed system setting', target],
    };
    const [icon, title, detail] = map[type] || ['⚡', `Ran action: ${type}`, target || 'No target'];
    return { icon, title, detail };
}

function appendActionCard(messageDiv: HTMLElement, action: any): void {
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

// ─── Approval Cards ───────────────────────────────────────────────────────────

function getApprovalCopy(approval: any): { title: string; body: string; reason: string; approveLabel: string } {
    const action = approval?.action || {};
    const actionType = action.type || 'unknown';
    const target = formatActionTarget(action);
    const isDangerous = approval?.tier === 'dangerous';
    return {
        title: isDangerous ? 'Dangerous action requires approval' : 'Action requires approval',
        body: target ? `${actionType}: ${target}` : actionType,
        reason: approval?.reason || 'This action needs your permission before Mint can run it.',
        approveLabel: isDangerous ? 'Allow Dangerous Action' : 'Allow Action',
    };
}

function appendApprovalCard(messageDiv: HTMLElement, approval: any, activity: AgentActivity | null = null): void {
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
    const setDone = (message: string, state: string) => {
        approveBtn.disabled = true;
        cancelBtn.disabled = true;
        card.dataset.state = state;
        reason.textContent = message;
    };
    approveBtn.addEventListener('click', async () => {
        approveBtn.disabled = true;
        cancelBtn.disabled = true;
        reason.textContent = 'Running approved action...';
        const runStep = activity?.add('Running approved action', 'running', describeActionActivity(approval.action)) ?? null;
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
        } catch (error: any) {
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

// ─── Typing Indicator ─────────────────────────────────────────────────────────

function showTyping(): void {
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

function removeTyping(): void {
    document.getElementById('typing-indicator')?.remove();
}

function scrollToBottom(): void {
    chatContainer.scrollTop = chatContainer.scrollHeight;
}

// ─── Live2D Loader ────────────────────────────────────────────────────────────

function loadScript(src: string): Promise<void> {
    return new Promise((resolve, reject) => {
        if (document.querySelector(`script[src="${src}"]`)) { resolve(); return; }
        const script = document.createElement('script');
        script.src = src;
        script.onload = () => resolve();
        script.onerror = () => reject(new Error(`Failed to load script: ${src}`));
        document.body.appendChild(script);
    });
}

function hideStartupLoading(): void {
    appContainer?.classList.remove('is-loading');
    if (!startupLoading) return;
    startupLoading.classList.add('is-hidden');
    setTimeout(() => startupLoading!.remove(), 400);
}

async function loadLive2DWhenIdle(): Promise<void> {
    if (!modelMount || window.Live2DManager) { hideStartupLoading(); return; }
    try {
        await loadScript('../../node_modules/@hazart-pkg/live2d-core/live2dcubismcore.min.js');
        await loadScript('../../node_modules/pixi.js/dist/browser/pixi.min.js');
        await loadScript('../../node_modules/pixi-live2d-display/dist/cubism4.min.js');
        await loadScript('live2d_manager.js');
        if (window.Live2DManager) {
            await window.Live2DManager.loadModel(modelMount, modelStatus, modelShell);
            applyModelPanelControlState();
        }
    } catch (err) {
        console.error('[Live2D] Deferred load failed:', err);
        if (modelStatus) { modelStatus.classList.add('is-error'); modelStatus.textContent = 'Live2D model unavailable.'; }
    } finally {
        hideStartupLoading();
    }
}

// ─── Chat History Loader ──────────────────────────────────────────────────────

async function loadChatHistory(): Promise<void> {
    try {
        const history: any[] = await window.api.getChatHistory();
        const initial = chatContainer.querySelector<HTMLElement>('.message.initial');
        if (!Array.isArray(history) || history.length === 0) {
            if (initial) { initial.style.display = 'flex'; initial.style.opacity = '1'; }
            return;
        }
        initial?.remove();
        for (const item of history) {
            if (!item || typeof item.text !== 'string' || !item.text.trim()) continue;
            const sender: 'user' | 'ai' = item.sender === 'user' ? 'user' : 'ai';
            if (sender === 'user' && !String(item.text).startsWith('Model interaction:')) rememberConversationLanguage(item.text);
            appendMessage(item.text, sender, null, item.timestamp, { providerInfo: sender === 'ai' ? item.providerInfo : null });
        }
    } catch (error) {
        console.error('Failed to load chat history:', error);
    }
}

// ─── Send Message ─────────────────────────────────────────────────────────────

async function sendTextMessage(text: string, options: { allowSmartContext?: boolean; includePendingImage?: boolean; displayText?: string; trackLanguage?: boolean; showAgentActivity?: boolean } = {}): Promise<void> {
    const cleanText = (text || '').trim();
    const allowSmartContext = options.allowSmartContext !== false;
    const includePendingImage = options.includePendingImage !== false;
    const displayText = options.displayText !== undefined ? options.displayText : cleanText;
    const trackLanguage = options.trackLanguage !== false;
    if (!cleanText && (!includePendingImage || !currentBase64Image)) return;

    let imageToSend = includePendingImage ? currentBase64Image : null;
    chatInput.value = '';
    if (includePendingImage) {
        currentBase64Image = null;
        imagePreviewContainer.style.display = 'none';
        imagePreview.src = '';
    }

    const now = new Date().toISOString();
    appendMessage(displayText, 'user', imageToSend, now);
    if (trackLanguage) rememberConversationLanguage(displayText || cleanText);

    const activity = shouldShowAgentActivity(options) ? createAgentActivityCard() : null;
    const contextStep = activity?.add('Preparing desktop context', 'running') ?? null;
    showTyping();
    setMintActivity('thinking');

    let messageToSend = cleanText;
    const smartToggle = document.getElementById('smart-context-toggle') as HTMLInputElement | null;
    if (allowSmartContext && smartToggle?.checked && !imageToSend) {
        try {
            const [silentCapture, smartContext] = await Promise.all([
                window.api.captureSilentScreen(),
                window.api.getSmartContext ? window.api.getSmartContext() : Promise.resolve(null),
            ]);
            if (silentCapture) imageToSend = silentCapture;
            if (smartContext) messageToSend = appendSmartContextToMessage(cleanText, smartContext);
            activity?.update(contextStep, 'done', 'Read Smart Context', describeSmartContextActivity(smartContext, Boolean(silentCapture)));
        } catch (err: any) {
            activity?.update(contextStep, 'error', 'Smart Context unavailable', err.message || '');
        }
    } else {
        activity?.update(contextStep, 'skipped', 'Smart Context skipped', imageToSend ? 'image already attached' : 'toggle is off');
    }

    hideProactiveBar();
    const modelStep = activity?.add('Waiting for model response', 'running') ?? null;

    try {
        const response = await window.api.sendMessage(messageToSend, imageToSend, null);
        removeTyping();
        activity?.update(modelStep, 'done', 'Model response received');
        if (typeof response.response !== 'string') response.response = normalizeAiText(response.response);

        if (response.action?.type === 'system_info') {
            const infoStep = activity?.add('Running local info action', 'running', describeActionActivity(response.action)) ?? null;
            const city = (response.action.target || '').trim();
            const weatherKeywords = ['date', 'time', 'วัน', 'เวลา', 'today', 'now'];
            const isWeather = city && !weatherKeywords.some((k) => city.toLowerCase().includes(k));
            if (isWeather) {
                const weather = await window.api.getWeather(city);
                response.response += `\n\n🌡️ ${weather.data}`;
                activity?.update(infoStep, 'done', 'Weather info added', city);
            } else {
                const info = await window.api.getSystemInfo();
                const machine = info.machine?.display ? `\n🖥️ รุ่นเครื่อง: ${info.machine.display}` : '';
                const distro = info.distro ? `\nระบบ: ${info.distro}` : '';
                response.response += `\n\n📅 วันนี้: ${info.date}\n⏰ เวลา: ${info.time}${machine}${distro}\n💻 CPU: ${info.cpu.model} (${info.cpu.cores} คอร์)\n💻 RAM: ${info.ram.used} / ${info.ram.total} (${info.ram.percent})`;
                activity?.update(infoStep, 'done', 'System info added');
            }
        }

        const msgDiv = await appendAiMessages(response.response, { allowDelay: true, timestamp: response.timestamp, providerInfo: response.providerInfo });
        await speakText(normalizeAiText(response.response), { onEnd: resumeSpeechIfNeeded });
        notifyAiIfNeeded();

        if (response.approval?.required) {
            activity?.add('Selected action', 'approval', describeActionActivity(response.approval.action));
            activity?.add('Waiting for approval', 'running', response.approval.reason || '');
            activity?.finish('waiting', 'Waiting');
            appendApprovalCard(msgDiv!, response.approval, activity);
        } else if (response.action && response.action.type !== 'none' && response.action.type !== 'system_info') {
            activity?.add('Selected action', 'done', describeActionActivity(response.action));
            appendActionCard(msgDiv!, response.action);
            activity?.finish('done', 'Completed');
        } else if (response.action?.type === 'system_info') {
            activity?.add('Selected action', 'done', describeActionActivity(response.action));
            activity?.finish('done', 'Completed');
        } else {
            activity?.add('No desktop action selected', 'done');
            activity?.finish('done', 'Completed');
        }
    } catch (error: any) {
        removeTyping();
        setMintActivity('error');
        activity?.update(modelStep, 'error', 'Model request failed', error.message || '');
        activity?.finish('error', 'Failed');
        appendMessage('Sorry, I encountered an error communicating with the main process.', 'ai');
        console.error(error);
        resumeSpeechIfNeeded();
    }
}

chatForm.addEventListener('submit', throttle(async (e: Event) => {
    e.preventDefault();
    const text = chatInput.value.trim();
    await sendTextMessage(text);
}, 500));

window.addEventListener('live2d-model-interaction', async (event: Event) => {
    const customEvent = event as CustomEvent;
    const prompt = customEvent.detail?.prompt;
    if (!prompt) return;
    setMintActivity('thinking');
    const interactionPrompt = `${prompt}\n\n${buildInteractionLanguageInstruction()}`;
    const displayPrefix = lastConversationLanguage === 'thai' ? 'แตะโมเดล' : 'Model interaction';
    await sendTextMessage(interactionPrompt, {
        allowSmartContext: false,
        includePendingImage: false,
        trackLanguage: false,
        displayText: `${displayPrefix}: ${customEvent.detail.label || customEvent.detail.region || 'Interaction'}`,
    });
});

// ─── Image Paste & Drag-n-Drop ────────────────────────────────────────────────

function handleImageFile(file: File | null): void {
    if (!file || !file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = (e: ProgressEvent<FileReader>) => {
        currentBase64Image = e.target!.result as string;
        imagePreview.src = currentBase64Image;
        imagePreviewContainer.style.display = 'block';
        chatInput.focus();
    };
    reader.readAsDataURL(file);
}

chatInput.addEventListener('paste', (e: ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of Array.from(items)) {
        if (item.kind === 'file' && item.type.startsWith('image/')) {
            handleImageFile(item.getAsFile());
            break;
        }
    }
});

const inputArea = document.querySelector('.input-area') as HTMLElement;
inputArea.addEventListener('dragover', (e: DragEvent) => { e.preventDefault(); e.stopPropagation(); inputArea.style.opacity = '0.7'; });
inputArea.addEventListener('dragleave', (e: DragEvent) => { e.preventDefault(); e.stopPropagation(); inputArea.style.opacity = '1'; });
inputArea.addEventListener('drop', (e: DragEvent) => {
    e.preventDefault(); e.stopPropagation();
    inputArea.style.opacity = '1';
    if (e.dataTransfer?.files?.length) handleImageFile(e.dataTransfer.files[0]);
});

// ─── Init ─────────────────────────────────────────────────────────────────────

window.addEventListener('DOMContentLoaded', async () => {
    chatInput.focus();
    await loadTheme();
    setMintActivity('idle');
    await loadChatHistory();
    loadLive2DWhenIdle();
});

// ─── Proactive Notifications ──────────────────────────────────────────────────

window.api.onProactiveNotification((data: any) => {
    if (!data?.message) return;
    appendMessage(data.message, 'ai');
    speakText(data.message);
});

window.addEventListener('focus', () => {
    if (window.api.clearAiNotifications) window.api.clearAiNotifications();
});

document.addEventListener('click', closeProviderPopover);
document.addEventListener('keydown', (event: KeyboardEvent) => { if (event.key === 'Escape') closeProviderPopover(); });

// ─── Proactive Smart Suggestions ──────────────────────────────────────────────

function showProactiveBar(data: ProactiveSuggestionData): void {
    proactiveChips.innerHTML = '';
    proactiveMessage.textContent = data.message || '';
    data.suggestions.forEach((item, index) => {
        const chip = document.createElement('button');
        chip.className = 'suggestion-chip';
        chip.textContent = item.label;
        chip.style.animationDelay = `${index * 60}ms`;
        chip.addEventListener('click', async () => {
            hideProactiveBar();
            if (window.api.recordBehavior) window.api.recordBehavior(`User picked: ${item.label}`);
            showTyping();
            try {
                const result = await window.api.executeProactiveAction(item.action);
                removeTyping();
                const confirmText = result?.message || `เปิด ${item.label} แล้วค่ะ ✅`;
                const msgDiv = appendMessage(confirmText, 'ai');
                speakText(confirmText);
                if (item.action && item.action.type !== 'none') appendActionCard(msgDiv, item.action);
            } catch (err) {
                removeTyping();
                appendMessage('ขออภัยค่ะ เกิดข้อผิดพลาด', 'ai');
                console.error('[Chip] Error:', err);
            }
        });
        proactiveChips.appendChild(chip);
    });
    proactiveBar.style.display = 'none';
    requestAnimationFrame(() => { proactiveBar.style.display = 'block'; });
}

function hideProactiveBar(): void {
    proactiveBar.style.display = 'none';
    proactiveChips.innerHTML = '';
}

window.api.onProactiveSuggestion((data: any) => {
    if (data?.message && Array.isArray(data.suggestions) && data.suggestions.length > 0) {
        showProactiveBar(data);
        notifyAiIfNeeded();
    }
});

proactiveDismissBtn.addEventListener('click', hideProactiveBar);

const smartContextToggle = document.getElementById('smart-context-toggle') as HTMLInputElement | null;
smartContextToggle?.addEventListener('change', () => {
    window.api.toggleProactive(smartContextToggle!.checked);
});

// ─── Live2D Model Panel Controls ──────────────────────────────────────────────

const toggleModelBtn = document.getElementById('toggle-model-btn') as HTMLButtonElement | null;
const assistantWorkspace = document.querySelector('.assistant-workspace') as HTMLElement | null;
const modelLockBtn = document.getElementById('model-lock-btn') as HTMLButtonElement | null;
const modelScaleSlider = document.getElementById('model-scale-slider') as HTMLInputElement | null;
const modelScaleValue = document.getElementById('model-scale-value') as HTMLElement | null;
const modelScaleResetBtn = document.getElementById('model-scale-reset-btn') as HTMLButtonElement | null;
const modelBgBtn = document.getElementById('model-bg-btn') as HTMLButtonElement | null;
const layoutPresetBtns = document.querySelectorAll<HTMLButtonElement>('.layout-preset-btn');

const modelBgClasses = ['model-bg-default', 'model-bg-clear', 'model-bg-grid', 'model-bg-stage'];
const modelBgLabels = ['Default background', 'Clear background', 'Grid background', 'Stage background'];
const workspaceLayoutClasses = ['layout-chat'];
const workspaceLayoutPresets = ['companion', 'chat'];

function setModelHidden(isHidden: boolean): void {
    if (!assistantWorkspace || !toggleModelBtn) return;
    assistantWorkspace.classList.toggle('model-hidden', isHidden);
    toggleModelBtn.classList.toggle('active', isHidden);
    toggleModelBtn.setAttribute('aria-pressed', String(isHidden));
    localStorage.setItem('mint-model-hidden', String(isHidden));
    if (!isHidden && window.Live2DManager?.model) {
        setTimeout(() => { window.dispatchEvent(new Event('resize')); window.Live2DManager?.fitModelToMount?.(); }, 450);
    }
}

function setModelPositionLocked(isLocked: boolean): void {
    localStorage.setItem('mint-model-position-locked', String(isLocked));
    modelLockBtn?.classList.toggle('is-active', isLocked);
    modelLockBtn?.setAttribute('aria-pressed', String(isLocked));
    modelLockBtn?.setAttribute('title', isLocked ? 'Unlock model position' : 'Lock model position');
    if (window.Live2DManager) window.Live2DManager.setPointerTrackingEnabled(!isLocked);
}

function setModelBackground(index: number): void {
    if (!modelShell) return;
    const normalized = ((Number(index) || 0) + modelBgClasses.length) % modelBgClasses.length;
    modelBgClasses.forEach((cls) => modelShell!.classList.remove(cls));
    if (normalized > 0) modelShell.classList.add(modelBgClasses[normalized]);
    localStorage.setItem('mint-model-background', String(normalized));
    modelBgBtn?.setAttribute('title', modelBgLabels[normalized]);
}

function setModelScale(value: number): void {
    const next = Math.max(78, Math.min(128, Number(value) || 100));
    localStorage.setItem('mint-model-scale', String(next));
    if (modelScaleSlider) modelScaleSlider.value = String(next);
    if (modelScaleValue) modelScaleValue.textContent = `${(next / 100).toFixed(2)}x`;
    if (window.Live2DManager) window.Live2DManager.setZoomMultiplier(next / 100);
}

function setWorkspaceLayout(layout: string): void {
    if (!assistantWorkspace) return;
    const normalized = workspaceLayoutPresets.includes(layout) ? layout : 'companion';
    workspaceLayoutClasses.forEach((cls) => assistantWorkspace!.classList.remove(cls));
    if (normalized !== 'companion') assistantWorkspace.classList.add(`layout-${normalized}`);
    localStorage.setItem('mint-workspace-layout', normalized);
    layoutPresetBtns.forEach((button) => {
        const isActive = button.dataset.layoutPreset === normalized;
        button.classList.toggle('is-active', isActive);
        button.setAttribute('aria-pressed', String(isActive));
    });
}

function applyModelPanelControlState(): void {
    setModelPositionLocked(localStorage.getItem('mint-model-position-locked') === 'true');
    setModelBackground(Number(localStorage.getItem('mint-model-background') || 0));
    setModelScale(Number(localStorage.getItem('mint-model-scale') || 100));
    setWorkspaceLayout(localStorage.getItem('mint-workspace-layout') || 'companion');
}

if (toggleModelBtn && assistantWorkspace) {
    toggleModelBtn.addEventListener('click', () => setModelHidden(!assistantWorkspace!.classList.contains('model-hidden')));
    const saved = localStorage.getItem('mint-model-hidden');
    setModelHidden(saved === null || saved === 'true');
}

modelLockBtn?.addEventListener('click', () => setModelPositionLocked(localStorage.getItem('mint-model-position-locked') !== 'true'));
modelScaleSlider?.addEventListener('input', (e: Event) => setModelScale(Number((e.target as HTMLInputElement).value)));
modelScaleResetBtn?.addEventListener('click', () => setModelScale(100));
modelBgBtn?.addEventListener('click', () => setModelBackground(Number(localStorage.getItem('mint-model-background') || 0) + 1));
layoutPresetBtns.forEach((btn) => btn.addEventListener('click', () => setWorkspaceLayout(btn.dataset.layoutPreset ?? 'companion')));

applyModelPanelControlState();

// ─── Live2D Interaction Controls ──────────────────────────────────────────────

const changeExpressionBtn = document.getElementById('change-expression-btn') as HTMLButtonElement | null;
changeExpressionBtn?.addEventListener('click', () => { if (window.Live2DManager) window.Live2DManager.cycleExpression(); });

const accessoryStorageKey = 'mint-live2d-accessories';
const accessoryCycleBtn = document.getElementById('accessory-cycle-btn') as HTMLButtonElement | null;
const accessoryCycleLabel = document.getElementById('accessory-cycle-label') as HTMLElement | null;
const accessoryCycleOrder: (string | null)[] = [null, 'glasses', 'pen', 'cat'];
const accessoryLabels: Record<string, string> = { glasses: 'Glasses', pen: 'Pen', cat: 'Cat' };
let savedAccessories: Record<string, boolean> = {};
try { savedAccessories = JSON.parse(localStorage.getItem(accessoryStorageKey) || '{}') || {}; } catch { savedAccessories = {}; }

const getSavedAccessoryId = (): string | null => accessoryCycleOrder.find((id) => id && savedAccessories[id] === true) ?? null;

function updateAccessoryCycleButton(accessoryId: string | null): void {
    if (!accessoryCycleBtn) return;
    const isActive = Boolean(accessoryId);
    const label = accessoryId ? accessoryLabels[accessoryId] : 'Accessory';
    accessoryCycleBtn.classList.toggle('active', isActive);
    accessoryCycleBtn.setAttribute('aria-pressed', String(isActive));
    accessoryCycleBtn.title = `Accessory: ${label}`;
    if (accessoryCycleLabel) accessoryCycleLabel.textContent = label;
}

let currentAccessoryId: string | null = getSavedAccessoryId();
updateAccessoryCycleButton(currentAccessoryId);

accessoryCycleBtn?.addEventListener('click', () => {
    const currentIndex = accessoryCycleOrder.indexOf(currentAccessoryId);
    currentAccessoryId = accessoryCycleOrder[(currentIndex + 1) % accessoryCycleOrder.length];
    updateAccessoryCycleButton(currentAccessoryId);
    if (window.Live2DManager) {
        window.Live2DManager.setExclusiveAccessory(currentAccessoryId, true);
    } else {
        savedAccessories = {};
        if (currentAccessoryId) savedAccessories[currentAccessoryId] = true;
        localStorage.setItem(accessoryStorageKey, JSON.stringify(savedAccessories));
    }
});

const toggleInteractionBtn = document.getElementById('toggle-interaction-btn') as HTMLButtonElement | null;
if (toggleInteractionBtn) {
    const savedEnabled = localStorage.getItem('mint-model-interaction-enabled') !== 'false';
    toggleInteractionBtn.classList.toggle('active', savedEnabled);
    toggleInteractionBtn.setAttribute('aria-pressed', String(savedEnabled));
    if (window.Live2DManager) window.Live2DManager.setInteractionEnabled(savedEnabled);
    toggleInteractionBtn.addEventListener('click', () => {
        const isEnabled = !toggleInteractionBtn.classList.contains('active');
        toggleInteractionBtn.classList.toggle('active', isEnabled);
        toggleInteractionBtn.setAttribute('aria-pressed', String(isEnabled));
        if (window.Live2DManager) { window.Live2DManager.setInteractionEnabled(isEnabled, true); }
        else { localStorage.setItem('mint-model-interaction-enabled', String(isEnabled)); }
    });
}

const interactionGuideBtn = document.getElementById('interaction-guide-btn') as HTMLButtonElement | null;
if (interactionGuideBtn && modelShell) {
    const savedGuideVisible = localStorage.getItem('mint-interaction-guide-visible') === 'true';
    modelShell.classList.toggle('show-interaction-guide', savedGuideVisible);
    interactionGuideBtn.classList.toggle('active', savedGuideVisible);
    interactionGuideBtn.addEventListener('click', () => {
        const isVisible = modelShell!.classList.toggle('show-interaction-guide');
        interactionGuideBtn.classList.toggle('active', isVisible);
        localStorage.setItem('mint-interaction-guide-visible', String(isVisible));
    });
}

// ─── Spotlight Integration ────────────────────────────────────────────────────

window.api.onSpotlightToChat((query: string) => {
    chatInput.value = query;
    chatForm.dispatchEvent(new Event('submit'));
});
