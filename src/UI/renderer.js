const chatContainer = document.getElementById('chat-container');
const chatForm = document.getElementById('chat-form');
const chatInput = document.getElementById('chat-input');
const closeBtn = document.getElementById('close-btn');
const clearBtn = document.getElementById('clear-btn');
const settingsBtn = document.getElementById('settings-btn');
const micBtn = document.getElementById('mic-btn');

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

micBtn.addEventListener('click', () => {
    if (recognition) {
        if (micBtn.classList.contains('listening')) {
            recognition.stop();
        } else {
            recognition.start();
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

// Settings button
settingsBtn.addEventListener('click', () => {
    window.api.openSettings();
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

function appendMessage(text, sender) {
    const messageDiv = document.createElement('div');
    messageDiv.classList.add('message', `${sender}-message`);

    const bubble = document.createElement('div');
    bubble.classList.add('message-bubble');
    bubble.textContent = text;

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

chatForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const text = chatInput.value.trim();
    if (!text) return;

    // Clear input
    chatInput.value = '';

    // Show user message
    appendMessage(text, 'user');

    // Show typing
    showTyping();

    try {
        // Send to main process
        const response = await window.api.sendMessage(text);
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
});

// Focus input on load + init theme
window.addEventListener('DOMContentLoaded', async () => {
    chatInput.focus();
    await loadTheme();
    await loadChatHistory();
});
