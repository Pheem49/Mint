const chatContainer = document.getElementById('chat-container');
const chatForm = document.getElementById('chat-form');
const chatInput = document.getElementById('chat-input');
const closeBtn = document.getElementById('close-btn');
const micBtn = document.getElementById('mic-btn');

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

        // Show AI response
        const msgDiv = appendMessage(response.response, 'ai');

        // Speak AI response
        speakText(response.response);

        // Append action card if applicable
        if (response.action && response.action.type !== 'none') {
            appendActionCard(msgDiv, response.action);
        }
    } catch (error) {
        removeTyping();
        appendMessage("Sorry, I encountered an error communicating with the main process.", 'ai');
        console.error(error);
    }
});

// Focus input on load
window.addEventListener('DOMContentLoaded', () => {
    chatInput.focus();
});
