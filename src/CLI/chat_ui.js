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

/**
 * We wrap everything in an async function to load ESM modules
 */
async function createChatUI(options) {
    // Dynamic imports for ESM modules
    const { render, Box, Text, useInput, useApp, Static } = await import('ink');
    const TextInput = (await import('ink-text-input')).default;
    const { useState, useImperativeHandle, forwardRef, createRef, useEffect } = React;

    const App = forwardRef(({ onSubmit, onExit, initialHistory = [] }, ref) => {
        const config = readConfig();
        const { exit } = useApp();
        const [input, setInput] = useState('');
        const [history, setHistory] = useState(initialHistory);
        const [thinking, setThinking] = useState(false);
        const [mode, setMode] = useState('Chat');
        const [model, setModel] = useState('');
        const [workspace, setWorkspace] = useState(process.cwd());

        const lastSystemMessage = React.useRef('');

        // Export methods to the outside world via ref
        useImperativeHandle(ref, () => ({
            appendMessage: (role, text) => {
                setHistory(prev => [...prev, { role, text, time: new Date() }]);
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

        // Handle exiting
        useInput((input, key) => {
            if (key.escape || (key.ctrl && input === 'c')) {
                onExit();
                exit();
            }
        });

        const handleSubmit = (value) => {
            const text = value.trim();
            if (!text) return;
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
                        h(Text, { color: 'magentaBright' }, (model || config.geminiModel || 'gemini').slice(0, 30))
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
        appendMessage: (role, text) => ref.current?.appendMessage(role, text),
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
