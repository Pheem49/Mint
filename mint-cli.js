#!/usr/bin/env node
require('dotenv').config({ quiet: true });
const { Command } = require('commander');
const { handleChat, handleGeminiChatStream, resetChat, refreshApiKeyFromConfig } = require('./src/AI_Brain/Gemini_API');
const agentOrchestrator = require('./src/AI_Brain/agent_orchestrator');
const workspaceManager = require('./src/CLI/workspace_manager');
const systemMonitor = require('./src/Plugins/system_monitor');
const { sendNotification } = require('./src/System/notifications');
const pkg = require('./package.json');
const { runOnboarding } = require('./src/CLI/onboarding');
const { startAgent } = require('./src/AI_Brain/headless_agent');
const { displayFeatures } = require('./src/CLI/list_features');
const { readConfig, writeConfig } = require('./src/System/config_manager');
const { executeCodeTask } = require('./src/CLI/code_agent');
const { detectCodeIntent, runChatRoutedTask } = require('./src/CLI/chat_router');
const readline = require('readline');
const { createChatUI } = require('./src/CLI/chat_ui');

// Startup Info
const startupConfig = readConfig();
const startupModel = startupConfig.geminiModel || 'gemini-2.5-flash';
const startupNow = new Date();
const startupTime = startupNow.toLocaleString('th-TH', { 
    day: '2-digit', month: '2-digit', year: 'numeric', 
    hour: '2-digit', minute: '2-digit', hour12: false 
}).replace(',', '');
console.log(`\x1b[38;5;121m[Mint] v${pkg.version} | ${startupTime} | Active Model: ${startupModel}\x1b[0m`);

// ANSI Colors
const colors = {
    reset: "\x1b[0m",
    bright: "\x1b[1m",
    mint: "\x1b[38;5;121m",
    pink: "\x1b[38;5;213m",
    gray: "\x1b[90m",
    cyan: "\x1b[36m",
    yellow: "\x1b[33m"
};

const program = new Command();

program
    .name('mint-ai')
    .description('Mint - Your Personal AI Assistant CLI')
    .version(pkg.version);

// Chat Command (Interactive Mode)
program
    .command('chat', { isDefault: true })
    .description('Start interactive chat session with Mint')
    .argument('[message]', 'Initial message to send to Mint')
    .action(async (message) => {
        await startInteractiveChat(message);
    });

// Onboard Command
program
    .command('onboard')
    .description('Setup Mint for the first time')
    .option('--install-daemon', 'Automatically install systemd background agent')
    .action(async (options) => {
        await runOnboarding(options);
    });

// Agent Command (Headless Daemon Mode)
program
    .command('agent')
    .description('Run Mint as a background agent (headless)')
    .argument('[initialTask]', 'Optional first task to perform immediately on startup')
    .action(async (initialTask) => {
        if (initialTask) {
            const taskManager = require('./src/System/task_manager');
            taskManager.addTask(initialTask);
            console.log(`\n${colors.mint}${colors.bright}[Mint-Agent] Starting with initial task:${colors.reset} "${initialTask}"`);
        }
        await startAgent();
    });

// List Command
program
    .command('list')
    .description('Show list of Mint features and commands')
    .action(() => {
        displayFeatures();
    });

// Task Command (Autonomous Background Task)
program
    .command('task')
    .description('Delegate a complex task to the background agent')
    .argument('<description>', 'Description of the task for Mint to perform autonomously')
    .action(async (description) => {
        const taskManager = require('./src/System/task_manager');
        const task = taskManager.addTask(description);
        console.log(`\n${colors.mint}${colors.bright}Task Received!${colors.reset}`);
        console.log(`${colors.gray}Task ID: ${task.id}${colors.reset}`);
        console.log(`"${description}"`);
        console.log(`\n${colors.cyan}Mint Agent is starting to work on this in the background.${colors.reset}`);
        console.log(`${colors.gray}You will receive a notification when it's done.${colors.reset}\n`);
    });

program
    .command('code')
    .description('Run Mint in workspace-aware coding mode for the current project')
    .argument('<task>', 'Coding task to execute in the current working directory')
    .action(async (task) => {
        console.log(`\n${colors.mint}${colors.bright}[Mint Code]${colors.reset} Workspace: ${process.cwd()}`);
        console.log(`${colors.gray}[Mint Code] Task: ${task}${colors.reset}\n`);

        try {
            const result = await executeCodeTask(task, {
                cwd: process.cwd(),
                onProgress: (message) => {
                    console.log(`${colors.gray}[Mint Code] ${message}${colors.reset}`);
                },
                requestApproval: requestCodeApproval
            });

            console.log(`\n${colors.mint}${colors.bright}Summary${colors.reset}`);
            console.log(result.summary);
            console.log(`\n${colors.cyan}Verification:${colors.reset} ${result.verification}`);
            console.log(`${colors.gray}Completed in ${result.steps} step(s).${colors.reset}\n`);
        } catch (error) {
            console.error(`\n${colors.pink}[Mint Code Error]${colors.reset} ${error.message}\n`);
            process.exitCode = 1;
        }
    });

program.parse(process.argv);

/**
 * The Interactive Chat Loop — Gemini-style TUI
 */
async function startInteractiveChat(initialMessage = null) {
    let lastResponseText = "";
    const { screen, appendMessage, streamMessage, setThinking, updateStatusModel, copyLastResponse, requestApproval, setMode } = createChatUI({
        onSubmit: async (text) => {
            if (text.startsWith('/')) {
                if (text.startsWith('/agent')) {
                    const args = text.split(' ');
                    if (args[1] === 'list') {
                        appendMessage('system', `Available Agents: ${agentOrchestrator.listAgents().join(', ')}`);
                    } else if (args[1]) {
                        const success = agentOrchestrator.setAgent(args[1]);
                        if (success) {
                            const agent = agentOrchestrator.getCurrentAgent();
                            appendMessage('system', `Switched to Agent: ${agent.icon} ${agent.name}`);
                            updateStatusModel(null, agent.name); // Pass name to status bar
                            resetChat(); // Reset to apply new system prompt
                        } else {
                            appendMessage('error', `Agent "${args[1]}" not found. Try /agent list`);
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
                    const args = text.split(' ');
                    const subCmd = args[1];

                    if (subCmd === 'add') {
                        const name = args[2];
                        const wsPath = args[3] || '.';
                        const instructions = args.slice(4).join(' ');
                        if (!name) {
                            appendMessage('error', 'Usage: /workspace add <name> [path] [instructions]');
                        } else {
                            workspaceManager.addWorkspace(name, wsPath, instructions);
                            appendMessage('system', `Workspace "${name}" registered at ${path.resolve(wsPath)}`);
                            resetChat();
                        }
                    } else if (subCmd === 'list') {
                        const all = workspaceManager.listWorkspaces();
                        let listMsg = "Registered Workspaces:\n";
                        for (const n in all) listMsg += `- ${n}: ${all[n].path}\n`;
                        appendMessage('system', Object.keys(all).length ? listMsg : "No workspaces registered.");
                    } else if (subCmd === 'remove') {
                        const name = args[2];
                        if (workspaceManager.removeWorkspace(name)) {
                            appendMessage('system', `Removed workspace "${name}"`);
                            resetChat();
                        } else {
                            appendMessage('error', `Workspace "${name}" not found.`);
                        }
                    } else {
                        const ws = workspaceManager.getWorkspaceByPath(process.cwd());
                        appendMessage('system', ws ? `Current Workspace: ${ws.name}\nPath: ${ws.path}` : "Not currently in a registered workspace.\nUsage: /workspace <add|list|remove>");
                    }
                    return;
                }

                if (text.startsWith('/review')) {
                    if (!lastResponseText) {
                        appendMessage('error', 'Nothing to review yet. Get a response first.');
                        return;
                    }
                    agentOrchestrator.setAgent('reviewer');
                    appendMessage('system', '⚖️ Requesting second-pass review from Mint Reviewer...');
                    text = `Please review this previous response and provide a critique:\n\n${lastResponseText}`;
                } else {
                    // Other slash commands
                    const fakeRl = { close: () => { } };
                    appendMessage('user', text);
                    await handleSlashCommandUI(text, appendMessage, updateStatusModel, copyLastResponse, setThinking, requestApproval, setMode);
                    return;
                }
            }
            appendMessage('user', text);

            const routeDecision = await detectCodeIntent(text, process.cwd());
            if (routeDecision.route === 'code') {
                appendMessage('system', `Router: entering Code Mode. ${routeDecision.reason}`);
                await runChatRoutedTask(text, {
                    appendMessage,
                    setThinking,
                    requestApproval,
                    setMode
                });
                return;
            }

            setMode('Chat');

            // Start thinking timer
            let seconds = 0;
            setThinking(true, seconds);
            const timer = setInterval(() => {
                seconds++;
                setThinking(true, seconds);
            }, 1000);

            try {
                const config = require('./src/System/config_manager').readConfig();
                const provider = config.aiProvider || 'gemini';
                const currentAgent = agentOrchestrator.getCurrentAgent();
                updateStatusModel(null, currentAgent.name);
                if (provider === 'gemini') {
                    // ── Streaming path (Gemini only) ──────────────────────────────────
                    // Gemini returns JSON so we buffer all chunks and progressively
                    // extract the "response" field as more of the JSON arrives.
                    clearInterval(timer);

                    let jsonBuffer = '';
                    let finalParsed = null;
                    let streamer = null;
                    let displayedChars = 0; // chars of response text already sent to TUI

                    try {
                        for await (const event of handleGeminiChatStream(text)) {
                            if (event.chunk) {
                                jsonBuffer += event.chunk;

                                // Progressively extract readable text from the growing JSON buffer
                                const match = jsonBuffer.match(/"response"\s*:\s*"((?:[^"\\]|\\.)*)"/s);
                                if (match) {
                                    const fullText = match[1]
                                        .replace(/\\n/g, '\n')
                                        .replace(/\\"/g, '"')
                                        .replace(/\\\\/g, '\\');
                                    const newChars = fullText.slice(displayedChars);
                                    if (newChars.length > 0) {
                                        if (!streamer) {
                                            setThinking(false);
                                            streamer = streamMessage('assistant');
                                        }
                                        streamer.appendChunk(newChars);
                                        displayedChars = fullText.length;
                                    }
                                }
                            } else if (event.done) {
                                finalParsed = event.parsed;
                                // Flush any remaining response text not yet displayed
                                if (finalParsed && finalParsed.response) {
                                    const remaining = finalParsed.response.slice(displayedChars);
                                    if (!streamer) {
                                        setThinking(false);
                                        streamer = streamMessage('assistant');
                                    }
                                    if (remaining) streamer.appendChunk(remaining);
                                }
                                if (streamer) {
                                    streamer.finalize(event.timestamp);
                                } else {
                                    setThinking(false);
                                    appendMessage('assistant',
                                        finalParsed ? finalParsed.response : '',
                                        event.timestamp);
                                }
                            }
                        }
                    } catch (streamErr) {
                        setThinking(false);
                        appendMessage('error', streamErr.message);
                        return;
                    }

                    // Execute Actions from the final parsed response
                    if (finalParsed) {
                        const { executeAction } = require('./mint-cli-logic');
                        if (finalParsed.action && finalParsed.action.type !== 'none') {
                            const result = await executeAction(finalParsed.action);
                            if (result) appendMessage('system', `Action: ${result}`);
                        }
                    }

                } else {
                    // ── Non-streaming fallback (Ollama, Anthropic, OpenAI, etc.) ──
                    const response = await handleChat(text);
                    clearInterval(timer);
                    setThinking(false);
                    lastResponseText = response.response;
                    appendMessage('assistant', response.response, response.timestamp);

                    const { executeAction } = require('./mint-cli-logic');
                    if (response.action && response.action.type !== 'none') {
                        const result = await executeAction(response.action);
                        if (result) appendMessage('system', `Action: ${result}`);
                    }
                }
            } catch (err) {
                clearInterval(timer);
                setThinking(false);
                appendMessage('error', err.message);
            }
        },
        onExit: () => {
            screen.destroy();
            // Explicitly restore terminal state and disable ALL mouse tracking modes
            process.stdout.write('\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l'); 
            process.stdout.write('\x1b[?25h');   // Show cursor
            console.log(`\n${colors.pink}Goodbye! See you again soon!${colors.reset}\n`);
            process.exit(0);
        }
    });

    // Handle initial message if passed via CLI arg
    if (initialMessage) {
        appendMessage('user', initialMessage);
        const routeDecision = await detectCodeIntent(initialMessage, process.cwd());
        if (routeDecision.route === 'code') {
            appendMessage('system', `Router: entering Code Mode. ${routeDecision.reason}`);
            await runChatRoutedTask(initialMessage, {
                appendMessage,
                setThinking,
                requestApproval,
                setMode
            });
        } else {
            setMode('Chat');
            let seconds = 0;
            setThinking(true, seconds);
            const timer = setInterval(() => { seconds++; setThinking(true, seconds); }, 1000);
            try {
                const response = await handleChat(initialMessage);
                clearInterval(timer);
                setThinking(false);
                appendMessage('assistant', response.response, response.timestamp);
            } catch (err) {
                clearInterval(timer);
                setThinking(false);
                appendMessage('error', err.message);
            }
        }
    }
}

/**
 * Handles slash commands within the TUI context
 */
async function handleSlashCommandUI(input, appendMessage, updateStatusModel, copyLastResponse, setThinking, requestApproval, setMode) {
    const parts = input.split(' ');
    const command = parts[0].toLowerCase();
    const args = parts.slice(1);

    switch (command) {
        case '/help':
        case '/?':
            appendMessage('system', [
                'Mint Slash Commands:',
                '  /code <task>     — Force workspace Code Mode',
                '  /models [name]  — List or switch Gemini models',
                '  /config         — Show current configuration',
                '  /copy           — Copy last response to clipboard',
                '  /clear          — Clear conversation history',
                '  /reset          — Reset conversation history',
                '  /exit           — Exit Mint'
            ].join('\n'));
            break;

        case '/model':
        case '/models':
            const config = readConfig();
            if (args.length === 0) {
                appendMessage('system', [
                    `Current Provider: ${config.aiProvider}`,
                    `Current Gemini Model: ${config.geminiModel}`,
                    'Available Providers/Presets:',
                    '  - gemini-2.5-flash (Default Gemini)',
                    '  - ollama (Local provider)',
                    '  - anthropic (Claude)',
                    '  - openai (GPT)',
                    '  - huggingface (Inference API)',
                    '  - local (LM Studio / OpenAI Compatible)',
                    'Usage: /models <name> to switch'
                ].join('\n'));
            } else {
                const { writeConfig } = require('./src/System/config_manager');
                const newModel = args[0];
                let newProvider = 'gemini';
                
                if (newModel === 'ollama') {
                    newProvider = 'ollama';
                } else if (newModel === 'anthropic') {
                    newProvider = 'anthropic';
                } else if (newModel === 'openai') {
                    newProvider = 'openai';
                } else if (newModel === 'huggingface') {
                    newProvider = 'huggingface';
                } else if (newModel === 'local' || newModel === 'local_openai') {
                    newProvider = 'local_openai';
                } else if (newModel.startsWith('gpt-')) {
                    newProvider = 'openai';
                    config.openaiModel = newModel;
                } else if (newModel.startsWith('claude-')) {
                    newProvider = 'anthropic';
                    config.anthropicModel = newModel;
                } else {
                    newProvider = 'gemini';
                    config.geminiModel = newModel;
                }
                
                config.aiProvider = newProvider;
                writeConfig(config);
                appendMessage('system', `✅ Switched to: ${newProvider} ${newProvider === 'gemini' ? `(${newModel})` : ''}`);
                if (updateStatusModel) updateStatusModel(newProvider === 'gemini' ? newModel : newProvider);
            }
            break;

        case '/code':
            if (args.length === 0) {
                appendMessage('system', 'Usage: /code <task>');
                break;
            }
            await runChatRoutedTask(`/code ${args.join(' ')}`, {
                appendMessage,
                setThinking,
                requestApproval,
                setMode
            });
            break;

        case '/config':
            const currentCfg = readConfig();
            appendMessage('system', [
                'Current Configuration:',
                `  Version   : v${pkg.version}`,
                `  Provider  : ${currentCfg.aiProvider}`,
                `  Model     : ${currentCfg.geminiModel}`,
                `  Ollama    : ${currentCfg.ollamaModel}`,
                `  Voice     : ${currentCfg.enableVoiceReply ? 'ON' : 'OFF'}`,
                `  Language  : ${currentCfg.language}`,
                `  API Key   : ${currentCfg.apiKey ? 'SET (****)' : 'NOT SET'}`
            ].join('\n'));
            break;

        case '/copy':
            if (copyLastResponse && copyLastResponse()) {
                appendMessage('system', '✓ Last response copied to clipboard.');
            } else {
                appendMessage('system', '✖ Nothing to copy, or xclip/xsel not installed.');
            }
            break;

        case '/clear':
        case '/reset':
            resetChat();
            appendMessage('system', 'Conversation history cleared.');
            break;

        case '/exit':
        case '/quit':
            process.exit(0);
            break;

        default:
            appendMessage('system', `Unknown command: ${command}. Type /help for options.`);
    }
}

async function requestCodeApproval(request) {
    const typeLabel = request.type === 'shell'
        ? 'Shell Command'
        : request.type === 'patch'
            ? 'Patch Edit'
            : 'File Write';

    console.log(`\n${colors.yellow}${colors.bright}[Approval Required]${colors.reset} ${typeLabel}`);
    if (request.label) {
        console.log(`${colors.gray}${request.label}${colors.reset}`);
    }
    if (request.preview) {
        console.log(`${colors.gray}${request.preview}${colors.reset}\n`);
    }

    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    const answer = await new Promise((resolve) => {
        rl.question('Approve this action? [y/N]: ', (value) => {
            rl.close();
            resolve((value || '').trim().toLowerCase());
        });
    });

    const approved = answer === 'y' || answer === 'yes';
    console.log(approved
        ? `${colors.mint}[Mint Code] Approved.${colors.reset}\n`
        : `${colors.pink}[Mint Code] Denied.${colors.reset}\n`);
    return approved;
}
