/**
 * Mint CLI - Ink-based UI (ESM-compatible Version)
 * A modern, React-based terminal UI for a better chat experience.
 * Uses dynamic imports to handle ESM dependencies (Ink).
 */
const React = require('react');
const path = require('path');
const { readConfig } = require('../System/config_manager');

// Helper to make element creation less verbose
const h = React.createElement;

const SLASH_COMMANDS = [
    { cmd: '/help', desc: 'Show available commands' },
    { cmd: '/code', desc: 'Force workspace Code Mode' },
    { cmd: '/cd', desc: 'Change current working directory' },
    { cmd: '/models', desc: 'List or switch Gemini models' },
    { cmd: '/config', desc: 'Show current configuration' },
    { cmd: '/copy', desc: 'Copy last response to clipboard' },
    { cmd: '/clear', desc: 'Clear conversation history' },
    { cmd: '/reset', desc: 'Reset conversation history' },
    { cmd: '/agent', desc: 'Switch AI agents (e.g. /agent code)' },
    { cmd: '/workspace', desc: 'Manage registered workspaces' },
    { cmd: '/stats', desc: 'Show system statistics' },
    { cmd: '/review', desc: 'Request second-pass review' },
    { cmd: '/exit', desc: 'Exit Mint' }
];

/**
 * We wrap everything in an async function to load ESM modules
 */
async function createChatUI(options) {
    // Dynamic imports for ESM modules
    const { render, Box, Text, useInput, useApp, Static } = await import('ink');
    const TextInput = (await import('ink-text-input')).default;
    const { useState, useImperativeHandle, forwardRef, createRef, useEffect, useMemo } = React;

    const App = forwardRef(({ onSubmit, onExit, initialHistory = [] }, ref) => {
        const config = readConfig();
        const { exit } = useApp();
        const [input, setInput] = useState('');
        const [history, setHistory] = useState(initialHistory);
        const [thinking, setThinking] = useState(false);
        const [mode, setMode] = useState('Chat');
        const [model, setModel] = useState('');
        const [workspace, setWorkspace] = useState(process.cwd());

        // Suggestions State
        const [selectedIndex, setSelectedIndex] = useState(0);
        const inputRef = React.useRef(input);
        const selectedIndexRef = React.useRef(selectedIndex);
        
        useEffect(() => {
            inputRef.current = input;
        }, [input]);

        useEffect(() => {
            selectedIndexRef.current = selectedIndex;
        }, [selectedIndex]);

        const showSuggestions = input.startsWith('/') && !input.includes(' ');
        const suggestions = useMemo(() => {
            if (!showSuggestions) return [];
            const query = input.toLowerCase();
            return SLASH_COMMANDS.filter(s => s.cmd.startsWith(query));
        }, [input, showSuggestions]);

        // Reset index when suggestions change
        useEffect(() => {
            setSelectedIndex(0);
        }, [suggestions.length]);

        const lastSystemMessage = React.useRef('');

        // Export methods to the outside world via ref
        useImperativeHandle(ref, () => ({
            appendMessage: (role, text, metadata = {}) => {
                setHistory(prev => [...prev, { role, text, time: new Date(), ...metadata }]);
                if (metadata.providerInfo) {
                    const { provider, model } = metadata.providerInfo;
                    setModel(model ? `${provider} • ${model}` : provider);
                }
            },
            setThinking: (val) => setThinking(val),
            setMode: (val) => setMode(val),
            updateStatusModel: (val) => setModel(val),
            updateWorkspace: (val) => setWorkspace(val),
            appendCodeStep: (info) => {
                let text = '';
                let label = 'System';
                let labelColor = 'blueBright';
                let isThought = false;

                if (typeof info === 'string') {
                    text = info;
                } else {
                    const { action, phase, target, message, thought } = info;
                    if (thought) {
                        text = thought;
                        label = 'Thinking';
                        labelColor = 'gray';
                        isThought = true;
                    } else if (action === 'thinking' || phase === 'thinking') {
                        return;
                    } else {
                        label = action || phase || 'Action';
                        text = target || message || '';
                        if (!text) return;

                        // Color coding for specific actions
                        if (label.includes('search')) labelColor = 'yellowBright';
                        else if (label.includes('file') || label.includes('path')) labelColor = 'cyanBright';
                        else if (label.includes('write') || label.includes('edit') || label.includes('patch')) labelColor = 'greenBright';
                        else if (label.includes('shell') || label.includes('run')) labelColor = 'magentaBright';
                    }
                }

                const fullText = `[${label}] ${text}`;
                if (fullText === lastSystemMessage.current) return;
                lastSystemMessage.current = fullText;

                setHistory(prev => [...prev, { 
                    role: 'system', 
                    label,
                    labelColor,
                    text, 
                    isThought,
                    time: new Date() 
                }]);
            }
        }));

        // Handle exiting and keyboard navigation
        useInput((inputStr, key) => {
            if (key.escape || (key.ctrl && inputStr === 'c')) {
                onExit();
                exit();
            }

            const currentInput = inputRef.current;
            const currentShowSuggestions = currentInput.startsWith('/') && !currentInput.includes(' ');

            if (currentShowSuggestions) {
                const query = currentInput.toLowerCase();
                const currentSuggestions = SLASH_COMMANDS.filter(s => s.cmd.startsWith(query));

                if (currentSuggestions.length > 0) {
                    if (key.upArrow) {
                        setSelectedIndex(prev => (prev > 0 ? prev - 1 : currentSuggestions.length - 1));
                    } else if (key.downArrow) {
                        setSelectedIndex(prev => (prev < currentSuggestions.length - 1 ? prev + 1 : 0));
                    } else if (key.tab || (key.return && currentInput.startsWith('/'))) {
                        const picked = currentSuggestions[selectedIndexRef.current];
                        if (picked) {
                            setInput(picked.cmd + ' ');
                        }
                    }
                }
            }
        });

        const handleSubmit = (value) => {
            const text = value.trim();
            if (!text) return;

            if (showSuggestions && suggestions.length > 0) {
                const picked = suggestions[selectedIndex];
                if (picked && text !== picked.cmd) {
                    setInput(picked.cmd + ' ');
                    return;
                }
            }

            setInput('');
            onSubmit(text);
        };

        return h(Box, { flexDirection: 'column', paddingX: 1, width: '100%' },
            // Static History: Messages
            h(Static, { items: history }, (msg, index) => {
                if (msg.isThought) {
                    return h(Box, { key: index, flexDirection: 'row', marginBottom: 0, paddingLeft: 2 },
                        h(Text, { color: 'gray', dimColor: true }, `Thinking: ${msg.text}`)
                    );
                }

                let name = 'Mint';
                let nameColor = 'greenBright';
                
                if (msg.role === 'user') {
                    name = 'You';
                    nameColor = 'cyanBright';
                } else if (msg.role === 'error') {
                    name = 'Error';
                    nameColor = 'redBright';
                } else if (msg.role === 'system') {
                    name = msg.label || 'System';
                    nameColor = msg.labelColor || 'blueBright';
                }

                return h(Box, { key: index, flexDirection: 'column', marginBottom: 0 },
                    h(Box, null,
                        h(Text, { bold: true, color: nameColor }, name),
                        h(Text, { color: 'gray' }, ` ${msg.time instanceof Date ? msg.time.toLocaleTimeString() : ''}`)
                    ),
                    h(Box, { paddingLeft: 2, marginBottom: 1 },
                        h(Text, null, msg.text)
                    )
                );
            }),

            // Floating (Persistent) UI part
            h(Box, { flexDirection: 'column' },
                thinking && h(Box, { marginBottom: 1 },
                    h(Text, { color: 'yellow' }, '● Mint is thinking...')
                ),

                // Suggestions Menu
                showSuggestions && suggestions.length > 0 && h(Box, { 
                    flexDirection: 'column', 
                    borderStyle: 'single', 
                    borderColor: 'gray',
                    paddingX: 1,
                    marginBottom: 0
                },
                    suggestions.map((s, i) => h(Box, { key: s.cmd, flexDirection: 'row' },
                        h(Text, { 
                            backgroundColor: i === selectedIndex ? 'green' : undefined,
                            color: i === selectedIndex ? 'white' : 'greenBright' 
                        }, s.cmd.padEnd(12)),
                        h(Text, { color: 'gray' }, ` ${s.desc}`)
                    ))
                ),

                // Compact Input Area
                h(Box, { borderStyle: 'round', borderColor: 'greenBright', paddingX: 1, flexDirection: 'row' },
                    h(Text, { bold: true, color: 'greenBright' }, '› '),
                    h(TextInput, { 
                        value: input, 
                        onChange: setInput, 
                        onSubmit: handleSubmit,
                        placeholder: 'Ask anything...'
                    })
                ),

                // Status Bar
                h(Box, { justifyContent: 'space-between' },
                    h(Box, null,
                        h(Text, { color: 'cyan' }, `[${mode}] `),
                        h(Text, { color: 'magentaBright' }, (model || config.geminiModel || 'gemini').slice(0, 46))
                    ),
                    h(Box, null,
                        h(Text, { color: 'gray' }, `path: ...${workspace.slice(-20)}`)
                    )
                )
            )
        );
    });

    // Print banner once before rendering the main app-
    console.log(`\x1b[38;5;121m\x1b[1m __  __ _       _    ___ _    ___ \x1b[0m`);
    console.log(`\x1b[38;5;121m\x1b[1m|  \\/  (_)_ __ | |_ / __| |  |_ _|\x1b[0m`);
    console.log(`\x1b[38;5;121m\x1b[1m| |\\/| | | '_ \\|  _| (__| |__ | | \x1b[0m`);
    console.log(`\x1b[38;5;121m\x1b[1m|_|  |_|_|_| |_|\\__|\\___|____|___|\x1b[0m`);
    console.log(`\x1b[90mType naturally to chat. Esc to exit.\x1b[0m\n`);

    const ref = createRef();
    render(h(App, { ref, ...options }));

    return {
        appendMessage: (role, text, metadata) => ref.current?.appendMessage(role, text, metadata),
        setThinking: (val) => ref.current?.setThinking(val),
        setMode: (val) => ref.current?.setMode(val),
        updateStatusModel: (val) => ref.current?.updateStatusModel(val),
        updateWorkspace: (val) => ref.current?.updateWorkspace(val),
        appendCodeStep: (info) => ref.current?.appendCodeStep(info),
        streamMessage: () => {
            let fullText = '';
            return {
                appendChunk: (chunk) => {
                    fullText += chunk;
                },
                finalize: () => {
                    ref.current?.appendMessage('assistant', fullText);
                }
            };
        },
        copyLastResponse: () => false,
        requestApproval: () => Promise.resolve(true),
        askUser: () => Promise.resolve('')
    };
}

module.exports = { createChatUI };
