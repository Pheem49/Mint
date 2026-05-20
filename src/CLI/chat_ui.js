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
    { cmd: '/image', desc: 'Attach an image from a file path' },
    { cmd: '/paste', desc: 'Attach an image from the clipboard' },
    { cmd: '/fast', desc: 'Toggle fast mode (hide thinking)' },
    { cmd: '/learn', desc: 'Remember a markdown skill file' },
    { cmd: '/code', desc: 'Force workspace Code Mode' },
    { cmd: '/cd', desc: 'Change current working directory' },
    { cmd: '/models', desc: 'List or switch Gemini models' },
    { cmd: '/memory', desc: 'List, search, clear, or export long-term memory' },
    { cmd: '/memory skills', desc: 'Show learned skill files' },
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

    const App = forwardRef(({ onSubmit, onExit, onPasteImage, initialHistory = [] }, ref) => {
        const config = readConfig();
        const { exit } = useApp();
        const [input, setInput] = useState('');
        const [history, setHistory] = useState(initialHistory);
        const [liveAssistant, setLiveAssistant] = useState(null);
        const [thinking, setThinking] = useState(false);
        const [fastMode, setFastMode] = useState(false);
        const [mode, setMode] = useState('Agent');
        const [model, setModel] = useState('');
        const [workspace, setWorkspace] = useState(process.cwd());
        const [pendingImages, setPendingImages] = useState([]);
        const [pendingImagePrefix, setPendingImagePrefix] = useState('');
        const [pendingPaste, setPendingPaste] = useState(null);
        const [pendingPastePrefix, setPendingPastePrefix] = useState('');

        // Suggestions State
        const [selectedIndex, setSelectedIndex] = useState(0);
        const inputRef = React.useRef(input);
        const pendingImagesRef = React.useRef(pendingImages);
        const pendingImagePrefixRef = React.useRef(pendingImagePrefix);
        const pendingPasteRef = React.useRef(pendingPaste);
        const pendingPastePrefixRef = React.useRef(pendingPastePrefix);
        const liveAssistantRef = React.useRef(liveAssistant);
        const fastModeRef = React.useRef(fastMode);
        const suppressPasteCharRef = React.useRef(false);
        const selectedIndexRef = React.useRef(selectedIndex);

        const removePasteArtifact = (value) => {
            const text = String(value || '');
            return text.replace(/[vV]$/, '');
        };

        const normalizeInputText = (value) => {
            return String(value || '').replace(/\s*[\r\n]+\s*/g, ' ');
        };

        const shouldStoreAsPastedContent = (value) => {
            const text = String(value || '');
            return text.length > 500 || /[\r\n]/.test(text);
        };
        
        useEffect(() => {
            inputRef.current = input;
        }, [input]);

        useEffect(() => {
            pendingImagesRef.current = pendingImages;
        }, [pendingImages]);

        useEffect(() => {
            pendingImagePrefixRef.current = pendingImagePrefix;
        }, [pendingImagePrefix]);

        useEffect(() => {
            pendingPasteRef.current = pendingPaste;
        }, [pendingPaste]);

        useEffect(() => {
            pendingPastePrefixRef.current = pendingPastePrefix;
        }, [pendingPastePrefix]);

        useEffect(() => {
            liveAssistantRef.current = liveAssistant;
        }, [liveAssistant]);

        useEffect(() => {
            fastModeRef.current = fastMode;
        }, [fastMode]);

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
            beginAssistantStream: (metadata = {}) => {
                const msg = { role: 'assistant', text: '', time: new Date(), ...metadata };
                liveAssistantRef.current = msg;
                setLiveAssistant(msg);
                if (metadata.providerInfo) {
                    const { provider, model } = metadata.providerInfo;
                    setModel(model ? `${provider} • ${model}` : provider);
                }
            },
            appendAssistantStreamChunk: (chunk) => {
                const current = liveAssistantRef.current || { role: 'assistant', text: '', time: new Date() };
                const next = { ...current, text: `${current.text || ''}${chunk}` };
                liveAssistantRef.current = next;
                setLiveAssistant(next);
            },
            finalizeAssistantStream: () => {
                const current = liveAssistantRef.current;
                liveAssistantRef.current = null;
                setLiveAssistant(null);
                if (current && String(current.text || '').trim()) {
                    setHistory(prev => [...prev, current]);
                }
            },
            setThinking: (val) => setThinking(val),
            setMode: (val) => setMode(val),
            setFastMode: (val) => {
                const next = Boolean(val);
                fastModeRef.current = next;
                setFastMode(next);
                return next;
            },
            toggleFastMode: () => {
                const next = !fastModeRef.current;
                fastModeRef.current = next;
                setFastMode(next);
                return next;
            },
            getFastMode: () => fastModeRef.current,
            setInputText: (val) => setInput(val || ''),
            setPendingPasteText: (text) => {
                const normalized = normalizeInputText(text);
                setPendingPaste({ text: normalized, label: `[Pasted Content ${normalized.length} chars]` });
                setPendingPastePrefix('');
                setInput('');
            },
            updateStatusModel: (val) => setModel(val),
            updateWorkspace: (val) => setWorkspace(val),
            attachImage: (image) => {
                setPendingImages(prev => {
                    if (prev.length === 0) {
                        const prefix = normalizeInputText(inputRef.current).trim();
                        setPendingImagePrefix(prefix);
                        pendingImagePrefixRef.current = prefix;
                        setInput('');
                    }
                    return [...prev, image];
                });
            },
            appendCodeStep: (info) => {
                if (fastModeRef.current) {
                    return;
                }

                let text = '';
                let label = 'System';
                let labelColor = 'blueBright';
                let isThought = false;

                if (typeof info === 'string') {
                    text = info;
                } else {
                    const { action, phase, target, message, thought } = info;
                    if (action === 'memory_context' && process.env.MINT_SHOW_MEMORY_TRACE !== '1') {
                        return;
                    }
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
            if (key.escape && pendingImagesRef.current.length > 0) {
                setPendingImages([]);
                pendingImagesRef.current = [];
                setPendingImagePrefix('');
                pendingImagePrefixRef.current = '';
                return;
            }

            if (key.escape && pendingPasteRef.current) {
                setPendingPaste(null);
                pendingPasteRef.current = null;
                setPendingPastePrefix('');
                pendingPastePrefixRef.current = '';
                return;
            }

            if (key.ctrl && key.backspace && pendingImagesRef.current.length > 0) {
                setPendingImages(prev => prev.slice(0, -1));
                pendingImagesRef.current = pendingImagesRef.current.slice(0, -1);
                if (pendingImagesRef.current.length === 0) {
                    setPendingImagePrefix('');
                    pendingImagePrefixRef.current = '';
                }
                return;
            }

            if (key.escape || (key.ctrl && inputStr === 'c')) {
                exit();
                onExit();
                return;
            }

            const currentInput = inputRef.current;
            if (key.ctrl && inputStr === 'v') {
                suppressPasteCharRef.current = true;
                const inputBeforePaste = currentInput;
                setInput(prev => removePasteArtifact(prev));
                if (typeof onPasteImage === 'function') {
                    Promise.resolve(onPasteImage())
                        .then((image) => {
                            if (image) {
                                setPendingImages(prev => {
                                    if (prev.length === 0) {
                                        const prefix = normalizeInputText(inputBeforePaste).trim();
                                        setPendingImagePrefix(prefix);
                                        pendingImagePrefixRef.current = prefix;
                                    }
                                    return [...prev, image];
                                });
                            }
                        })
                        .catch((err) => {
                            setHistory(prev => [...prev, {
                                role: 'error',
                                text: err && err.message ? err.message : String(err || 'Unknown error'),
                                time: new Date()
                            }]);
                        })
                        .finally(() => {
                            setInput(prev => {
                                if (prev === `${inputBeforePaste}v` || prev === `${inputBeforePaste}V`) {
                                    return inputBeforePaste;
                                }
                                return removePasteArtifact(prev);
                            });
                        });
                }
                return;
            }

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
            const text = normalizeInputText(value).trim();
            const images = pendingImagesRef.current;
            const imagePrefix = normalizeInputText(pendingImagePrefixRef.current).trim();
            const imageLabels = images.map((_, index) => `[Image #${index + 1}]`).join(' ');
            const pasted = pendingPasteRef.current;
            const pastePrefix = normalizeInputText(pendingPastePrefixRef.current).trim();
            const submittedText = pasted
                ? [pastePrefix, pasted.text, text].filter(Boolean).join('\n\n')
                : images.length > 0
                    ? [imagePrefix, imageLabels, text].filter(Boolean).join('\n\n')
                    : text;
            if (!submittedText && images.length === 0) return;

            if (!pasted && images.length === 0 && showSuggestions && suggestions.length > 0) {
                const picked = suggestions[selectedIndex];
                if (picked && text !== picked.cmd) {
                    setInput(picked.cmd + ' ');
                    return;
                }
            }

            setInput('');
            setPendingImages([]);
            setPendingImagePrefix('');
            setPendingPaste(null);
            setPendingPastePrefix('');
            pendingImagesRef.current = [];
            pendingImagePrefixRef.current = '';
            pendingPasteRef.current = null;
            pendingPastePrefixRef.current = '';
            onSubmit(submittedText, { images, pasted });
        };

        const handleInputChange = (value) => {
            if (shouldStoreAsPastedContent(value)) {
                const normalized = normalizeInputText(value);
                const previous = normalizeInputText(inputRef.current).trim();
                setPendingPaste({ text: normalized, label: `[Pasted Content ${normalized.length} chars]` });
                setPendingPastePrefix(previous);
                setInput('');
                return;
            }

            const normalizedValue = normalizeInputText(value);
            if (suppressPasteCharRef.current) {
                suppressPasteCharRef.current = false;
                const previous = inputRef.current;
                if (normalizedValue === `${previous}v` || normalizedValue === `${previous}V`) {
                    setInput(previous);
                    return;
                }
                if (normalizedValue.length > previous.length && /^[vV]$/.test(normalizedValue.slice(previous.length))) {
                    setInput(previous);
                    return;
                }
            }
            setInput(normalizedValue);
        };

        const renderMessage = (msg, index, keyPrefix = 'msg') => {
            if (msg.isThought) {
                return h(Box, { key: `${keyPrefix}-${index}`, flexDirection: 'row', marginBottom: 0, paddingLeft: 2 },
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

            return h(Box, { key: `${keyPrefix}-${index}`, flexDirection: 'column', marginBottom: 0 },
                h(Box, null,
                    h(Text, { bold: true, color: nameColor }, name),
                    h(Text, { color: 'gray' }, ` ${msg.time instanceof Date ? msg.time.toLocaleTimeString() : ''}`)
                ),
                h(Box, { paddingLeft: 2, marginBottom: 1 },
                    h(Text, null, msg.text)
                )
            );
        };

        return h(Box, { flexDirection: 'column', paddingX: 1, width: '100%' },
            // Static History: Messages
            h(Static, { items: history }, (msg, index) => renderMessage(msg, index, 'history')),
            liveAssistant && renderMessage(liveAssistant, 'live', 'live'),

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
                h(Box, { borderStyle: 'round', borderColor: 'greenBright', paddingX: 1, flexDirection: 'column' },
                    pendingImages.length > 0 && h(Box, null,
                        pendingImagePrefix && h(Text, { color: 'cyanBright' }, '[Text before] '),
                        h(Text, { color: 'greenBright' }, pendingImages.map((_, index) => `[Image #${index + 1}]`).join(' ') + ' '),
                        h(Text, { color: 'gray' }, 'Enter to send, Ctrl+Backspace remove, Esc clear')
                    ),
                    pendingPaste && h(Box, null,
                        pendingPastePrefix && h(Text, { color: 'cyanBright' }, '[Text before] '),
                        h(Text, { color: 'yellowBright' }, pendingPaste.label),
                        h(Text, { color: 'gray' }, ' Enter to send, Esc clear')
                    ),
                    h(Box, { flexDirection: 'row' },
                        h(Text, { bold: true, color: 'greenBright' }, '› '),
                        h(TextInput, {
                            value: input,
                            onChange: handleInputChange,
                            onSubmit: handleSubmit,
                            placeholder: 'Ask anything...'
                        })
                    )
                ),

                // Status Bar
                h(Box, { justifyContent: 'space-between' },
                    h(Box, null,
                        h(Text, { color: 'cyan' }, `[${fastMode ? 'Fast' : mode}] `),
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
    render(h(App, { ref, ...options }), { exitOnCtrlC: false });

    return {
        appendMessage: (role, text, metadata) => ref.current?.appendMessage(role, text, metadata),
        setThinking: (val) => ref.current?.setThinking(val),
        setMode: (val) => ref.current?.setMode(val),
        setFastMode: (val) => ref.current?.setFastMode(val),
        toggleFastMode: () => ref.current?.toggleFastMode(),
        getFastMode: () => ref.current?.getFastMode(),
        setInputText: (val) => ref.current?.setInputText(val),
        setPendingPasteText: (text) => ref.current?.setPendingPasteText(text),
        updateStatusModel: (val) => ref.current?.updateStatusModel(val),
        updateWorkspace: (val) => ref.current?.updateWorkspace(val),
        attachImage: (image) => ref.current?.attachImage(image),
        appendCodeStep: (info) => ref.current?.appendCodeStep(info),
        streamMessage: (metadata = {}) => {
            let fullText = '';
            ref.current?.beginAssistantStream(metadata);
            return {
                appendChunk: (chunk) => {
                    fullText += chunk;
                    ref.current?.appendAssistantStreamChunk(chunk);
                },
                finalize: () => {
                    ref.current?.finalizeAssistantStream();
                }
            };
        },
        copyLastResponse: () => false,
        requestApproval: () => Promise.resolve(true),
        askUser: () => Promise.resolve('')
    };
}

module.exports = { createChatUI };
