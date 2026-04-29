/**
 * Mint CLI - Gemini-style TUI using blessed
 * Provides a rich terminal UI with chat history, input box, and status bar
 */
const blessed = require('blessed');
const path = require('path');
const { execSync } = require('child_process');
const { readConfig } = require('../System/config_manager');

const SLASH_COMMANDS = [
    { name: '/code',   desc: 'Force workspace code mode for a task' },
    { name: '/models', desc: 'List or switch Gemini models' },
    { name: '/config', desc: 'Show current configuration' },
    { name: '/copy',   desc: 'Copy last response to clipboard' },
    { name: '/clear',  desc: 'Clear conversation history' },
    { name: '/reset',  desc: 'Reset conversation history' },
    { name: '/help',   desc: 'Show help information' },
    { name: '/exit',   desc: 'Exit Mint' }
];

/**
 * Creates and returns the Mint chat TUI screen
 * @param {Object} options
 * @param {Function} options.onSubmit - Called with (userInput: string) when user sends a message
 * @param {Function} options.onExit  - Called when user exits
 * @returns {{ screen, appendMessage, setThinking }}
 */
function createChatUI({ onSubmit, onExit }) {
    const config = readConfig();
    const modelName = config.geminiModel || 'gemini';
    const workspaceName = path.basename(process.cwd());
    const HINT_DEFAULT = `{gray-fg}  Enter send  ·  Ctrl+Y copy  ·  /help commands{/}`;
    const INPUT_FG = '#f8fafc';
    const INPUT_BG = '#10141c';

    // ─── Screen ───────────────────────────────────────────────────────────────
    const screen = blessed.screen({
        smartCSR: true,
        fullUnicode: true,
        title: 'Mint CLI',
        mouse: true
    });

    // ─── Banner ───────────────────────────────────────────────────────────────
    const banner = blessed.box({
        top: 0, left: 1, width: '100%-2', height: 4,
        tags: true,
        padding: { left: 1, right: 1 },
        style: { bg: 'default', fg: '#d7dde8' }
    });
    banner.setContent([
        `{#88e0b0-fg} __  __ _       _    ___ _    ___ {/}`,
        `{#88e0b0-fg}|  \\/  (_)_ __ | |_ / __| |  |_ _|{/}`,
        `{#88e0b0-fg}| |\\/| | | '_ \\|  _| (__| |__ | | {/}`,
        `{#88e0b0-fg}|_|  |_|_|_| |_|\\__|\\___|____|___|{/}`
    ].join('\n'));

    const subBanner = blessed.box({
        top: 4, left: 2, width: '100%-4', height: 2,
        tags: true,
        content: `{gray-fg}Type naturally to chat. Coding requests can auto-enter {/}{#ffd166-fg}Code Mode{/}{gray-fg}. Use {/}{#88e0b0-fg}/help{/}{gray-fg}, {/}{#88e0b0-fg}/code{/}{gray-fg}, or {/}{#88e0b0-fg}Esc{/}{gray-fg}.{/}`,
        style: { bg: 'default', fg: '#9aa6bf' }
    });

    // ─── Chat log (scrollable) ────────────────────────────────────────────────
    const chatBox = blessed.log({
        top: 6, left: 1, width: '100%-2',
        bottom: 8,
        tags: true,
        scrollable: true,
        alwaysScroll: true,
        scrollbar: { ch: '┃', style: { fg: '#335d52' } },
        style: { bg: '#171b24', fg: '#ffffff', border: { fg: '#2f3747' } },
        mouse: true,
        scrollable: true,
        border: { type: 'line' },
        padding: { left: 1, right: 1, top: 0, bottom: 0 },
        label: ' Conversation '
    });

    // ─── Hint bar ─────────────────────────────────────────────────────────────
    const hintBar = blessed.box({
        bottom: 6, left: 1, width: '100%-2', height: 1,
        tags: true,
        content: HINT_DEFAULT,
        style: { bg: 'default' }
    });

    // ─── Input area ───────────────────────────────────────────────────────────
    const inputBox = blessed.textbox({
        bottom: 3, left: 1, width: '100%-2', height: 3,
        tags: false,
        inputOnFocus: true,
        keys: true,
        style: {
            bg: INPUT_BG,
            fg: INPUT_FG,
            border: { fg: '#335d52' },
            focus: {
                fg: INPUT_FG,
                bg: INPUT_BG,
                border: { fg: '#88e0b0' }
            }
        },
        border: { type: 'line' },
        padding: { left: 1 },
        label: ' Message '
    });

    // ─── Placeholder (SIBLING widget floating over input content area) ─────────
    // inputBox: bottom=3, height=3, border=1 → content row at bottom=4, left=2
    const placeholderWidget = blessed.text({
        bottom: 4,        // inside input content area (border offset)
        left: 3,
        width: '100%-6',
        height: 1,
        content: '> Ask anything, or describe a coding task for this workspace',
        tags: false,
        style: { fg: '#5d6678', bg: '#10141c' }
    });

    let placeholderVisible = true;

    function hidePlaceholder() {
        if (placeholderVisible) {
            placeholderVisible = false;
            placeholderWidget.hide();
            screen.render();
        }
    }

    function showPlaceholder() {
        if (!placeholderVisible) {
            placeholderVisible = true;
            placeholderWidget.show();
            screen.render();
        }
    }

    function refreshInputStyles() {
        inputBox.style.fg = INPUT_FG;
        inputBox.style.bg = INPUT_BG;
        if (inputBox.style.focus) {
            inputBox.style.focus.fg = INPUT_FG;
            inputBox.style.focus.bg = INPUT_BG;
        }
        if (Array.isArray(inputBox.children)) {
            inputBox.children.forEach((child) => {
                if (child.style) {
                    child.style.fg = INPUT_FG;
                    child.style.bg = INPUT_BG;
                }
            });
        }
        applyTerminalInputAttrs();
    }

    function applyTerminalInputAttrs() {
        try {
            if (!screen || !screen.program || typeof inputBox.sattr !== 'function' || typeof screen.codeAttr !== 'function') {
                return;
            }
            const attr = inputBox.sattr(inputBox.style);
            screen.program.write(screen.codeAttr(attr));
        } catch (_) {}
    }

    // ─── Status bar (3 columns: left / center / right) ──────────────────────
    const statusBar = blessed.box({
        bottom: 0, left: 1, width: '100%-2', height: 3,
        tags: true,
        style: { bg: '#10141c', fg: '#888888' },
        border: { type: 'line', fg: '#222c38' }
    });

    // Left: workspace info
    const statusLeft = blessed.text({
        parent: statusBar,
        top: 0, left: 1,
        width: '33%',
        height: 1,
        tags: true,
        content: `  workspace {bold}(${workspaceName}){/bold}`,
        style: { bg: '#10141c', fg: '#93a0b7' }
    });

    // Center: mode + status
    const statusCenter = blessed.text({
        parent: statusBar,
        top: 0,
        left: 'center',
        width: '44%',
        height: 1,
        align: 'center',
        tags: true,
        content: `{#88aaff-fg}[Chat]{/} {#cc4444-fg}no sandbox{/}`,
        style: { bg: '#10141c', fg: '#888888' }
    });

    // Right: current model
    const statusRight = blessed.text({
        parent: statusBar,
        top: 0, right: 1,
        width: '33%',
        height: 1,
        align: 'right',
        tags: true,
        content: `{#88e0b0-fg}${modelName}{/}`,
        style: { bg: '#10141c', fg: '#88e0b0' }
    });

    let activeMode = 'Chat';

    function formatModeTag(mode) {
        if (mode === 'Code') return `{#ffd166-fg}[Code]{/}`;
        return `{#88aaff-fg}[Chat]{/}`;
    }

    function updateStatusBar(thinkingText = null) {
        if (thinkingText) {
            statusCenter.setContent(`${formatModeTag(activeMode)} {#88e0b0-fg}${thinkingText}{/}`);
        } else {
            statusCenter.setContent(`${formatModeTag(activeMode)} {#cc4444-fg}no sandbox{/}`);
        }
        screen.render();
    }

    function setMode(mode) {
        activeMode = mode === 'Code' ? 'Code' : 'Chat';
        updateStatusBar(null);
    }

    /** Update model name in status bar (called after /models switch) */
    function updateStatusModel(newModel) {
        statusRight.setContent(`{#88e0b0-fg}${newModel}{/}`);
        screen.render();
    }
    updateStatusBar();

    // ─── Append widgets to screen ─────────────────────────────────────────────
    screen.append(banner);
    screen.append(subBanner);
    screen.append(chatBox);
    screen.append(hintBar);
    screen.append(inputBox);
    screen.append(statusBar);
    screen.append(placeholderWidget); // sibling on top of inputBox

    // ─── Suggestion List ──────────────────────────────────────────────────────
    const commandList = blessed.list({
        parent: screen,
        bottom: 6,
        left: 3,
        width: '64%',
        height: 8,
        tags: true,
        keys: false, // We will handle keys manually to keep focus on input
        vi: false,
        hidden: true,
        border: { type: 'line', fg: '#88e0b0' },
        style: {
            bg: '#10141c',
            fg: '#ffffff',
            selected: {
                bg: '#22352f',
                fg: '#88e0b0',
                bold: true
            }
        }
    });

    let activeSuggestions = [];
    const approvalDialog = blessed.question({
        parent: screen,
        tags: true,
        border: { type: 'line', fg: '#88e0b0' },
        style: {
            bg: '#10141c',
            fg: '#ffffff',
            border: { fg: '#88e0b0' }
        },
        width: '80%',
        height: 'shrink',
        top: 'center',
        left: 'center',
        label: ' Approval ',
        hidden: true
    });

    function updateSuggestions(filter = '') {
        activeSuggestions = SLASH_COMMANDS.filter(cmd => 
            cmd.name.toLowerCase().startsWith(filter.toLowerCase())
        );

        if (activeSuggestions.length === 0) {
            commandList.hide();
            screen.render();
            return;
        }

        const items = activeSuggestions.map(cmd => 
            ` {bold}${cmd.name}{/} {gray-fg}${cmd.desc}{/}`
        );
        commandList.setItems(items);
        commandList.select(0);
        commandList.show();
        commandList.setFront();
        screen.render();
    }


    // ─── Input events ─────────────────────────────────────────────────────────

    // ─── Input events ─────────────────────────────────────────────────────────
    let lastListVisible = false;

    // Consolidated key handling
    inputBox.on('element keypress', (el, ch, key) => {
        refreshInputStyles();
        // 1. Handle placeholder visibility
        if (!key.ctrl && !key.meta && key.name !== 'enter' && key.name !== 'tab') {
            if (ch) hidePlaceholder();
        }

        // 2. Handle suggestion list navigation
        if (!commandList.hidden) {
            if (key.name === 'up') {
                commandList.up();
                screen.render();
                return false;
            }
            if (key.name === 'down') {
                commandList.down();
                screen.render();
                return false;
            }
            if (key.name === 'escape') {
                commandList.hide();
                lastListVisible = false;
                screen.render();
                return false;
            }
        }

        // 3. Logic for suggestions and placeholder after key is processed
        setImmediate(() => {
            refreshInputStyles();
            const val = (inputBox.getValue ? inputBox.getValue() : inputBox.value) || '';
            const isCommand = val.startsWith('/') && !val.includes(' ');
            
            // Only render if visibility changed or list is updated
            if (isCommand) {
                updateSuggestions(val);
                lastListVisible = true;
            } else if (lastListVisible) {
                commandList.hide();
                lastListVisible = false;
                screen.render();
            }

            if (!val.trim()) {
                showPlaceholder();
            } else {
                hidePlaceholder();
            }
        });
    });

    inputBox.on('focus', () => {
        refreshInputStyles();
        screen.render();
    });

    inputBox.on('keypress', () => {
        applyTerminalInputAttrs();
    });


    // Submit or Select Suggestion on Enter
    inputBox.key(['enter'], () => {
        if (!commandList.hidden) {
            const selected = activeSuggestions[commandList.selected];
            if (selected) {
                inputBox.setValue(selected.name + ' ');
                commandList.hide();
                hidePlaceholder();
                inputBox.focus();
                refreshInputStyles();
                screen.render();
                return; // Don't submit yet, let user add args or press enter again
            }
        }

        const raw = (inputBox.getValue ? inputBox.getValue() : inputBox.value) || '';
        const text = raw.trim();
        if (!text) {
            inputBox.clearValue();
            showPlaceholder();
            inputBox.focus();
            refreshInputStyles();
            screen.render();
            return;
        }

        // Clear input and restore placeholder
        inputBox.clearValue();
        showPlaceholder();
        inputBox.focus();
        refreshInputStyles();
        screen.render();

        if (text.toLowerCase() === 'exit' || text.toLowerCase() === 'quit') {
            onExit();
            return;
        }

        onSubmit(text);
    });

    // Shift+Enter = newline in input
    // Ctrl+C — double-press to exit
    let ctrlCPressed = false;
    let ctrlCTimer = null;
    screen.key(['C-c'], () => {
        if (ctrlCPressed) {
            clearTimeout(ctrlCTimer);
            onExit();
        } else {
            ctrlCPressed = true;
            hintBar.setContent(`{bold}{yellow-fg}  Press Ctrl+C again to exit.{/}  {gray-fg}(or type 'exit'){/}`);
            screen.render();
            ctrlCTimer = setTimeout(() => {
                ctrlCPressed = false;
                hintBar.setContent(HINT_DEFAULT);
                screen.render();
            }, 2000);
        }
    });

    // ESC — exit immediately
    screen.key(['escape'], () => {
        onExit();
    });

    // ─── Clipboard copy (Ctrl+Y) ──────────────────────────────────────────────
    function copyToClipboard(text) {
        // Try xclip first, then xsel as fallback
        const tools = [
            `echo ${JSON.stringify(text)} | xclip -selection clipboard`,
            `echo ${JSON.stringify(text)} | xsel --clipboard --input`
        ];
        for (const cmd of tools) {
            try {
                execSync(cmd, { stdio: 'pipe' });
                return true;
            } catch (_) {}
        }
        return false;
    }

    function flashHint(msg, durationMs = 2000) {
        hintBar.setContent(msg);
        screen.render();
        setTimeout(() => {
            hintBar.setContent(HINT_DEFAULT);
            screen.render();
        }, durationMs);
    }

    screen.key(['C-y'], () => {
        if (!lastAssistantResponse) {
            flashHint(`{yellow-fg}  No response to copy yet.{/}`);
            return;
        }
        const ok = copyToClipboard(lastAssistantResponse);
        if (ok) {
            flashHint(`{#88e0b0-fg}  ✓ Copied to clipboard!{/}`);
        } else {
            flashHint(`{red-fg}  ✖ Copy failed. Install xclip: sudo apt install xclip{/}`, 3000);
        }
    });

    // ─── Initial render ───────────────────────────────────────────────────────
    inputBox.focus();
    refreshInputStyles();
    screen.render();

    // ─── Public API ───────────────────────────────────────────────────────────

    // Track last assistant response for clipboard copy
    let lastAssistantResponse = '';

    /**
     * @param {'user'|'assistant'|'system'|'error'} role
     * @param {string} text
     * @param {string} timestamp - ISO string or Date object
     */
    function wrapLineSmart(line, width) {
        if (line.length <= width) return [line];
        if (!line.includes(' ')) {
            const pieces = [];
            for (let index = 0; index < line.length; index += width) {
                pieces.push(line.slice(index, index + width));
            }
            return pieces;
        }

        const words = line.split(/\s+/);
        const lines = [];
        let current = '';
        for (const word of words) {
            if (word.length > width) {
                if (current) {
                    lines.push(current);
                    current = '';
                }
                for (let index = 0; index < word.length; index += width) {
                    const slice = word.slice(index, index + width);
                    if (slice.length === width) {
                        lines.push(slice);
                    } else {
                        current = slice;
                    }
                }
                continue;
            }

            if (!current) {
                current = word;
                continue;
            }

            if (`${current} ${word}`.length <= width) {
                current += ` ${word}`;
            } else {
                lines.push(current);
                current = word;
            }
        }
        if (current) lines.push(current);
        return lines;
    }

    function wrapText(str, width) {
        const lines = [];
        const originalLines = String(str).split('\n');
        for (const line of originalLines) {
            if (line.length === 0) {
                lines.push('');
                continue;
            }
            lines.push(...wrapLineSmart(line, width));
        }
        return lines;
    }

    function appendMessage(role, text, timestamp = null) {
        const now = timestamp ? new Date(timestamp) : new Date();
        const timeStr = now.toLocaleTimeString('th-TH', { hour: '2-digit', minute: '2-digit', hour12: false });
        const maxLineWidth = Math.max(screen.width - 20, 36);
        const lines = wrapText(text, maxLineWidth);

        if (role === 'user') {
            chatBox.log(``);
            chatBox.log(` {bold}{#88e0b0-fg}You{/} {gray-fg}${timeStr}{/}`);
            lines.forEach(l => chatBox.log(` {#88e0b0-fg}▏{/} {#ffffff-fg}${l}{/}`));
        } else if (role === 'assistant') {
            lastAssistantResponse = text;
            chatBox.log(``);
            chatBox.log(` {bold}{#d4a8ff-fg}Mint{/} {gray-fg}${timeStr}{/}`);
            lines.forEach(l => chatBox.log(` {#5a456d-fg}▏{/} {#ffffff-fg}${l}{/}`));
        } else if (role === 'system') {
            const displayTag = text.startsWith('Action:')
                ? '{#88e0b0-fg}Action{/}'
                : text.startsWith('[Code]')
                    ? '{#ffd166-fg}Code{/}'
                    : '{#8ba0ff-fg}System{/}';
            const cleanText = text.replace(/^(Action:|System:)\s*/, '');
            const systemLines = wrapText(cleanText, maxLineWidth - 4);
            chatBox.log(``);
            chatBox.log(` {bold}${displayTag}{/}`);
            systemLines.forEach(l => chatBox.log(`   {#95a2b8-fg}${l}{/}`));
        } else if (role === 'error') {
            chatBox.log(``);
            chatBox.log(` {bold}{#ff6b6b-fg}Error{/} {gray-fg}${timeStr}{/}`);
            lines.forEach(l => chatBox.log(` {#7a2e2e-fg}▏{/} {#ff7d7d-fg}${l}{/}`));
        }
        screen.render();
    }

    /** Show/hide thinking indicator in status bar */
    function setThinking(active, secondsElapsed = 0) {
        if (active) {
            updateStatusBar(`Thinking... {gray-fg}(esc to cancel, ${secondsElapsed}s){/}`);
        } else {
            updateStatusBar(null);
        }
    }

    /** Copy last assistant response to clipboard */
    function copyLastResponse() {
        if (!lastAssistantResponse) return false;
        return copyToClipboard(lastAssistantResponse);
    }

    function requestApproval(request) {
        return new Promise((resolve) => {
            const typeLabel = request.type === 'shell'
                ? 'Shell Command'
                : request.type === 'patch'
                    ? 'Patch Edit'
                    : 'File Write';
            const preview = request.preview || request.label || '';
            const message = [
                `{bold}${typeLabel}{/bold}`,
                '',
                preview,
                '',
                'Approve this action?'
            ].join('\n');

            approvalDialog.ask(message, (approved) => {
                inputBox.focus();
                refreshInputStyles();
                screen.render();
                resolve(Boolean(approved));
            });
        });
    }

    return { screen, appendMessage, setThinking, updateStatusModel, copyLastResponse, requestApproval, setMode };
}

module.exports = { createChatUI };
