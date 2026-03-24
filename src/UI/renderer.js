const chatContainer = document.getElementById('chat-container');
const chatForm = document.getElementById('chat-form');
const chatInput = document.getElementById('chat-input');
const closeBtn = document.getElementById('close-btn');
const maximizeBtn = document.getElementById('maximize-btn');
const clearBtn = document.getElementById('clear-btn');
const settingsBtn = document.getElementById('settings-btn');
const micBtn = document.getElementById('mic-btn');
const visionBtn = document.getElementById('vision-btn');
const imagePreviewContainer = document.getElementById('image-preview-container');
const imagePreview = document.getElementById('image-preview');
const removeImageBtn = document.getElementById('remove-image-btn');

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

// --- Theme Loading ---
function applyTheme(theme, accentColor, systemTextColor, config = {}) {
    document.documentElement.setAttribute('data-theme', theme || 'dark');
    const accent = accentColor || '#8b5cf6';
    const textColor = systemTextColor || '#f8fafc';
    document.documentElement.style.setProperty('--accent', accent);
    document.documentElement.style.setProperty('--accent-hover', lightenColor(accent, 20));
    document.documentElement.style.setProperty('--text-main', textColor);

    // Dynamic UI Customizations
    document.documentElement.style.setProperty('--glass-blur', config.glassBlur || 'blur(16px)');
    document.body.style.fontFamily = config.fontFamily || "'Outfit', sans-serif";

    if (theme === 'custom') {
        if (config.customBgStart && config.customBgEnd) {
            const gradient = `linear-gradient(135deg, ${config.customBgStart} 0%, ${config.customBgEnd} 100%)`;
            document.documentElement.style.setProperty('--bg-gradient', gradient);
        }
        if (config.customPanelBg) {
            const rgb = hexToRgb(config.customPanelBg);
            document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`);
        }
    } else {
        document.documentElement.style.removeProperty('--bg-gradient');
        document.documentElement.style.removeProperty('--panel-bg');
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
        applyTheme(config.theme, config.accentColor, config.systemTextColor, config);
        enableVoiceReply = config.enableVoiceReply !== false;
        ttsProvider = config.ttsProvider || 'google';
        ttsVolume = config.ttsVolume !== undefined ? config.ttsVolume : 1.0;
        ttsSpeed = config.ttsSpeed !== undefined ? config.ttsSpeed : 1.0;
        ttsPitch = config.ttsPitch !== undefined ? config.ttsPitch : 1.0;
    } catch (e) {
        applyTheme('dark', '#8b5cf6', '#f8fafc');
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
    applyTheme(config.theme, config.accentColor, config.systemTextColor, config);
    enableVoiceReply = config.enableVoiceReply !== false;
    ttsProvider = config.ttsProvider || 'google';
    ttsVolume = config.ttsVolume !== undefined ? config.ttsVolume : 1.0;
    ttsSpeed = config.ttsSpeed !== undefined ? config.ttsSpeed : 1.0;
    ttsPitch = config.ttsPitch !== undefined ? config.ttsPitch : 1.0;
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
        };

    } catch (err) {
        console.error("Microphone access error:", err);
        micBtn.style.display = 'none';
        appendMessage("❌ ไม่สามารถเข้าถึงไมโครโฟนได้ค่ะ กรุณาตรวจสอบการตั้งค่าระดับระบบ", 'ai');
    }
}

function resetMicUI() {
    micBtn.classList.remove('listening');
    chatInput.placeholder = DEFAULT_PLACEHOLDER;
}

async function sendVoiceMessage(base64Audio) {
    showTyping();
    chatInput.placeholder = "Processing voice...";
    try {
        // Send empty text, but include the audio
        const response = await window.api.sendMessage("", null, base64Audio);
        removeTyping();
        
        // Show AI response
        const msgDiv = await appendAiMessages(response.response, { allowDelay: true });
        await speakText(normalizeAiText(response.response), { onEnd: resumeSpeechIfNeeded });
        notifyAiIfNeeded();

        if (response.action && response.action.type !== 'none') {
            appendActionCard(msgDiv, response.action);
        }
    } catch (error) {
        removeTyping();
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
            if (window.api && window.api.setAiState) window.api.setAiState('listening');
        } else {
            mediaRecorder.stop();
            if (window.api && window.api.setAiState) window.api.setAiState('thinking');
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
        if (window.api && window.api.setAiState) window.api.setAiState('listening');
    } else {
        mediaRecorder.stop();
        if (window.api && window.api.setAiState) window.api.setAiState('thinking');
    }
});

// --- Speech Synthesis Setup ---
let currentAudioPlayer = null;

function speakText(text, options = {}) {
    if (window.api && window.api.setAiState) window.api.setAiState('speaking');
    const onEnd = typeof options.onEnd === 'function' ? options.onEnd : null;
    return new Promise(async (resolve) => {
        if (!enableVoiceReply) {
            if (window.api && window.api.setAiState) window.api.setAiState('idle');
            if (onEnd) onEnd();
            return resolve();
        }

        // Stop any currently playing audio
        if (currentAudioPlayer) {
            currentAudioPlayer.pause();
            currentAudioPlayer.currentTime = 0;
            currentAudioPlayer = null;
        }
        if ('speechSynthesis' in window) {
            window.speechSynthesis.cancel();
        }

        if (!text || !text.trim()) {
            if (window.api && window.api.setAiState) window.api.setAiState('idle');
            if (onEnd) onEnd();
            return resolve();
        }

        try {
            if (ttsProvider !== 'native') {
                const urls = await window.api.getTtsUrls(text);
                if (urls && urls.length > 0) {
                    let i = 0;
                    const playNext = () => {
                        if (i >= urls.length) {
                            if (window.api && window.api.setAiState) window.api.setAiState('idle');
                            if (onEnd) onEnd();
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
                            fallbackSpeak(text, onEnd, resolve);
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
        fallbackSpeak(text, onEnd, resolve);
    });
}

function fallbackSpeak(text, onEnd, resolve) {
    if (!('speechSynthesis' in window)) {
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
        if (window.api && window.api.setAiState) window.api.setAiState('idle');
        if (onEnd) onEnd();
        resolve();
    };

    utterance.onend = done;
    utterance.onerror = done;
    window.speechSynthesis.speak(utterance);
}

// Close window handler (hides to tray)
closeBtn.addEventListener('click', () => {
    window.api.closeWindow();
});

maximizeBtn.addEventListener('click', () => {
    window.api.maximizeWindow();
});

// Settings button
settingsBtn.addEventListener('click', () => {
    window.api.openSettings();
});

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

// Clear chat history
clearBtn.addEventListener('click', async () => {
    await window.api.resetChat();
    // Remove all messages except the initial greeting
    const messages = chatContainer.querySelectorAll('.message:not(.initial)');
    messages.forEach(m => m.remove());
    // Append a clear confirmation
    appendMessage('Chat history cleared. Starting fresh! 🌿', 'ai');
});

function appendMessage(text, sender, base64Image = null) {
    const messageDiv = document.createElement('div');
    messageDiv.classList.add('message', `${sender}-message`);

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
        const textSpan = document.createElement('span');
        textSpan.textContent = text;
        bubble.appendChild(textSpan);
    }

    messageDiv.appendChild(bubble);
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
    const parts = splitAiMessages(text);
    let lastDiv = null;

    for (let index = 0; index < parts.length; index += 1) {
        if (allowDelay && index > 0) {
            showTyping();
            await sleep(estimateMessageDelay(parts[index]));
            removeTyping();
        }
        lastDiv = appendMessage(parts[index], 'ai');
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
    const card = document.createElement('div');
    card.classList.add('action-card');

    let icon = '⚡';
    let text = '';

    if (action.type === 'open_url') {
        icon = '🌐';
        text = `Opened URL: ${action.target}`;
    } else if (action.type === 'open_app') {
        icon = '🚀';
        text = `Launched App: ${action.target}`;
    } else if (action.type === 'search') {
        icon = '🔍';
        text = `Searched info: ${action.target}`;
    } else {
        return; // Do nothing if none or unknown
    }

    card.textContent = `${icon} ${text}`;

    // Append after the bubble
    messageDiv.querySelector('.message-bubble').appendChild(card);
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

async function loadChatHistory() {
    try {
        const history = await window.api.getChatHistory();
        if (!Array.isArray(history) || history.length === 0) {
            return;
        }

        const initial = chatContainer.querySelector('.message.initial');
        if (initial) {
            initial.remove();
        }

        for (const item of history) {
            if (!item || typeof item.text !== 'string' || !item.text.trim()) continue;
            const sender = item.sender === 'user' ? 'user' : 'ai';
            if (sender === 'ai') {
                await appendAiMessages(item.text, { allowDelay: false });
            } else {
                appendMessage(item.text, sender);
            }
        }
    } catch (error) {
        console.error('Failed to load chat history:', error);
    }
}

async function sendTextMessage(text, options = {}) {
    const cleanText = (text || '').trim();
    const allowSmartContext = options.allowSmartContext !== false;

    // We can send either a text message, an image, or both.
    if (!cleanText && !currentBase64Image) return;

    // Cache the image for sending and UI, then clear
    let imageToSend = currentBase64Image;

    // Clear input & UI for explicit images
    chatInput.value = '';
    currentBase64Image = null;
    imagePreviewContainer.style.display = 'none';
    imagePreview.src = '';

    // Show user message (with explicit image if available)
    appendMessage(cleanText, 'user', imageToSend);

    // Show typing early so user knows we are processing
    showTyping();

    // Check Smart Context Toggle
    const smartToggle = document.getElementById('smart-context-toggle');
    if (allowSmartContext && smartToggle && smartToggle.checked && !imageToSend) {
        try {
            const silentCapture = await window.api.captureSilentScreen();
            if (silentCapture) {
                // Set imageToSend so it gets sent to the API, but we already appended the chat bubble
                imageToSend = silentCapture;
            }
        } catch (err) {
            console.error("Smart Context capture failed:", err);
        }
    }

    // Hide proactive bar if user is actively typing a message
    hideProactiveBar();

    try {
        // Send to main process (text, image, audio=null)
        const response = await window.api.sendMessage(cleanText, imageToSend, null);
        removeTyping();

        if (typeof response.response !== 'string') {
            response.response = normalizeAiText(response.response);
        }

        // Handle system_info action: fetch data and append to AI message
        if (response.action && response.action.type === 'system_info') {
            const city = (response.action.target || '').trim();
            // Only treat as weather if city looks like a real location name (not blank, not 'date', not 'time')
            const weatherKeywords = ['date', 'time', 'วัน', 'เวลา', 'today', 'now'];
            const isWeather = city && !weatherKeywords.some(k => city.toLowerCase().includes(k));
            
            if (isWeather) {
                // Weather query
                const weather = await window.api.getWeather(city);
                response.response += `\n\n🌡️ ${weather.data}`;
            } else {
                // General system info (date, time, RAM, CPU)
                const info = await window.api.getSystemInfo();
                response.response += `\n\n📅 วันนี้: ${info.date}\n⏰ เวลา: ${info.time}\n💻 RAM: ${info.ram.used} / ${info.ram.total} (${info.ram.percent})`;
            }
        }

        // Show AI response
        const msgDiv = await appendAiMessages(response.response, { allowDelay: true });

        // Speak AI response
        await speakText(normalizeAiText(response.response), { onEnd: resumeSpeechIfNeeded });
        notifyAiIfNeeded();

        // Append action card if applicable
        if (response.action && response.action.type !== 'none' && response.action.type !== 'system_info') {
            appendActionCard(msgDiv, response.action);
        }
    } catch (error) {
        removeTyping();
        appendMessage("Sorry, I encountered an error communicating with the main process.", 'ai');
        console.error(error);
        resumeSpeechIfNeeded();
    }
}

chatForm.addEventListener('submit', throttle(async (e) => {
    e.preventDefault();
    if (window.api && window.api.setAiState) window.api.setAiState('thinking');
    const text = chatInput.value.trim();
    await sendTextMessage(text);
}, 500));

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
    await loadChatHistory();
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

// Spotlight integration
window.api.onSpotlightToChat((query) => {
    chatInput.value = query;
    chatForm.dispatchEvent(new Event('submit'));
});
