'use strict';

const crypto = require('crypto');
const { colors, exitWithGoodbye } = require('./cli_colors');
const { splitResponseSentences }  = require('./cli_formatters');
const {
    isRepoSummaryRequest,
    parseRepoSummaryArgs,
    isSymbolIndexRequest,
    parseSymbolIndexArgs,
    isSemanticCodeSearchRequest,
    parseSemanticCodeArgs,
    extractSemanticCodeQuery
} = require('./intent_detectors');
const { handleSlashCommandUI }    = require('./slash_command_handler');
const { createChatUI }            = require('./chat_ui');
const { loadImageAsDataUri, loadClipboardImageAsDataUri } = require('./image_input');
const { summarizeRepository, formatRepoSummary }  = require('./repo_summarizer');
const { buildSymbolIndex, formatSymbolIndex }      = require('./symbol_indexer');
const {
    indexSemanticCode,
    searchSemanticCode,
    formatSemanticCodeIndex,
    formatSemanticCodeSearch
} = require('./semantic_code_search');
const { handleChat, getChatTranscript } = require('../AI_Brain/Gemini_API');
const agentOrchestrator = require('../AI_Brain/agent_orchestrator');
const systemMonitor     = require('../Plugins/system_monitor');
const workspaceManager  = require('./workspace_manager');
const { executeCodeTask } = require('./code_agent');
const { resetChat }       = require('../AI_Brain/Gemini_API');
const { saveChatImages }  = require('../System/picture_store');

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

function createSessionStats() {
    return {
        sessionId: crypto.randomUUID(),
        startedAt: Date.now(),
        activeStartedAt: null,
        agentActiveMs: 0,
        toolCalls: { total: 0, success: 0, failed: 0 },
        modelUsage: {}
    };
}

function addUsageRow(stats, row = {}) {
    const provider = row.provider || 'unknown';
    const model = row.model || 'unknown';
    const key = `${provider}:${model}`;
    if (!stats.modelUsage[key]) {
        stats.modelUsage[key] = {
            provider,
            model,
            requests: 0,
            inputTokens: 0,
            cacheReads: 0,
            outputTokens: 0,
            reasoningTokens: 0,
            totalTokens: 0
        };
    }

    const target = stats.modelUsage[key];
    target.requests += Number(row.requests) || 1;
    target.inputTokens += Number(row.inputTokens) || 0;
    target.cacheReads += Number(row.cacheReads) || 0;
    target.outputTokens += Number(row.outputTokens) || 0;
    target.reasoningTokens += Number(row.reasoningTokens) || 0;
    target.totalTokens += Number(row.totalTokens) || 0;
}

function normalizeProviderUsage(providerInfo = {}) {
    const usage = providerInfo.usage;
    if (Array.isArray(usage)) return usage;
    if (!usage || typeof usage !== 'object') {
        return [{
            provider: providerInfo.provider,
            model: providerInfo.model,
            requests: 1
        }];
    }

    return [{
        provider: providerInfo.provider,
        model: providerInfo.model,
        requests: 1,
        inputTokens: usage.promptTokenCount || usage.prompt_tokens || usage.input_tokens,
        cacheReads: usage.cachedContentTokenCount ||
            (usage.prompt_tokens_details && usage.prompt_tokens_details.cached_tokens) ||
            usage.cache_read_input_tokens,
        outputTokens: usage.candidatesTokenCount || usage.completion_tokens || usage.output_tokens,
        reasoningTokens: usage.thoughtsTokenCount ||
            (usage.completion_tokens_details && usage.completion_tokens_details.reasoning_tokens),
        totalTokens: usage.totalTokenCount || usage.total_tokens
    }];
}

function recordProviderInfo(stats, providerInfo) {
    if (!providerInfo) return;
    for (const row of normalizeProviderUsage(providerInfo)) {
        addUsageRow(stats, row);
    }
}

function markAgentActive(stats, active) {
    const now = Date.now();
    if (active && !stats.activeStartedAt) {
        stats.activeStartedAt = now;
        return;
    }
    if (!active && stats.activeStartedAt) {
        stats.agentActiveMs += now - stats.activeStartedAt;
        stats.activeStartedAt = null;
    }
}

function buildExitSummary(stats) {
    const activeMs = stats.agentActiveMs + (stats.activeStartedAt ? Date.now() - stats.activeStartedAt : 0);
    const total = stats.toolCalls.total;
    return {
        message: 'Agent powering down. Goodbye!',
        sessionId: stats.sessionId,
        toolCalls: {
            ...stats.toolCalls,
            successRate: total ? (stats.toolCalls.success / total) * 100 : 0
        },
        wallMs: Date.now() - stats.startedAt,
        agentActiveMs: activeMs,
        modelUsage: Object.values(stats.modelUsage),
        quotaHint: 'Use /models to view model quota information'
    };
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Streams response text sentence-by-sentence into the TUI.
 */
async function streamAssistantSentences(text, appendMessage, metadata = {}, streamMessage = null) {
    const sentences = splitResponseSentences(text);
    const chunks    = sentences.filter(s => String(s || '').trim());

    if (typeof streamMessage === 'function') {
        const stream = streamMessage(metadata);
        for (let i = 0; i < chunks.length; i++) {
            stream.appendChunk(chunks[i]);
            if (i < chunks.length - 1) await sleep(90);
        }
        stream.finalize();
        return;
    }

    for (let i = 0; i < chunks.length; i++) {
        appendMessage('assistant', chunks[i], i === 0 ? metadata : {});
        if (i < chunks.length - 1) await sleep(90);
    }
}

/**
 * Runs a timer that increments seconds and calls setThinking every 1s.
 * Returns a cancel function.
 */
function startThinkingTimer(setThinking) {
    let seconds = 0;
    setThinking(true, seconds);
    const timer = setInterval(() => {
        seconds++;
        setThinking(true, seconds);
    }, 1000);
    return () => clearInterval(timer);
}

// ---------------------------------------------------------------------------
// Local tool message senders
// ---------------------------------------------------------------------------

async function sendRepoSummaryMessage({ rawArgs = '', appendMessage, streamMessage, setThinking }) {
    const formatErr = (err) => err && err.message ? err.message : String(err || 'Unknown error');
    try {
        if (typeof setThinking === 'function') setThinking(false);
        const opts        = parseRepoSummaryArgs(rawArgs);
        const summary     = summarizeRepository(opts.targetPath);
        const responseText = opts.json ? JSON.stringify(summary, null, 2) : formatRepoSummary(summary);
        await streamAssistantSentences(responseText, appendMessage, {}, streamMessage);
        return responseText;
    } catch (err) {
        if (typeof setThinking === 'function') setThinking(false);
        appendMessage('error', formatErr(err));
        return '';
    }
}

async function sendSymbolIndexMessage({ rawArgs = '', appendMessage, streamMessage, setThinking }) {
    const formatErr = (err) => err && err.message ? err.message : String(err || 'Unknown error');
    try {
        if (typeof setThinking === 'function') setThinking(false);
        const opts        = parseSymbolIndexArgs(rawArgs);
        const index       = buildSymbolIndex(opts.targetPath);
        const responseText = opts.json
            ? JSON.stringify(index, null, 2)
            : formatSymbolIndex(index, { limit: opts.limit });
        await streamAssistantSentences(responseText, appendMessage, {}, streamMessage);
        return responseText;
    } catch (err) {
        if (typeof setThinking === 'function') setThinking(false);
        appendMessage('error', formatErr(err));
        return '';
    }
}

async function sendSemanticCodeMessage({ rawArgs = '', appendMessage, streamMessage, setThinking, appendCodeStep }) {
    const formatErr = (err) => err && err.message ? err.message : String(err || 'Unknown error');
    const opts = parseSemanticCodeArgs(rawArgs);
    try {
        if (typeof setThinking === 'function') setThinking(true, 0);
        let responseText;

        if (opts.mode === 'index') {
            const index = await indexSemanticCode(opts.targetPath, {
                onProgress: (info) => {
                    if (typeof appendCodeStep === 'function' &&
                        (info.current === 1 || info.current === info.total || info.current % 25 === 0)) {
                        appendCodeStep({ action: 'semantic_code_index', target: `${info.current}/${info.total} ${info.file}` });
                    }
                }
            });
            responseText = opts.json ? JSON.stringify(index, null, 2) : formatSemanticCodeIndex(index);
        } else {
            if (!opts.query) throw new Error('Usage: /semantic-code search <query>');
            const results = await searchSemanticCode(opts.query, opts.targetPath, { topK: opts.topK });
            responseText  = opts.json ? JSON.stringify(results, null, 2) : formatSemanticCodeSearch(results);
        }

        if (typeof setThinking === 'function') setThinking(false);
        await streamAssistantSentences(responseText, appendMessage, {}, streamMessage);
        return responseText;
    } catch (err) {
        if (typeof setThinking === 'function') setThinking(false);
        appendMessage('error', formatErr(err));
        return '';
    }
}

async function sendImageMessage({ images, image, prompt, appendMessage, streamMessage, setThinking, appendCodeStep, stats }) {
    const formatErr = (err) => err && err.message ? err.message : String(err || 'Unknown error');
    const imageList = images || (image ? [image] : []);
    const message   = prompt || 'Analyze this image.';
    const labels    = imageList.map((_, i) => `[Image #${i + 1}]`).join(' ');
    const displayMessage = labels && message.includes(labels) ? message : `${message}\n${labels}`;

    appendMessage('user', displayMessage);
    if (appendCodeStep) {
        appendCodeStep({
            thought: imageList.length > 1
                ? `Analyzing ${imageList.length} attached images before answering.`
                : 'Analyzing the attached image before answering.'
        });
    }

    const cancelTimer = startThinkingTimer(setThinking);
    if (stats) markAgentActive(stats, true);
    try {
        const imageDataUris = imageList.map(item => item.dataUri);
        const result        = await handleChat(message, imageDataUris, null);
        try {
            saveChatImages(imageDataUris, { source: 'cli', message });
        } catch (saveError) {
            console.error('[Pictures] Failed to save CLI image:', saveError.message);
        }
        cancelTimer();
        setThinking(false);
        if (stats) markAgentActive(stats, false);

        const responseText = result.response || '';
        if (stats) recordProviderInfo(stats, result.providerInfo);
        await streamAssistantSentences(responseText, appendMessage, { providerInfo: result.providerInfo }, streamMessage);
        return { responseText, labels, imageList };
    } catch (err) {
        cancelTimer();
        setThinking(false);
        if (stats) markAgentActive(stats, false);
        appendMessage('error', formatErr(err));
        return { responseText: '', labels, imageList };
    }
}

// ---------------------------------------------------------------------------
// Agent task execution (shared by onSubmit + initial message)
// ---------------------------------------------------------------------------

async function runAgentTask(text, { appendMessage, streamMessage, setThinking, requestApproval, askUser, setMode, appendCodeStep }, sharedState) {
    const formatErr   = (err) => err && err.message ? err.message : String(err || 'Unknown error');
    const transcript  = await getChatTranscript();
    const contextualHistory = sharedState.recentImageContextText
        ? [...transcript, { sender: 'system', text: sharedState.recentImageContextText, timestamp: new Date().toISOString() }]
        : transcript;

    if (setMode) setMode('Agent');
    const cancelTimer = startThinkingTimer(setThinking);
    markAgentActive(sharedState.stats, true);

    try {
        const config             = require('../System/config_manager').readConfig();
        const availableProviders = require('../System/config_manager').getAvailableProviders(config);
        const preferredProvider  = require('./code_agent')._helpers.selectSupportedCodeProvider(config, availableProviders);
        let streamedFinalSummary = false;

        const result = await executeCodeTask(text, {
            cwd: process.cwd(),
            requestApproval,
            askUser,
            provider: preferredProvider,
            history:  contextualHistory,
            onProgress: (info) => {
                if (info && info.phase === 'tool_call') {
                    sharedState.stats.toolCalls.total += 1;
                    if (info.status === 'success') sharedState.stats.toolCalls.success += 1;
                    else sharedState.stats.toolCalls.failed += 1;
                }
                if (appendCodeStep) appendCodeStep(info);
            },
            onFinalSummary: async (info) => {
                cancelTimer();
                setThinking(false);
                markAgentActive(sharedState.stats, false);
                recordProviderInfo(sharedState.stats, info.providerInfo);
                streamedFinalSummary = true;
                await streamAssistantSentences(info.summary, appendMessage, { providerInfo: info.providerInfo }, streamMessage);
            }
        });

        cancelTimer();
        setThinking(false);
        markAgentActive(sharedState.stats, false);
        sharedState.lastResponseText = result.summary;
        if (!streamedFinalSummary) {
            recordProviderInfo(sharedState.stats, result.providerInfo);
            await streamAssistantSentences(result.summary, appendMessage, { providerInfo: result.providerInfo }, streamMessage);
        }
    } catch (err) {
        cancelTimer();
        setThinking(false);
        markAgentActive(sharedState.stats, false);
        appendMessage('error', formatErr(err));
    } finally {
        if (setMode) setMode('Agent');
    }
}

// ---------------------------------------------------------------------------
// Public: startInteractiveChat
// ---------------------------------------------------------------------------

/**
 * Starts the interactive TUI chat session.
 *
 * @param {string|null} initialMessage  Optional first message (from CLI arg).
 * @param {{ imagePath?: string }}  options
 */
async function startInteractiveChat(initialMessage = null, options = {}) {
    const formatErr = (err) => err && err.message ? err.message : String(err || 'Unknown error');

    // Shared mutable state between onSubmit closures
    const sharedState = {
        lastResponseText:        '',
        recentImageContextText:  '',
        isBusy:                  false,
        stats:                   createSessionStats()
    };

    // -----------------------------------------------------------------------
    let ui;
    ui = await createChatUI({
        onPasteImage: async () => {
            try {
                const image = loadClipboardImageAsDataUri();
                return { label: image.path, image };
            } catch (err) {
                throw new Error(formatErr(err));
            }
        },

        onSubmit: async (text, submitOptions = {}) => {
            if (sharedState.isBusy) {
                ui.appendMessage('system', 'Mint is still working on the previous request. Please wait for it to finish before sending another command.');
                return;
            }
            sharedState.isBusy = true;

            const {
                appendMessage, streamMessage, setThinking, updateStatusModel,
                copyLastResponse, requestApproval, setMode, appendCodeStep,
                updateWorkspace, askUser, attachImage, setInputText,
                setPendingPasteText, setFastMode, toggleFastMode, getFastMode
            } = ui;

            try {
                // ── Image submission ────────────────────────────────────────
                if (submitOptions.images && submitOptions.images.length > 0) {
                    const images = submitOptions.images.map(item => item.image || item);
                    const { responseText, labels, imageList } = await sendImageMessage({
                        images,
                        prompt: text.trim() || 'Analyze this image.',
                        appendMessage, streamMessage, setThinking, appendCodeStep,
                        stats: sharedState.stats
                    });
                    sharedState.lastResponseText = responseText;
                    if (responseText) {
                        sharedState.recentImageContextText = [
                            `Recent image context: the user attached ${imageList.length} image(s) labelled ${labels || '[Image #1]'}.`,
                            'The terminal UI displays image attachments as labels only; it does not render thumbnails inside the chat.',
                            `Assistant response to those image(s): ${responseText}`
                        ].join('\n');
                    }
                    return;
                }

                // ── Slash commands ──────────────────────────────────────────
                if (text.startsWith('/')) {
                    if (text.startsWith('/agent')) {
                        const aArgs = text.split(' ');
                        if (aArgs[1] === 'list') {
                            appendMessage('system', `Available Agents: ${agentOrchestrator.listAgents().join(', ')}`);
                        } else if (aArgs[1]) {
                            const success = agentOrchestrator.setAgent(aArgs[1]);
                            if (success) {
                                const agent = agentOrchestrator.getCurrentAgent();
                                appendMessage('system', `Switched to Agent: ${agent.icon} ${agent.name}`);
                                updateStatusModel(agent.name);
                                resetChat();
                            } else {
                                appendMessage('error', `Agent "${aArgs[1]}" not found. Try /agent list`);
                            }
                        } else {
                            const agent = agentOrchestrator.getCurrentAgent();
                            appendMessage('system', `Current Agent: ${agent.icon} ${agent.name}\nUsage: /agent <type> or /agent list`);
                        }
                        return;
                    }

                    if (text.startsWith('/stats')) {
                        appendMessage('system', '📊 Fetching system statistics...');
                        const stats = await systemMonitor.execute('stats');
                        appendMessage('system', stats);
                        return;
                    }

                    if (text.startsWith('/workspace')) {
                        const wArgs  = text.split(' ');
                        const subCmd = wArgs[1];
                        if (subCmd === 'add') {
                            const name         = wArgs[2];
                            const wsPath       = wArgs[3] || '.';
                            const instructions = wArgs.slice(4).join(' ');
                            if (!name) {
                                appendMessage('error', 'Usage: /workspace add <name> [path] [instructions]');
                            } else {
                                workspaceManager.addWorkspace(name, wsPath, instructions);
                                appendMessage('system', `Workspace "${name}" registered at ${require('path').resolve(wsPath)}`);
                                resetChat();
                            }
                        } else if (subCmd === 'list') {
                            const all = workspaceManager.listWorkspaces();
                            let listMsg = 'Registered Workspaces:\n';
                            for (const n in all) listMsg += `- ${n}: ${all[n].path}\n`;
                            appendMessage('system', Object.keys(all).length ? listMsg : 'No workspaces registered.');
                        } else if (subCmd === 'remove') {
                            const name = wArgs[2];
                            if (workspaceManager.removeWorkspace(name)) {
                                appendMessage('system', `Removed workspace "${name}"`);
                                resetChat();
                            } else {
                                appendMessage('error', `Workspace "${name}" not found.`);
                            }
                        } else if (subCmd === 'use' || subCmd === 'switch') {
                            const name = wArgs[2];
                            const all  = workspaceManager.listWorkspaces();
                            if (all[name]) {
                                const newPath = all[name].path;
                                try {
                                    process.chdir(newPath);
                                    updateWorkspace(newPath);
                                    appendMessage('system', `✓ Switched to workspace "${name}" at ${newPath}`);
                                    resetChat();
                                } catch (e) {
                                    appendMessage('error', `Failed to change directory: ${e.message}`);
                                }
                            } else {
                                appendMessage('error', `Workspace "${name}" not found. Try /workspace list`);
                            }
                        } else {
                            const ws = workspaceManager.getWorkspaceByPath(process.cwd());
                            appendMessage('system', ws
                                ? `Current Workspace: ${ws.name}\nPath: ${ws.path}`
                                : `Not currently in a registered workspace.\nActive Path: ${process.cwd()}\nUsage: /workspace <add|use|list|remove>`);
                        }
                        return;
                    }

                    if (text.startsWith('/review')) {
                        if (!sharedState.lastResponseText) {
                            appendMessage('error', 'Nothing to review yet. Get a response first.');
                            return;
                        }
                        agentOrchestrator.setAgent('reviewer');
                        appendMessage('system', '⚖️ Requesting second-pass review from Mint Reviewer...');
                        text = `Please review this previous response and provide a critique:\n\n${sharedState.lastResponseText}`;
                    } else {
                        if (!text.startsWith('/image') && !text.startsWith('/paste')) {
                            appendMessage('user', text);
                        }
                        const slashResult = await handleSlashCommandUI(
                            text, appendMessage, updateStatusModel, copyLastResponse,
                            setThinking, requestApproval, setMode, appendCodeStep, updateWorkspace, {
                                sendImageMessage: (args) => sendImageMessage({ ...args, stats: sharedState.stats }),
                                formatErrorMessage: formatErr,
                                attachImage, setInputText, setPendingPasteText,
                                setFastMode, toggleFastMode, getFastMode,
                                sendRepoSummaryMessage, sendSymbolIndexMessage,
                                sendSemanticCodeMessage, streamAssistantSentences,
                                streamMessage
                            }
                        );
                        if (slashResult && slashResult.lastResponseText) {
                            sharedState.lastResponseText = slashResult.lastResponseText;
                        }
                        return;
                    }
                }

                appendMessage('user', text);

                // ── Local tool shortcuts (natural language) ─────────────────
                if (isRepoSummaryRequest(text)) {
                    const r = await sendRepoSummaryMessage({ appendMessage, streamMessage, setThinking });
                    sharedState.lastResponseText = r;
                    return;
                }
                if (isSymbolIndexRequest(text)) {
                    const r = await sendSymbolIndexMessage({ appendMessage, streamMessage, setThinking });
                    sharedState.lastResponseText = r;
                    return;
                }
                if (isSemanticCodeSearchRequest(text)) {
                    const query = extractSemanticCodeQuery(text);
                    const r = await sendSemanticCodeMessage({
                        rawArgs: `search ${query}`,
                        appendMessage, streamMessage, setThinking, appendCodeStep
                    });
                    sharedState.lastResponseText = r;
                    return;
                }

                // ── Default to guarded Code Agent ───────────────────────────
                await runAgentTask(text, {
                    appendMessage, streamMessage, setThinking,
                    requestApproval, askUser, setMode, appendCodeStep
                }, sharedState);
            } finally {
                sharedState.isBusy = false;
            }
        },

        onExit: () => {
            if (ui && typeof ui.unmount === 'function') ui.unmount();
            exitWithGoodbye(0, buildExitSummary(sharedState.stats));
        }
    });

    // ── Handle initial CLI --image option ───────────────────────────────────
    if (options.imagePath) {
        const { appendMessage, streamMessage, setThinking, appendCodeStep } = ui;
        const image  = loadImageAsDataUri(options.imagePath);
        const prompt = initialMessage || 'Analyze this image.';
        const { responseText, labels, imageList } = await sendImageMessage({
            images: [image], prompt, appendMessage, streamMessage, setThinking, appendCodeStep,
            stats: sharedState.stats
        });
        sharedState.lastResponseText = responseText;
        if (responseText) {
            sharedState.recentImageContextText = [
                `Recent image context: the user attached ${imageList.length} image(s) labelled ${labels || '[Image #1]'}.`,
                'The terminal UI displays image attachments as labels only; it does not render thumbnails inside the chat.',
                `Assistant response to those image(s): ${responseText}`
            ].join('\n');
        }
        return;
    }

    // ── Handle initial CLI message argument ─────────────────────────────────
    if (initialMessage) {
        const { appendMessage, streamMessage, setThinking, requestApproval, setMode, appendCodeStep, askUser } = ui;
        appendMessage('user', initialMessage);

        if (isRepoSummaryRequest(initialMessage)) {
            sharedState.lastResponseText = await sendRepoSummaryMessage({ appendMessage, streamMessage, setThinking });
            return;
        }
        if (isSymbolIndexRequest(initialMessage)) {
            sharedState.lastResponseText = await sendSymbolIndexMessage({ appendMessage, streamMessage, setThinking });
            return;
        }
        if (isSemanticCodeSearchRequest(initialMessage)) {
            const query = extractSemanticCodeQuery(initialMessage);
            sharedState.lastResponseText = await sendSemanticCodeMessage({
                rawArgs: `search ${query}`,
                appendMessage, streamMessage, setThinking, appendCodeStep
            });
            return;
        }

        await runAgentTask(initialMessage, {
            appendMessage, streamMessage, setThinking,
            requestApproval, askUser, setMode, appendCodeStep
        }, sharedState);
    }
}

module.exports = { startInteractiveChat };
