/**
 * Mint CLI - Gemini-style TUI using blessed
 * Provides a rich terminal UI with chat history, input box, and status bar
 */
const blessed = require('blessed');
const path = require('path');
const { execSync } = require('child_process');
const { readConfig } = require('../System/config_manager');
const fs = require('fs');

const SLASH_COMMANDS = [
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

    // ─── Screen ───────────────────────────────────────────────────────────────
    const screen = blessed.screen({
        smartCSR: true,
        fullUnicode: true,
        title: 'Mint CLI',
        cursor: {
            artificial: true,
            shape: 'line',
            blink: true,
            color: '#88e0b0'
        }
    });

    // ─── Banner ───────────────────────────────────────────────────────────────
    const banner = blessed.box({
        top: 0, left: 0, width: '100%', height: 9,
        tags: true,
        style: { bg: 'default' }
    });
    banner.setContent([
        `{bold}{#88e0b0-fg}  __  __ _       _      _____ _      _____ {/}`,
        `{bold}{#88e0b0-fg} |  \\/  (_)     | |    / ____| |    |_   _|{/}`,
        `{bold}{#88e0b0-fg} | \\  / |_ _ __ | |_  | |    | |      | |  {/}`,
        `{bold}{#88e0b0-fg} | |\\/| | | '_ \\| __| | |    | |      | |  {/}`,
        `{bold}{#88e0b0-fg} | |  | | | | | | |_  | |____| |____ _| |_ {/}`,
        `{bold}{#88e0b0-fg} |_|  |_|_|_| |_|\\__|  \\_____|______|_____|{/}`,
        ``,
        `{bold}  Welcome to Mint Interactive AI!{/}  {gray-fg}Type '/help' for commands · 'exit' or Esc to quit{/}`
    ].join('\n'));

    // ─── Divider under banner ─────────────────────────────────────────────────
    const divider1 = blessed.line({
        top: 9, left: 0, width: '100%',
        orientation: 'horizontal',
        style: { fg: '#333333' }
    });

    // ─── Chat log (scrollable) ────────────────────────────────────────────────
    const chatBox = blessed.log({
        top: 10, left: 0, width: '100%',
        bottom: 8,  // statusbar(3) + hint(1) + inputBox(3) + divider(1)
        tags: true,
        scrollable: true,
        alwaysScroll: true,
        scrollbar: { ch: '│', style: { fg: '#334433' } },
        style: { bg: 'default', fg: '#ffffff' }
    });

    // ─── Divider above input ──────────────────────────────────────────────────
    const divider2 = blessed.line({
        bottom: 7, left: 0, width: '100%',
        orientation: 'horizontal',
        style: { fg: '#333333' }
    });

    // ─── Hint bar ─────────────────────────────────────────────────────────────
    const hintBar = blessed.box({
        bottom: 6, left: 0, width: '100%', height: 1,
        tags: true,
        content: `{gray-fg}  Shift+Tab to accept edits  ·  /help for slash commands{/}`,
        style: { bg: 'default' }
    });

    // ─── Input area ───────────────────────────────────────────────────────────
    const inputBox = blessed.textarea({
        bottom: 3, left: 0, width: '100%', height: 3,
        tags: false,
        inputOnFocus: true,
        keys: true,
        style: {
            bg: '#111111',
            fg: '#ffffff',
            border: { fg: '#334433' },
            focus: { border: { fg: '#88e0b0' } }
        },
        border: { type: 'line' },
        padding: { left: 1 }
    });

    // ─── Placeholder (SIBLING widget floating over input content area) ─────────
    // inputBox: bottom=3, height=3, border=1 → content row at bottom=4, left=2
    const placeholderWidget = blessed.text({
        bottom: 4,        // inside input content area (border offset)
        left: 2,          // border(1) + padding(1)
        width: '100%-4',  // minus borders and padding
        height: 1,
        content: '> Type your message or @path/to/file',
        tags: false,
        style: { fg: '#555555', bg: '#111111' }
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

    // ─── Status bar (3 columns: left / center / right) ──────────────────────
    const statusBar = blessed.box({
        bottom: 0, left: 0, width: '100%', height: 3,
        tags: true,
        style: { bg: '#111111', fg: '#888888' },
        border: { type: 'line', fg: '#222222' }
    });

    // Left: workspace info
    const statusLeft = blessed.text({
        parent: statusBar,
        top: 0, left: 1,
        width: '33%',
        height: 1,
        tags: true,
        content: `  workspace {bold}(${workspaceName}){/bold}`,
        style: { bg: '#111111', fg: '#888888' }
    });

    // Center: sandbox status
    const statusCenter = blessed.text({
        parent: statusBar,
        top: 0,
        left: 'center',
        width: '34%',
        height: 1,
        align: 'center',
        tags: true,
        content: `{#cc4444-fg}no sandbox{/}`,
        style: { bg: '#111111', fg: '#888888' }
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
        style: { bg: '#111111', fg: '#888888' }
    });

    function updateStatusBar(thinkingText = null) {
        if (thinkingText) {
            statusCenter.setContent(`{#88e0b0-fg}${thinkingText}{/}`);
        } else {
            statusCenter.setContent(`{#cc4444-fg}no sandbox{/}`);
        }
        screen.render();
    }

    /** Update model name in status bar (called after /models switch) */
    function updateStatusModel(newModel) {
        statusRight.setContent(`{#88e0b0-fg}${newModel}{/}`);
        screen.render();
    }
    updateStatusBar();

    // ─── Append widgets to screen ─────────────────────────────────────────────
    screen.append(banner);
    screen.append(divider1);
    screen.append(chatBox);
    screen.append(divider2);
    screen.append(hintBar);
    screen.append(inputBox);
    screen.append(statusBar);
    screen.append(placeholderWidget); // sibling on top of inputBox

    // ─── Suggestion List ──────────────────────────────────────────────────────
    const commandList = blessed.list({
        parent: screen,
        bottom: 6, // Above hintBar
        left: 2,
        width: '70%',
        height: 8,
        tags: true,
        keys: false, // We will handle keys manually to keep focus on input
        vi: false,
        hidden: true,
        border: { type: 'line', fg: '#88e0b0' },
        style: {
            bg: '#111111',
            fg: '#ffffff',
            selected: {
                bg: '#334433',
                fg: '#88e0b0',
                bold: true
            }
        }
    });

    let activeSuggestions = [];

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


    // Submit or Select Suggestion on Enter
    inputBox.key(['enter'], () => {
        if (!commandList.hidden) {
            const selected = activeSuggestions[commandList.selected];
            if (selected) {
                inputBox.setValue(selected.name + ' ');
                commandList.hide();
                hidePlaceholder();
                inputBox.focus();
                screen.render();
                return; // Don't submit yet, let user add args or press enter again
            }
        }

        const raw = (inputBox.getValue ? inputBox.getValue() : inputBox.value) || '';
        const text = raw.trim();
        if (!text) return;

        // Clear input and restore placeholder
        inputBox.clearValue();
        showPlaceholder();
        inputBox.focus();
        screen.render();

        if (text.toLowerCase() === 'exit' || text.toLowerCase() === 'quit') {
            onExit();
            return;
        }

        onSubmit(text);
    });

    // Shift+Enter = newline in input
    inputBox.key(['S-enter'], () => {
        hidePlaceholder();
        const val = (inputBox.getValue ? inputBox.getValue() : inputBox.value) || '';
        inputBox.setValue(val + '\n');
        screen.render();
    });

    // Ctrl+C — double-press to exit
    let ctrlCPressed = false;
    let ctrlCTimer = null;
    const HINT_DEFAULT = `{gray-fg}  Ctrl+Y copy last response  ·  /help for commands{/}`;

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
    screen.render();

    // ─── Public API ───────────────────────────────────────────────────────────

    // Track last assistant response for clipboard copy
    let lastAssistantResponse = '';

    /**
     * @param {'user'|'assistant'|'system'|'error'} role
     * @param {string} text
     * @param {string} timestamp - ISO string or Date object
     */
    function appendMessage(role, text, timestamp = null) {
        const lines = text.split('\n');
        const now = timestamp ? new Date(timestamp) : new Date();
        const timeStr = now.toLocaleTimeString('th-TH', { hour: '2-digit', minute: '2-digit', hour12: false });

        if (role === 'user') {
            chatBox.log(`\n {bold}{#88e0b0-fg}>{/} {#ffffff-fg}${lines[0]}{/}`);
            lines.slice(1).forEach(l => chatBox.log(`   {#ffffff-fg}${l}{/}`));
            chatBox.log(`   {gray-fg}${timeStr}{/}`);
        } else if (role === 'assistant') {
            lastAssistantResponse = text; // track for Ctrl+Y
            chatBox.log(`\n {bold}{#d4a8ff-fg}Mint:{/} {#ffffff-fg}${lines[0]}{/}`);
            lines.slice(1).forEach(l => chatBox.log(`   {#ffffff-fg}${l}{/}`));
            chatBox.log(`   {gray-fg}${timeStr}{/}`);
            chatBox.log('');
        } else if (role === 'system') {
            chatBox.log(`\n {gray-fg}${text}{/}`);
        } else if (role === 'error') {
            chatBox.log(`\n {red-fg}✖ ${text}{/}`);
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

    return { screen, appendMessage, setThinking, updateStatusModel, copyLastResponse };
}

module.exports = { createChatUI };
