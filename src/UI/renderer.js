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

// --- Theme Loading ---
function applyTheme(theme, accentColor) {
    document.documentElement.setAttribute('data-theme', theme || 'dark');
    const accent = accentColor || '#8b5cf6';
    document.documentElement.style.setProperty('--accent', accent);
    document.documentElement.style.setProperty('--accent-hover', lightenColor(accent, 20));
}

async function loadTheme() {
    try {
        const config = await window.api.getSettings();
        applyTheme(config.theme, config.accentColor);
    } catch (e) {
        applyTheme('dark', '#8b5cf6');
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
    applyTheme(config.theme, config.accentColor);
});

// --- Speech Recognition Setup ---
const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
let recognition = null;
if (SpeechRecognition) {
    recognition = new SpeechRecognition();
    recognition.continuous = false;
    // Set language to Thai (you can change to 'en-US' or leave empty for auto)
    recognition.lang = 'th-TH'; // Defaulting to Thai as requested
    recognition.interimResults = false;

    recognition.onstart = function () {
        micBtn.classList.add('listening');
        chatInput.placeholder = "Listening...";
    };

    recognition.onresult = function (event) {
        const transcript = event.results[0][0].transcript;
        chatInput.value = transcript;
        // Automatically submit the form once speech is recognized
        chatForm.dispatchEvent(new Event('submit'));
    };

    recognition.onerror = function (event) {
        console.error("Speech recognition error", event.error);
        micBtn.classList.remove('listening');
        chatInput.placeholder = "Type or speak a command...";
    };

    recognition.onend = function () {
        micBtn.classList.remove('listening');
        chatInput.placeholder = "Type or speak a command...";
    };
} else {
    micBtn.style.display = 'none'; // Hide if not supported
    console.warn("Speech Recognition API not supported in this browser.");
}

micBtn.addEventListener('click', (e) => {
    e.preventDefault(); // Prevent form submission if it's inside one
    if (recognition) {
        if (micBtn.classList.contains('listening')) {
            recognition.stop();
        } else {
            try {
                recognition.start();
            } catch (err) {
                console.warn("Speech recognition already started or failed:", err);
            }
        }
    }
});

// --- Speech Synthesis Setup ---
function speakText(text) {
    if ('speechSynthesis' in window) {
        // Stop any currently playing audio
        window.speechSynthesis.cancel();

        const utterance = new SpeechSynthesisUtterance(text);
        utterance.lang = 'th-TH'; // Assuming Thai voice

        // Optional: tweak pitch and rate
        // utterance.pitch = 1.1; 
        // utterance.rate = 1.0;

        window.speechSynthesis.speak(utterance);
    }
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
            appendMessage(item.text, sender);
        }
    } catch (error) {
        console.error('Failed to load chat history:', error);
    }
}

chatForm.addEventListener('submit', throttle(async (e) => {
    e.preventDefault();
    const text = chatInput.value.trim();
    
    // We can send either a text message, an image, or both.
    if (!text && !currentBase64Image) return;

    // Cache the image for sending and UI, then clear
    let imageToSend = currentBase64Image;
    let isSmartContextImage = false;
    
    // Clear input & UI for explicit images
    chatInput.value = '';
    currentBase64Image = null;
    imagePreviewContainer.style.display = 'none';
    imagePreview.src = '';

    // Show user message (with explicit image if available)
    appendMessage(text, 'user', imageToSend);

    // Show typing early so user knows we are processing
    showTyping();

    // Check Smart Context Toggle
    const smartToggle = document.getElementById('smart-context-toggle');
    if (smartToggle && smartToggle.checked && !imageToSend) {
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
        // Send to main process
        const response = await window.api.sendMessage(text, imageToSend);
        removeTyping();

        // Handle system_info action: fetch data and append to AI message
        if (response.action && response.action.type === 'system_info') {
            const city = response.action.target || '';
            if (city) {
                // Weather query
                const weather = await window.api.getWeather(city);
                response.response += `\n\n🌡️ ${weather.data}`;
            } else {
                // General system info
                const info = await window.api.getSystemInfo();
                response.response += `\n\n💻 RAM: ${info.ram.used} / ${info.ram.total} (${info.ram.percent})\n🕐 เวลา: ${info.time} — ${info.date}\n🖥️ CPU: ${info.cpu.cores} cores`;
            }
        }

        // Show AI response
        const msgDiv = appendMessage(response.response, 'ai');

        // Speak AI response
        speakText(response.response);

        // Append action card if applicable
        if (response.action && response.action.type !== 'none' && response.action.type !== 'system_info') {
            appendActionCard(msgDiv, response.action);
        }
    } catch (error) {
        removeTyping();
        appendMessage("Sorry, I encountered an error communicating with the main process.", 'ai');
        console.error(error);
    }
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
