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

const MAX_BLANK_LINES = 1;

function compactPathLabel(value) {
    const text = String(value || '').trim();
    if (!text) return '';
    return path.basename(text) || text;
}

function formatActivityStep(info = {}) {
    if (!info || typeof info !== 'object') return null;

    const { action, phase, target, message } = info;
    const rawText = String(target || message || '').trim();
    const kind = action || phase || 'activity';
    if (!rawText) return null;

    switch (kind) {
        case 'list_files':
            return { title: 'Explored', detail: `List ${rawText}` };
        case 'find_path':
            return { title: 'Explored', detail: `Find ${rawText}` };
        case 'read_file':
            return { title: 'Explored', detail: `Read ${compactPathLabel(rawText)}` };
        case 'search_code':
            return { title: 'Explored', detail: `Search ${rawText}` };
        case 'web_search':
            return { title: 'Searched', detail: rawText };
        case 'warn':
            return { title: '⚠ Notice', detail: rawText };
        case 'run_shell':
            return { title: 'Ran', detail: rawText };
        case 'plan':
            return { title: 'Plan', detail: rawText };
        case 'apply_patch':
        case 'write_file':
            return { title: 'Edited', detail: rawText };
        case 'evaluator':
            return { title: 'Checked', detail: rawText };
        case 'reviewer_start':
            return { title: 'Reviewing', detail: rawText };
        case 'ask_user':
            return { title: 'Ask User', detail: rawText };
        default:
            return { title: kind, detail: rawText };
    }
}

function stripInlineMarkdown(value) {
    return String(value || '')
        .replace(/\*\*([^*]+)\*\*/g, '$1')
        .replace(/\*([^*\n]+)\*/g, '$1')
        .replace(/__([^_]+)__/g, '$1')
        .replace(/`([^`\n]+)`/g, '$1');
}

function cleanDisplayText(text, role = 'assistant') {
    const raw = String(text || '').replace(/\r\n/g, '\n').trim();
    if (!raw) return '';

    const shouldPolishMarkdown = role === 'assistant' || role === 'system';
    const lines = raw.split('\n');
    const cleaned = [];
    let inCodeBlock = false;
    let blankCount = 0;

    for (const sourceLine of lines) {
        let line = sourceLine.replace(/\s+$/g, '');
        const fence = line.match(/^\s*```(.*)$/);

        if (fence) {
            inCodeBlock = !inCodeBlock;
            const label = fence[1] ? `code: ${fence[1].trim()}` : 'code';
            line = inCodeBlock ? label : '';
        } else if (inCodeBlock) {
            line = line ? `  ${line}` : '';
        } else if (shouldPolishMarkdown) {
            const heading = line.match(/^\s{0,3}#{1,6}\s+(.+)$/);
            const bullet = line.match(/^(\s*)[-*]\s+(.+)$/);
            const numbered = line.match(/^(\s*)\d+[.)]\s+(.+)$/);

            if (heading) {
                if (cleaned.length > 0 && cleaned[cleaned.length - 1] !== '') cleaned.push('');
                line = stripInlineMarkdown(heading[1]).trim();
            } else if (bullet) {
                line = `${bullet[1]}• ${stripInlineMarkdown(bullet[2]).trim()}`;
            } else if (numbered) {
                line = `${numbered[1]}${stripInlineMarkdown(line).trim()}`;
            } else {
                line = stripInlineMarkdown(line);
            }
        }

        if (!line.trim()) {
            blankCount++;
            if (blankCount <= MAX_BLANK_LINES && cleaned.length > 0) cleaned.push('');
            continue;
        }

        blankCount = 0;
        cleaned.push(line);
    }

    while (cleaned[0] === '') cleaned.shift();
    while (cleaned[cleaned.length - 1] === '') cleaned.pop();
    return cleaned.join('\n');
}

function formatDuration(totalSeconds) {
    const seconds = Math.max(0, Math.floor(Number(totalSeconds) || 0));
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;

    if (minutes <= 0) return `${remainingSeconds}s`;
    return `${minutes}m ${remainingSeconds}s`;
}

function splitDiffStatSegments(value) {
    const text = String(value || '');
    const match = text.match(/\(\+(\d+)\s+-(\d+)\)/);
    if (!match) return [{ text, color: 'cyanBright' }];

    return [
        { text: text.slice(0, match.index), color: 'cyanBright' },
        { text: '(', color: 'gray' },
        { text: `+${match[1]}`, color: 'greenBright' },
        { text: ' ', color: 'gray' },
        { text: `-${match[2]}`, color: 'redBright' },
        { text: ')', color: 'gray' },
        { text: text.slice(match.index + match[0].length), color: 'cyanBright' }
    ].filter(part => part.text);
}

const APPROVAL_CHOICES = ['approve', 'approve_session', 'deny'];
const SUGGESTION_WINDOW_SIZE = 5;

function getNextApprovalChoice(current, direction = 1) {
    const choices = APPROVAL_CHOICES;
    const index = choices.indexOf(current);
    const start = index === -1 ? 0 : index;
    return choices[(start + direction + choices.length) % choices.length];
}

function getVisibleSuggestions(suggestions, selectedIndex, limit = SUGGESTION_WINDOW_SIZE) {
    const items = Array.isArray(suggestions) ? suggestions : [];
    const safeLimit = Math.max(1, Number(limit) || SUGGESTION_WINDOW_SIZE);
    const safeSelected = Math.min(Math.max(0, Number(selectedIndex) || 0), Math.max(0, items.length - 1));
    const start = Math.min(
        Math.max(0, safeSelected - safeLimit + 1),
        Math.max(0, items.length - safeLimit)
    );
    const visible = items.slice(start, start + safeLimit);

    return {
        start,
        visible,
        current: items.length > 0 ? safeSelected + 1 : 0,
        total: items.length
    };
}

function parseUnifiedDiffPreview(preview) {
    const lines = String(preview || '').replace(/\r\n/g, '\n').split('\n');
    const files = [];
    let current = null;

    for (const line of lines) {
        if (line.startsWith('--- a/')) {
            current = {
                path: line.slice('--- a/'.length),
                additions: 0,
                deletions: 0,
                lines: []
            };
            files.push(current);
            continue;
        }

        if (!current) continue;
        if (line.startsWith('+++ b/')) {
            current.path = line.slice('+++ b/'.length) || current.path;
            continue;
        }

        if (line.startsWith('@@')) {
            current.lines.push({ type: 'hunk', text: line });
            continue;
        }

        if (line.startsWith('+')) {
            current.additions += 1;
            current.lines.push({ type: 'add', text: line });
            continue;
        }

        if (line.startsWith('-')) {
            current.deletions += 1;
            current.lines.push({ type: 'delete', text: line });
            continue;
        }

        current.lines.push({ type: 'context', text: line });
    }

    return files.filter(file => file.lines.length > 0 || file.additions > 0 || file.deletions > 0);
}

function isUnifiedDiffPreview(preview) {
    return parseUnifiedDiffPreview(preview).length > 0;
}

function getDiffLineStyle(line = {}) {
    if (line.type === 'add') return { color: 'greenBright' };
    if (line.type === 'delete') return { color: 'redBright' };
    if (line.type === 'hunk') return { color: 'cyanBright' };
    return { color: 'gray', dimColor: true };
}

function shouldAppendMessage(role, text) {
    if (role === 'assistant' || role === 'system') {
        return String(text || '').trim().length > 0;
    }
    return true;
}

function appendInlineImageToken(value, imageIndex) {
    const token = `[Image #${imageIndex}]`;
    const text = String(value || '').replace(/\s*[\r\n]+\s*/g, ' ').trimEnd();
    return text ? `${text} ${token}` : token;
}

function removeImageToken(value, imageIndex) {
    const tokenPattern = new RegExp(`\\s*\\[Image #${imageIndex}\\]`, 'g');
    return String(value || '').replace(tokenPattern, '').replace(/\s{2,}/g, ' ').trim();
}

function removeAllImageTokens(value) {
    return String(value || '').replace(/\s*\[Image #\d+\]/g, '').replace(/\s{2,}/g, ' ').trim();
}

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
        const [workingSeconds, setWorkingSeconds] = useState(0);
        const [fastMode, setFastMode] = useState(false);
        const [mode, setMode] = useState('Agent');
        const [model, setModel] = useState('');
        const [workspace, setWorkspace] = useState(process.cwd());
        const [pendingImages, setPendingImages] = useState([]);
        const [pendingPaste, setPendingPaste] = useState(null);
        const [pendingPastePrefix, setPendingPastePrefix] = useState('');
        const [pendingApproval, setPendingApproval] = useState(null);
        const [approvalChoice, setApprovalChoice] = useState('approve');
        const [approvalSessionAutoApprove, setApprovalSessionAutoApprove] = useState(false);
        const [inputResetKey, setInputResetKey] = useState(0);

        // Suggestions State
        const [selectedIndex, setSelectedIndex] = useState(0);
        const inputRef = React.useRef(input);
        const pendingImagesRef = React.useRef(pendingImages);
        const pendingPasteRef = React.useRef(pendingPaste);
        const pendingPastePrefixRef = React.useRef(pendingPastePrefix);
        const liveAssistantRef = React.useRef(liveAssistant);
        const thinkingStartedAtRef = React.useRef(null);
        const fastModeRef = React.useRef(fastMode);
        const suppressPasteCharRef = React.useRef(false);
        const suppressPasteBurstRef = React.useRef(false);
        const selectedIndexRef = React.useRef(selectedIndex);
        const pendingApprovalRef = React.useRef(null);
        const approvalChoiceRef = React.useRef('approve');
        const approvalSessionAutoApproveRef = React.useRef(false);

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

        const resetInputCursorToEnd = () => {
            setInputResetKey(key => key + 1);
        };
        
        useEffect(() => {
            inputRef.current = input;
        }, [input]);

        useEffect(() => {
            pendingImagesRef.current = pendingImages;
        }, [pendingImages]);

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
            if (!thinking) return undefined;

            const timer = setInterval(() => {
                if (!thinkingStartedAtRef.current) return;
                setWorkingSeconds(Math.floor((Date.now() - thinkingStartedAtRef.current) / 1000));
            }, 1000);

            return () => clearInterval(timer);
        }, [thinking]);

        useEffect(() => {
            fastModeRef.current = fastMode;
        }, [fastMode]);

        useEffect(() => {
            selectedIndexRef.current = selectedIndex;
        }, [selectedIndex]);

        useEffect(() => {
            pendingApprovalRef.current = pendingApproval;
            if (pendingApproval) {
                approvalChoiceRef.current = 'approve';
                setApprovalChoice('approve');
            }
        }, [pendingApproval]);

        useEffect(() => {
            approvalChoiceRef.current = approvalChoice;
        }, [approvalChoice]);

        useEffect(() => {
            approvalSessionAutoApproveRef.current = approvalSessionAutoApprove;
        }, [approvalSessionAutoApprove]);

        const showSuggestions = input.startsWith('/') && !input.includes(' ');
        const suggestions = useMemo(() => {
            if (!showSuggestions) return [];
            const query = input.toLowerCase();
            return SLASH_COMMANDS.filter(s => s.cmd.startsWith(query));
        }, [input, showSuggestions]);
        const visibleSuggestions = useMemo(
            () => getVisibleSuggestions(suggestions, selectedIndex),
            [suggestions, selectedIndex]
        );

        // Reset index when suggestions change
        useEffect(() => {
            setSelectedIndex(0);
        }, [suggestions.length]);

        const lastSystemMessage = React.useRef('');

        // Export methods to the outside world via ref
        useImperativeHandle(ref, () => ({
            appendMessage: (role, text, metadata = {}) => {
                if (!shouldAppendMessage(role, text)) return;
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
                if (!String(chunk || '').trim()) return;
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
            setThinking: (val, seconds = 0) => {
                if (val) {
                    const elapsed = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
                    if (!thinkingStartedAtRef.current) {
                        thinkingStartedAtRef.current = Date.now() - (elapsed * 1000);
                    }
                    setWorkingSeconds(Math.floor((Date.now() - thinkingStartedAtRef.current) / 1000));
                    setThinking(true);
                    return;
                }

                thinkingStartedAtRef.current = null;
                setWorkingSeconds(0);
                setThinking(false);
            },
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
            setInputText: (val) => {
                const next = val || '';
                inputRef.current = next;
                setInput(next);
                resetInputCursorToEnd();
            },
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
                    const imageIndex = prev.length + 1;
                    setInput(current => {
                        const next = appendInlineImageToken(current, imageIndex);
                        inputRef.current = next;
                        resetInputCursorToEnd();
                        return next;
                    });
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
                    if (phase === 'tool_call') {
                        return;
                    }
                    if (thought) {
                        if (process.env.MINT_HIDE_AGENT_NOTES === '1') {
                            return;
                        }
                        text = thought;
                        label = 'Working';
                        labelColor = 'gray';
                        isThought = true;
                    } else if (action === 'thinking' || phase === 'thinking') {
                        return;
                    } else {
                        const activity = formatActivityStep(info);
                        if (activity) {
                            const fullText = `[${activity.title}] ${activity.detail}`;
                            if (fullText === lastSystemMessage.current) return;
                            lastSystemMessage.current = fullText;

                            setHistory(prev => [...prev, {
                                role: 'system',
                                label: activity.title,
                                labelColor: 'blueBright',
                                text: activity.detail,
                                isActivity: true,
                                activityTitle: activity.title,
                                activityDetail: activity.detail,
                                time: new Date()
                            }]);
                            return;
                        }

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
            },
            requestApproval: (request = {}) => {
                if (approvalSessionAutoApproveRef.current) {
                    return Promise.resolve(true);
                }

                return new Promise((resolve) => {
                    const approval = {
                        type: request.type || 'action',
                        label: request.label || 'Requested action',
                        preview: request.preview || '',
                        summary: request.summary || '',
                        openPath: request.openPath || '',
                        warnings: Array.isArray(request.warnings) ? request.warnings.filter(Boolean) : [],
                        resolve
                    };
                    pendingApprovalRef.current = approval;
                    setPendingApproval(approval);
                });
            }
        }));

        // Handle exiting and keyboard navigation
        useInput((inputStr, key) => {
            const approval = pendingApprovalRef.current;
            if (approval) {
                const resolveApproval = (approved, approveForSession = false) => {
                    if (approveForSession) {
                        approvalSessionAutoApproveRef.current = true;
                        setApprovalSessionAutoApprove(true);
                    }
                    pendingApprovalRef.current = null;
                    setPendingApproval(null);
                    setHistory(prev => {
                        if (approved && isUnifiedDiffPreview(approval.preview)) {
                            return [...prev, {
                                role: 'system',
                                label: 'Edited',
                                labelColor: 'greenBright',
                                preview: approval.preview,
                                isDiffPreview: true,
                                time: new Date()
                            }];
                        }

                        return [...prev, {
                            role: 'system',
                            label: 'Approval',
                            labelColor: approved ? 'greenBright' : 'redBright',
                            text: `${approveForSession ? 'Approved this session' : (approved ? 'Approved' : 'Denied')}: ${approval.label}`,
                            time: new Date()
                        }];
                    });
                    approval.resolve(approved);
                };

                if (key.upArrow || key.downArrow || key.leftArrow || key.rightArrow || key.tab) {
                    const next = getNextApprovalChoice(
                        approvalChoiceRef.current,
                        (key.upArrow || key.leftArrow) ? -1 : 1
                    );
                    approvalChoiceRef.current = next;
                    setApprovalChoice(next);
                    return;
                }

                const answer = String(inputStr || '').toLowerCase();
                if (key.return) {
                    resolveApproval(approvalChoiceRef.current !== 'deny', approvalChoiceRef.current === 'approve_session');
                    return;
                }
                if (answer === 'y') {
                    resolveApproval(true);
                    return;
                }
                if (answer === 'a') {
                    resolveApproval(true, true);
                    return;
                }
                if (answer === 'n' || key.escape || (key.ctrl && inputStr === 'c')) {
                    resolveApproval(false);
                    return;
                }
                return;
            }

            if (key.escape && pendingImagesRef.current.length > 0) {
                setPendingImages([]);
                pendingImagesRef.current = [];
                setInput(current => {
                    const next = removeAllImageTokens(current);
                    inputRef.current = next;
                    resetInputCursorToEnd();
                    return next;
                });
                return;
            }

            if (key.escape && pendingPasteRef.current) {
                setPendingPaste(null);
                pendingPasteRef.current = null;
                setPendingPastePrefix('');
                pendingPastePrefixRef.current = '';
                suppressPasteBurstRef.current = false;
                inputRef.current = '';
                setInput('');
                resetInputCursorToEnd();
                return;
            }

            if (key.ctrl && key.backspace && pendingImagesRef.current.length > 0) {
                const imageIndex = pendingImagesRef.current.length;
                const nextImages = pendingImagesRef.current.slice(0, -1);
                pendingImagesRef.current = nextImages;
                setPendingImages(nextImages);
                setInput(current => {
                    const next = removeImageToken(current, imageIndex);
                    inputRef.current = next;
                    resetInputCursorToEnd();
                    return next;
                });
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
                                    const imageIndex = prev.length + 1;
                                    setInput(current => {
                                        const cleaned = removePasteArtifact(current);
                                        const next = appendInlineImageToken(cleaned || inputBeforePaste, imageIndex);
                                        inputRef.current = next;
                                        resetInputCursorToEnd();
                                        return next;
                                    });
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
            const imageLabels = images.map((_, index) => `[Image #${index + 1}]`).join(' ');
            const pasted = pendingPasteRef.current;
            const pastePrefix = normalizeInputText(pendingPastePrefixRef.current).trim();
            const submittedText = pasted
                ? [pastePrefix, pasted.text, text].filter(Boolean).join('\n\n')
                : images.length > 0
                    ? (text || imageLabels)
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
            setPendingPaste(null);
            setPendingPastePrefix('');
            pendingImagesRef.current = [];
            pendingPasteRef.current = null;
            pendingPastePrefixRef.current = '';
            suppressPasteBurstRef.current = false;
            onSubmit(submittedText, { images, pasted });
        };

        const handleInputChange = (value) => {
            if (suppressPasteBurstRef.current && pendingPasteRef.current) {
                inputRef.current = '';
                setInput('');
                resetInputCursorToEnd();
                return;
            }

            if (shouldStoreAsPastedContent(value)) {
                const normalized = normalizeInputText(value);
                const previous = normalizeInputText(inputRef.current).trim();
                const pasted = { text: normalized, label: `[Pasted Content ${normalized.length} chars]` };
                pendingPasteRef.current = pasted;
                pendingPastePrefixRef.current = previous;
                suppressPasteBurstRef.current = true;
                setPendingPaste(pasted);
                setPendingPastePrefix(previous);
                inputRef.current = '';
                setInput('');
                resetInputCursorToEnd();
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
            inputRef.current = normalizedValue;
            setInput(normalizedValue);
        };

        const renderActivityDetail = (value) => {
            const segments = splitDiffStatSegments(value);
            return segments.map((segment, segmentIndex) =>
                h(Text, {
                    key: `activity-detail-${segmentIndex}`,
                    color: segment.color,
                    wrap: 'wrap'
                }, segment.text)
            );
        };

        const renderDiffLine = (line, index) => {
            const style = getDiffLineStyle(line);
            return h(Text, {
                key: `diff-line-${index}`,
                ...style
            }, line.text || ' ');
        };

        const renderDiffPreview = (preview) => {
            const files = parseUnifiedDiffPreview(preview);
            if (files.length === 0) return null;

            return h(Box, { flexDirection: 'column', marginTop: 1 },
                ...files.map((file, fileIndex) =>
                    h(Box, { key: `approval-diff-${fileIndex}`, flexDirection: 'column', marginBottom: 1 },
                        h(Box, null,
                            h(Text, { color: 'gray' }, '• '),
                            h(Text, { bold: true, color: 'white' }, `Edited ${file.path} `),
                            h(Text, { color: 'gray' }, '('),
                            h(Text, { color: 'greenBright' }, `+${file.additions}`),
                            h(Text, { color: 'gray' }, ' '),
                            h(Text, { color: 'redBright' }, `-${file.deletions}`),
                            h(Text, { color: 'gray' }, ')')
                        ),
                        h(Box, { flexDirection: 'column', paddingLeft: 2 },
                            ...file.lines.slice(0, 120).map(renderDiffLine),
                            file.lines.length > 120 && h(Text, { color: 'gray', dimColor: true }, `... ${file.lines.length - 120} more diff lines`)
                        )
                    )
                )
            );
        };

        const renderApprovalPreview = (approval) => {
            const preview = approval && approval.preview ? approval.preview : '';
            if (approval && approval.type === 'plan') {
                return h(Box, { flexDirection: 'column', marginTop: 1 },
                    h(Text, { color: 'gray' }, approval.summary || 'Mint prepared a plan for this task.'),
                    approval.openPath && h(Text, { color: 'gray', dimColor: true }, `Details: ${approval.label}`)
                );
            }

            const diffPreview = renderDiffPreview(preview);
            if (!diffPreview) {
                return preview && preview !== approval.label
                    ? h(Box, null, h(Text, { color: 'gray', dimColor: true }, preview))
                    : null;
            }

            return diffPreview;
        };

        const renderMessage = (msg, index, keyPrefix = 'msg') => {
            if (msg.isThought) {
                return h(Box, { key: `${keyPrefix}-${index}`, flexDirection: 'row', marginBottom: 0, paddingLeft: 2 },
                    h(Text, { color: 'gray', dimColor: true }, `Thinking: ${msg.text}`)
                );
            }

            if (msg.isActivity) {
                return h(Box, { key: `${keyPrefix}-${index}`, flexDirection: 'column', marginBottom: 0 },
                    h(Box, null,
                        h(Text, { color: 'greenBright' }, '• '),
                        h(Text, { bold: true, color: msg.labelColor || 'blueBright' }, msg.activityTitle || msg.label || 'Activity')
                    ),
                    h(Box, { paddingLeft: 2, marginBottom: 1 },
                        h(Text, { color: 'gray' }, '└ '),
                        ...renderActivityDetail(msg.activityDetail || msg.text)
                    )
                );
            }

            if (msg.isDiffPreview) {
                return h(Box, { key: `${keyPrefix}-${index}`, flexDirection: 'column', marginBottom: 0 },
                    renderDiffPreview(msg.preview || '')
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
                    h(Text, { wrap: 'wrap' }, cleanDisplayText(msg.text, msg.role))
                )
            );
        };

        return h(Box, { flexDirection: 'column', paddingX: 1, width: '100%' },
            // Static History: Messages
            h(Static, { items: history }, (msg, index) => renderMessage(msg, index, 'history')),
            liveAssistant && renderMessage(liveAssistant, 'live', 'live'),

            // Floating (Persistent) UI part
            h(Box, { flexDirection: 'column' },
                thinking && h(Box, { flexDirection: 'column', marginBottom: 1 },
                    h(Text, { color: 'gray', dimColor: true }, `─ Working for ${formatDuration(workingSeconds)} ─────────────────────────────────────────────────────────`),
                    h(Text, { color: 'yellow' }, '● Mint is thinking...')
                ),

                pendingApproval && h(Box, {
                    flexDirection: 'column',
                    borderStyle: 'single',
                    borderColor: 'cyanBright',
                    paddingX: 1,
                    marginBottom: 0
                },
                    h(Box, null,
                        pendingApproval.type === 'plan'
                            ? h(Text, { bold: true, color: 'greenBright' }, 'Plan')
                            : [
                                h(Text, { key: 'approval-title', bold: true, color: 'greenBright' }, 'Approval '),
                                h(Text, { key: 'approval-type', color: 'cyanBright' }, `[${pendingApproval.type}] `),
                                h(Text, { key: 'approval-label', color: 'white' }, pendingApproval.label)
                            ]
                    ),
                    pendingApproval.warnings && pendingApproval.warnings.length > 0 && h(Box, { flexDirection: 'column', marginTop: 1, marginBottom: 1 },
                        ...pendingApproval.warnings.map((warning, index) =>
                            h(Box, { key: `approval-warning-${index}` },
                                h(Text, { color: 'yellowBright' }, 'Warning: '),
                                h(Text, { color: 'yellowBright' }, warning)
                            )
                        )
                    ),
                    renderApprovalPreview(pendingApproval)
                ),

                pendingApproval && h(Box, {
                    flexDirection: 'column',
                    borderStyle: 'single',
                    borderColor: approvalChoice === 'deny' ? 'redBright' : 'greenBright',
                    paddingX: 1,
                    marginBottom: 0
                },
                    h(Box, null,
                        h(Text, {
                            color: approvalChoice === 'approve' ? 'black' : 'greenBright',
                            backgroundColor: approvalChoice === 'approve' ? 'greenBright' : undefined,
                            bold: true
                        }, approvalChoice === 'approve' ? '▸ Approve' : '  Approve')
                    ),
                    h(Box, null,
                        h(Text, {
                            color: approvalChoice === 'approve_session' ? 'black' : 'cyanBright',
                            backgroundColor: approvalChoice === 'approve_session' ? 'cyanBright' : undefined,
                            bold: true
                        }, approvalChoice === 'approve_session' ? '▸ Approve Session' : '  Approve Session')
                    ),
                    h(Box, null,
                        h(Text, {
                            color: approvalChoice === 'deny' ? 'white' : 'redBright',
                            backgroundColor: approvalChoice === 'deny' ? 'redBright' : undefined,
                            bold: true
                        }, approvalChoice === 'deny' ? '▸ Deny' : '  Deny')
                    ),
                    h(Box, null,
                        h(Text, { color: 'gray', dimColor: true }, '  ↑/↓ Enter  y/a/n')
                    )
                ),

                // Compact Input Area
                h(Box, { borderStyle: 'round', borderColor: pendingApproval ? 'gray' : 'greenBright', paddingX: 1, flexDirection: 'column' },
                    pendingImages.length > 0 && h(Box, null,
                        h(Text, { color: 'greenBright' }, `${pendingImages.length} image${pendingImages.length === 1 ? '' : 's'} attached `),
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
                            key: `input-${inputResetKey}`,
                            value: input,
                            onChange: pendingApproval ? () => {} : handleInputChange,
                            onSubmit: pendingApproval ? () => {} : handleSubmit,
                            placeholder: pendingApproval ? 'Approval pending...' : 'Ask anything...'
                        })
                    )
                ),

                // Suggestions Menu
                showSuggestions && suggestions.length > 0 && h(Box, {
                    flexDirection: 'column',
                    borderStyle: 'single',
                    borderColor: 'gray',
                    paddingX: 1,
                    marginBottom: 0
                },
                    h(Box, { justifyContent: 'space-between' },
                        h(Text, { color: 'gray', dimColor: true }, 'Commands'),
                        h(Text, { color: 'gray', dimColor: true }, `${visibleSuggestions.current}/${visibleSuggestions.total}`)
                    ),
                    visibleSuggestions.visible.map((s, i) => {
                        const actualIndex = visibleSuggestions.start + i;
                        return h(Box, { key: s.cmd, flexDirection: 'row' },
                            h(Text, {
                                backgroundColor: actualIndex === selectedIndex ? 'green' : undefined,
                                color: actualIndex === selectedIndex ? 'white' : 'greenBright'
                            }, s.cmd.padEnd(12)),
                            h(Text, { color: 'gray' }, ` ${s.desc}`)
                        );
                    })
                ),

                // Status Bar
                h(Box, { justifyContent: 'space-between' },
                    h(Box, null,
                        h(Text, { color: 'cyan' }, `[${fastMode ? 'Fast' : mode}] `),
                        h(Text, { color: 'magentaBright' }, (model || config.geminiModel || 'gemini').slice(0, 46)),
                        approvalSessionAutoApprove && h(Text, { color: 'greenBright' }, ' approvals:session')
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
    const instance = render(h(App, { ref, ...options }), { exitOnCtrlC: false });

    return {
        unmount: () => instance.unmount(),
        appendMessage: (role, text, metadata) => ref.current?.appendMessage(role, text, metadata),
        setThinking: (val, seconds) => ref.current?.setThinking(val, seconds),
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
            ref.current?.beginAssistantStream(metadata);
            return {
                appendChunk: (chunk) => {
                    if (!String(chunk || '').trim()) return;
                    ref.current?.appendAssistantStreamChunk(chunk);
                },
                finalize: () => {
                    ref.current?.finalizeAssistantStream();
                }
            };
        },
        copyLastResponse: () => false,
        requestApproval: (request) => ref.current?.requestApproval(request) || Promise.resolve(false),
        askUser: () => Promise.resolve('')
    };
}

module.exports = {
    createChatUI,
    _helpers: {
        cleanDisplayText,
        stripInlineMarkdown,
        compactPathLabel,
        formatActivityStep,
        formatDuration,
        splitDiffStatSegments,
        getNextApprovalChoice,
        getVisibleSuggestions,
        parseUnifiedDiffPreview,
        isUnifiedDiffPreview,
        getDiffLineStyle,
        shouldAppendMessage,
        appendInlineImageToken,
        removeImageToken,
        removeAllImageTokens
    }
};
