#!/usr/bin/env node
require('dotenv').config({ quiet: true });
// Suppress experimental SQLite warning
const originalEmit = process.emit;
process.emit = function (name, data, ...args) {
    if (name === 'warning' && typeof data === 'object' && data.name === 'ExperimentalWarning' && data.message.includes('SQLite')) {
        return false;
    }
    return originalEmit.apply(process, [name, data, ...args]);
};
const { Command } = require('commander');
const fs = require('fs');
const path = require('path');
const { handleChat, handleGeminiChatStream, resetChat, refreshApiKeyFromConfig, getChatTranscript } = require('./src/AI_Brain/Gemini_API');
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
const memoryStore = require('./src/AI_Brain/memory_store');
const readline = require('readline');
const { createChatUI } = require('./src/CLI/chat_ui');
const { runUpdate, runStartupAutoUpdate, shouldRunAutoUpdate } = require('./src/CLI/updater');
const { runGmailAuth } = require('./src/CLI/gmail_auth');
const { loadImageAsDataUri, loadClipboardImageAsDataUri } = require('./src/CLI/image_input');

// Startup Info
const startupConfig = readConfig();
const startupProvider = startupConfig.aiProvider || 'gemini';
const startupModel = startupProvider === 'openai'
    ? (startupConfig.openaiModel || 'gpt-4o')
    : startupProvider === 'anthropic'
        ? (startupConfig.anthropicModel || 'claude-3-5-sonnet-latest')
        : startupProvider === 'local_openai'
            ? (startupConfig.localModelName || 'local-model')
            : startupProvider === 'ollama'
                ? (startupConfig.ollamaModel || 'llama3:latest')
                : (startupConfig.geminiModel || 'gemini-2.5-flash');
const startupNow = new Date();
const startupTime = startupNow.toLocaleString('th-TH', { 
    day: '2-digit', month: '2-digit', year: 'numeric', 
    hour: '2-digit', minute: '2-digit', hour12: false 
}).replace(',', '');
console.log(`\x1b[38;5;121m[Mint] v${pkg.version} | ${startupTime} | Active AI: ${startupProvider} • ${startupModel}\x1b[0m`);

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

let isExiting = false;

function exitWithGoodbye(code = 0) {
    if (isExiting) return;
    isExiting = true;

    process.stdout.write('\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l');
    process.stdout.write('\x1b[?25h');
    console.log(`\n${colors.pink}Goodbye! See you again soon!${colors.reset}\n`);
    process.exit(code);
}

process.once('SIGINT', () => {
    exitWithGoodbye(0);
});

function formatProgress(info) {
    if (typeof info === 'string') return `${colors.gray}[Mint Code] ${info}${colors.reset}`;

    const { step, phase, action, target, message } = info;
    
    if (action === 'ask_user') {
        return `\n${colors.mint}✓${colors.reset} ${colors.bright}Ask User${colors.reset}\n${colors.gray}   ${target || message || ''}${colors.reset}`;
    }

    let icon = `${colors.mint}✓${colors.reset}`;
    let label = action || phase;
    let color = colors.reset;

    switch (action) {
        case 'thinking': 
            return `\n${colors.yellow}* ${colors.bright}Thinking${colors.reset}`;
        case 'web_search': label = 'WebSearch'; break;
        case 'list_files':
        case 'find_path': label = 'Explored'; break;
        case 'read_file': label = 'ReadFile'; break;
        case 'search_code': label = 'SearchText'; break;
        case 'apply_patch':
        case 'write_file': label = 'Edited'; break;
        case 'run_shell': label = 'Ran command'; break;
        case 'json_repair': icon = '*'; label = 'Repairing JSON'; break;
        case 'reviewer_start': label = 'Reviewing'; break;
    }

    const content = target || message || '';
    return ` ${icon} ${colors.bright}${label}${colors.reset} ${color}${content}${colors.reset}`;
}

function formatMemoryInteractions(interactions, title = 'Remembered interactions') {
    if (!Array.isArray(interactions) || interactions.length === 0) {
        return `${title}:\n(no memories found)`;
    }

    const lines = [`${title}:`];
    interactions.forEach((item, index) => {
        const when = item.created_at ? ` (${item.created_at})` : '';
        const id = item.id ? `#${item.id} ` : '';
        lines.push(`${index + 1}. ${id}User${when}: ${item.user_text}`);
        lines.push(`   Mint: ${item.ai_text}`);
    });
    return lines.join('\n');
}

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

function splitResponseSentences(text) {
    const normalized = String(text || '').replace(/\r\n/g, '\n').trim();
    if (!normalized) return [];

    const sentences = [];
    let buffer = '';
    for (const char of normalized) {
        buffer += char;
        if (/[.!?。！？…\n]/u.test(char)) {
            const sentence = buffer.trim();
            if (sentence) sentences.push(sentence);
            buffer = '';
        }
    }

    const rest = buffer.trim();
    if (rest) sentences.push(rest);
    return sentences.length > 0 ? sentences : [normalized];
}

function learnSkillFile(filePath) {
    const targetPath = path.resolve(process.cwd(), filePath);
    if (!fs.existsSync(targetPath)) {
        throw new Error(`File not found: ${targetPath}`);
    }

    const stat = fs.statSync(targetPath);
    if (!stat.isFile()) {
        throw new Error(`Path is not a file: ${targetPath}`);
    }

    const ext = path.extname(targetPath).toLowerCase();
    if (ext !== '.md' && ext !== '.txt') {
        throw new Error('Mint learn currently supports .md and .txt files only.');
    }

    const maxBytes = 256 * 1024;
    if (stat.size > maxBytes) {
        throw new Error(`File is too large (${stat.size} bytes). Limit is ${maxBytes} bytes.`);
    }

    const content = fs.readFileSync(targetPath, 'utf8');
    return memoryStore.addLearnedSkill(path.basename(targetPath), targetPath, content);
}

const program = new Command();

program
    .name('mint')
    .description('Mint - Your Personal AI Assistant CLI')
    .version(pkg.version);

program.hook('preAction', async (thisCommand, actionCommand) => {
    if (actionCommand.name() === 'update' || process.env.MINT_SKIP_AUTO_UPDATE === '1') {
        return;
    }

    const config = readConfig();
    if (config.enableAutoUpdate === false) {
        return;
    }

    if (!shouldRunAutoUpdate(config)) {
        return;
    }

    console.log(`${colors.gray}[Mint Update] Checking for updates...${colors.reset}`);
    const result = await runStartupAutoUpdate(config, writeConfig);
    if (result.status === 'updated') {
        console.log(`${colors.mint}[Mint Update] ${result.message}${colors.reset}`);
    } else if (result.status === 'error') {
        console.log(`${colors.gray}[Mint Update] ${result.message}${colors.reset}`);
    }
});

// Chat Command (Interactive Mode)
program
    .command('chat', { isDefault: true })
    .description('Start interactive chat session with Mint')
    .argument('[message]', 'Initial message to send to Mint')
    .option('-i, --image <path>', 'Attach an image file to the initial message')
    .action(async (message, options) => {
        await startInteractiveChat(message, { imagePath: options.image });
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

program
    .command('learn')
    .description('Read a local markdown/text file and remember it as a Mint skill')
    .argument('[filePath]', 'Path to a .md or .txt skill/instruction file')
    .option('--delete <idOrPathOrName>', 'Delete a learned skill by id, path, or name')
    .option('--list', 'List learned skills')
    .action((filePath, options) => {
        try {
            if (options.list) {
                const skills = memoryStore.getLearnedSkills(50);
                if (skills.length === 0) {
                    console.log(`\n${colors.gray}No learned skills stored.${colors.reset}\n`);
                    return;
                }
                console.log(`\n${colors.bright}Learned Skills:${colors.reset}`);
                skills.forEach(skill => {
                    console.log(`${colors.mint}#${skill.id}${colors.reset} ${skill.name}`);
                    console.log(`  ${colors.gray}${skill.source_path}${colors.reset}`);
                });
                console.log('');
                return;
            }

            if (options.delete) {
                const deleted = memoryStore.deleteLearnedSkill(options.delete);
                if (deleted > 0) {
                    console.log(`\n${colors.mint}✓${colors.reset} Deleted learned skill: ${options.delete}\n`);
                } else {
                    console.log(`\n${colors.pink}✗${colors.reset} Learned skill not found: ${options.delete}\n`);
                    process.exitCode = 1;
                }
                return;
            }

            if (!filePath) {
                throw new Error('Usage: mint learn <path-to-skill.md>');
            }

            const learned = learnSkillFile(filePath);
            console.log(`\n${colors.mint}✓${colors.reset} Learned skill: ${learned.name}`);
            console.log(`${colors.gray}Path: ${learned.source_path}${colors.reset}`);
            if (learned.stored_length < learned.content_length) {
                console.log(`${colors.gray}Stored first ${learned.stored_length} of ${learned.content_length} characters.${colors.reset}`);
            }
            console.log('');
        } catch (error) {
            console.error(`\n${colors.pink}Learn failed:${colors.reset} ${error.message}\n`);
            process.exitCode = 1;
        }
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
    .command('update')
    .description('Check for and install the latest Mint CLI version from npm')
    .option('--check', 'Only check whether an update is available')
    .option('--dry-run', 'Show the npm update operation without installing')
    .action(async (options) => {
        console.log(`\n${colors.mint}${colors.bright}[Mint Update]${colors.reset} Checking npm for updates...`);

        try {
            const result = await runUpdate({
                checkOnly: options.check === true,
                dryRun: options.dryRun === true
            });

            const color = result.status === 'error' ? colors.pink : colors.mint;
            console.log(`${color}${result.message}${colors.reset}\n`);

            if (result.status === 'error') {
                process.exitCode = 1;
            }
        } catch (error) {
            console.error(`${colors.pink}Update failed: ${error.message}${colors.reset}\n`);
            process.exitCode = 1;
        }
    });

program
    .command('mcp')
    .description('Manage MCP (Model Context Protocol) servers')
    .addCommand(new Command('add')
        .description('Add a new MCP server')
        .argument('<name>', 'Server name')
        .argument('<command>', 'Command to run (e.g. npx)')
        .option('-a, --args <args...>', 'Command arguments')
        .option('-e, --env <env...>', 'Environment variables (KEY=VALUE)')
        .action((name, command, options) => {
            const config = readConfig();
            const mcpServers = config.mcpServers || {};
            
            const env = {};
            if (options.env) {
                options.env.forEach(kv => {
                    const [k, v] = kv.split('=');
                    if (k && v) env[k] = v;
                });
            }

            mcpServers[name] = {
                command,
                args: options.args || [],
                env
            };

            config.mcpServers = mcpServers;
            writeConfig(config);
            console.log(`\n${colors.mint}✓${colors.reset} MCP server "${name}" added successfully.`);
        })
    )
    .addCommand(new Command('remove')
        .description('Remove an MCP server')
        .argument('<name>', 'Server name')
        .action((name) => {
            const config = readConfig();
            if (config.mcpServers && config.mcpServers[name]) {
                delete config.mcpServers[name];
                writeConfig(config);
                console.log(`\n${colors.mint}✓${colors.reset} MCP server "${name}" removed.`);
            } else {
                console.log(`\n${colors.pink}✗${colors.reset} MCP server "${name}" not found.`);
            }
        })
    )
    .addCommand(new Command('list')
        .description('List configured MCP servers')
        .action(() => {
            const config = readConfig();
            const servers = Object.keys(config.mcpServers || {});
            if (servers.length === 0) {
                console.log(`\n${colors.gray}No MCP servers configured.${colors.reset}`);
            } else {
                console.log(`\n${colors.bright}Configured MCP Servers:${colors.reset}`);
                servers.forEach(name => {
                    const s = config.mcpServers[name];
                    console.log(`${colors.mint}• ${colors.bright}${name}${colors.reset}`);
                    console.log(`  ${colors.gray}Command:${colors.reset} ${s.command} ${(s.args || []).join(' ')}`);
                });
            }
        })
    )
    .addCommand(new Command('clear')
        .description('Remove all MCP servers')
        .action(() => {
            const config = readConfig();
            config.mcpServers = {};
            writeConfig(config);
            console.log(`\n${colors.mint}✓${colors.reset} All MCP servers cleared.`);
        })
    );

program
    .command('gmail')
    .description('Manage Gmail integration')
    .addCommand(new Command('auth')
        .description('Open Google OAuth login and save a Gmail refresh token')
        .option('--port <port>', 'Local callback port, defaults to a random available port')
        .option('--no-open', 'Print the auth link without opening a browser')
        .action(async (options) => {
            try {
                const result = await runGmailAuth({
                    port: options.port ? Number(options.port) : 0,
                    openBrowser: options.open,
                    logger: console
                });
                console.log(`\n${colors.mint}✓${colors.reset} Gmail connected for ${result.userId}. Refresh token saved.`);
                console.log(`${colors.gray}Scopes: ${result.scopes.join(', ')}${colors.reset}\n`);
            } catch (error) {
                console.error(`\n${colors.pink}Gmail auth failed:${colors.reset} ${error.message}\n`);
                process.exitCode = 1;
            }
        })
    );

program
    .command('code')
    .description('Run Mint in workspace-aware coding mode for the current project')
    .argument('<task>', 'Coding task to execute in the current working directory')
    .option('-i, --image <path>', 'Attach an image file as context for the coding task')
    .action(async (task, options) => {
        console.log(`\n${colors.mint}${colors.bright}[Mint Code]${colors.reset} Workspace: ${process.cwd()}`);

        try {
            let effectiveTask = task;
            let image = null;
            if (options.image) {
                image = loadImageAsDataUri(options.image);
                console.log(`${colors.gray}[Mint Code] Image: ${image.path}${colors.reset}`);
            }

            console.log(`${colors.gray}[Mint Code] Task: ${task}${colors.reset}\n`);

            const result = await executeCodeTask(effectiveTask, {
                cwd: process.cwd(),
                imageDataUri: image ? image.dataUri : null,
                imagePath: image ? image.path : null,
                onProgress: (info) => {
                    console.log(formatProgress(info));
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

program.parseAsync(process.argv).catch((error) => {
    console.error(`${colors.pink}${error.message}${colors.reset}`);
    process.exitCode = 1;
});

/**
 * The Interactive Chat Loop — Gemini-style TUI
 */
async function startInteractiveChat(initialMessage = null, options = {}) {
    let lastResponseText = "";
    let recentImageContextText = "";
    const formatErrorMessage = (err) => err && err.message ? err.message : String(err || 'Unknown error');
    const streamAssistantSentences = async (text, appendMessage, metadata = {}, streamMessage = null) => {
        const sentences = splitResponseSentences(text);
        if (typeof streamMessage === 'function') {
            const stream = streamMessage(metadata);
            for (let index = 0; index < sentences.length; index++) {
                const prefix = index === 0 ? '' : ' ';
                stream.appendChunk(`${prefix}${sentences[index]}`);
                if (index < sentences.length - 1) {
                    await sleep(90);
                }
            }
            stream.finalize();
            return;
        }

        for (let index = 0; index < sentences.length; index++) {
            appendMessage('assistant', sentences[index], index === 0 ? metadata : {});
            if (index < sentences.length - 1) {
                await sleep(90);
            }
        }
    };
    const sendImageMessage = async ({ images, image, prompt, appendMessage, streamMessage, setThinking, appendCodeStep }) => {
        const imageList = images || (image ? [image] : []);
        const message = prompt || 'Analyze this image.';
        const labels = imageList.map((_, index) => `[Image #${index + 1}]`).join(' ');
        const displayMessage = labels && message.includes(labels)
            ? message
            : `${message}\n${labels}`;
        appendMessage('user', displayMessage);
        if (appendCodeStep) {
            appendCodeStep({
                thought: imageList.length > 1
                    ? `Analyzing ${imageList.length} attached images before answering.`
                    : 'Analyzing the attached image before answering.'
            });
        }

        let seconds = 0;
        setThinking(true, seconds);
        const timer = setInterval(() => {
            seconds++;
            setThinking(true, seconds);
        }, 1000);

        try {
            const result = await handleChat(message, imageList.map(item => item.dataUri), null);
            clearInterval(timer);
            setThinking(false);
            const responseText = result.response || '';
            lastResponseText = responseText;
            recentImageContextText = [
                `Recent image context: the user attached ${imageList.length} image(s) labelled ${labels || '[Image #1]'}.`,
                'The terminal UI displays image attachments as labels only; it does not render thumbnails inside the chat.',
                `Assistant response to those image(s): ${responseText}`
            ].join('\n');
            await streamAssistantSentences(responseText, appendMessage, { providerInfo: result.providerInfo }, streamMessage);
            return responseText;
        } catch (err) {
            clearInterval(timer);
            setThinking(false);
            appendMessage('error', formatErrorMessage(err));
            return '';
        }
    };
    
    const ui = await createChatUI({
        onPasteImage: async () => {
            try {
                const image = loadClipboardImageAsDataUri();
                return { label: image.path, image };
            } catch (err) {
                throw new Error(formatErrorMessage(err));
            }
        },
        onSubmit: async (text, submitOptions = {}) => {
            const { screen, appendMessage, streamMessage, setThinking, updateStatusModel, copyLastResponse, requestApproval, setMode, appendCodeStep, updateWorkspace, askUser, attachImage, setInputText, setPendingPasteText, setFastMode, toggleFastMode, getFastMode } = ui;
            if (submitOptions.images && submitOptions.images.length > 0) {
                const images = submitOptions.images.map(item => item.image || item);
                await sendImageMessage({
                    images,
                    prompt: text.trim() || 'Analyze this image.',
                    appendMessage,
                    streamMessage,
                    setThinking,
                    appendCodeStep
                });
                return;
            }

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
                            updateStatusModel(agent.name); // Pass name to status bar
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
                    } else if (subCmd === 'use' || subCmd === 'switch') {
                        const name = args[2];
                        const all = workspaceManager.listWorkspaces();
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
                        appendMessage('system', ws ? `Current Workspace: ${ws.name}\nPath: ${ws.path}` : `Not currently in a registered workspace.\nActive Path: ${process.cwd()}\nUsage: /workspace <add|use|list|remove>`);
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
                    if (!text.startsWith('/image') && !text.startsWith('/paste')) {
                        appendMessage('user', text);
                    }
                    const slashResult = await handleSlashCommandUI(text, appendMessage, updateStatusModel, copyLastResponse, setThinking, requestApproval, setMode, appendCodeStep, updateWorkspace, {
                        sendImageMessage,
                        formatErrorMessage,
                        attachImage,
                        setInputText,
                        setPendingPasteText,
                        setFastMode,
                        toggleFastMode,
                        getFastMode,
                        streamAssistantSentences,
                        streamMessage
                    });
                    if (slashResult && slashResult.lastResponseText) {
                        lastResponseText = slashResult.lastResponseText;
                    }
                    return;
                }
            }
            appendMessage('user', text);

            const transcript = await getChatTranscript();
            const contextualHistory = recentImageContextText
                ? [...transcript, { sender: 'system', text: recentImageContextText, timestamp: new Date().toISOString() }]
                : transcript;
            if (setMode) setMode('Agent');

            let seconds = 0;
            setThinking(true, seconds);
            const timer = setInterval(() => {
                seconds++;
                setThinking(true, seconds);
            }, 1000);

            try {
                const config = require('./src/System/config_manager').readConfig();
                const availableProviders = require('./src/System/config_manager').getAvailableProviders(config);
                const preferredProvider = require('./src/CLI/code_agent')._helpers.selectSupportedCodeProvider(config, availableProviders);
                let streamedFinalSummary = false;

                const result = await executeCodeTask(text, {
                    cwd: process.cwd(),
                    requestApproval,
                    askUser,
                    provider: preferredProvider,
                    history: contextualHistory,
                    onProgress: (info) => {
                        if (appendCodeStep) appendCodeStep(info);
                    },
                    onFinalSummary: async (info) => {
                        clearInterval(timer);
                        setThinking(false);
                        streamedFinalSummary = true;
                        await streamAssistantSentences(info.summary, appendMessage, { providerInfo: info.providerInfo }, streamMessage);
                    }
                });

                clearInterval(timer);
                setThinking(false);
                lastResponseText = result.summary;
                if (!streamedFinalSummary) {
                    await streamAssistantSentences(result.summary, appendMessage, { providerInfo: result.providerInfo }, streamMessage);
                }

            } catch (err) {
                clearInterval(timer);
                setThinking(false);
                appendMessage('error', formatErrorMessage(err));
            } finally {
                if (setMode) setMode('Agent');
            }
        },
        onExit: () => {
            exitWithGoodbye(0);
        }
    });

    // Handle initial image if passed via CLI option.
    if (options.imagePath) {
        const { appendMessage, streamMessage, setThinking, appendCodeStep } = ui;
        const image = loadImageAsDataUri(options.imagePath);
        const prompt = initialMessage || 'Analyze this image.';
        await sendImageMessage({ images: [image], prompt, appendMessage, streamMessage, setThinking, appendCodeStep });

        return;
    }

    // Handle initial message if passed via CLI arg
    if (initialMessage) {
        const { appendMessage, streamMessage, setThinking, updateStatusModel, copyLastResponse, requestApproval, setMode, appendCodeStep, updateWorkspace, askUser } = ui;
        appendMessage('user', initialMessage);
        const transcript = await getChatTranscript();
        const contextualHistory = recentImageContextText
            ? [...transcript, { sender: 'system', text: recentImageContextText, timestamp: new Date().toISOString() }]
            : transcript;
        if (setMode) setMode('Agent');

        let seconds = 0;
        setThinking(true, seconds);
        const timer = setInterval(() => {
            seconds++;
            setThinking(true, seconds);
        }, 1000);

        try {
            const config = require('./src/System/config_manager').readConfig();
            const availableProviders = require('./src/System/config_manager').getAvailableProviders(config);
            const preferredProvider = require('./src/CLI/code_agent')._helpers.selectSupportedCodeProvider(config, availableProviders);
            let streamedFinalSummary = false;

            const result = await executeCodeTask(initialMessage, {
                cwd: process.cwd(),
                requestApproval,
                askUser,
                provider: preferredProvider,
                history: contextualHistory,
                onProgress: (info) => {
                    if (appendCodeStep) appendCodeStep(info);
                },
                onFinalSummary: async (info) => {
                    clearInterval(timer);
                    setThinking(false);
                    streamedFinalSummary = true;
                    await streamAssistantSentences(info.summary, appendMessage, { providerInfo: info.providerInfo }, streamMessage);
                }
            });

            clearInterval(timer);
            setThinking(false);
            lastResponseText = result.summary;
            if (!streamedFinalSummary) {
                await streamAssistantSentences(result.summary, appendMessage, { providerInfo: result.providerInfo }, streamMessage);
            }

        } catch (err) {
            clearInterval(timer);
            setThinking(false);
            appendMessage('error', formatErrorMessage(err));
        } finally {
            if (setMode) setMode('Agent');
        }
    }
}

/**
 * Handles slash commands within the TUI context
 */
async function handleSlashCommandUI(input, appendMessage, updateStatusModel, copyLastResponse, setThinking, requestApproval, setMode, appendCodeStep, updateWorkspace, helpers = {}) {
    const parts = input.split(' ');
    const command = parts[0].toLowerCase();
    const args = parts.slice(1);

    switch (command) {
        case '/help':
        case '/?':
            appendMessage('system', [
                'Mint Slash Commands:',
                '  /image <path> [prompt] — Attach an image from your computer',
                '  /paste [prompt]   — Attach an image from your clipboard',
                '  /fast [on|off]    — Hide or show thinking/progress output',
                '  /learn <path>     — Remember a .md/.txt file as a Mint skill',
                '  /code <task>      — Force workspace Code Mode',
                '  /cd <path>        — Change current working directory',
                '  /models [name]   — List or switch Gemini models',
                '  /memory [cmd]    — Manage long-term memory',
                '  /config          — Show current configuration',
                '  /copy            — Copy last response to clipboard',
                '  /clear           — Clear conversation history',
                '  /reset           — Reset conversation history',
                '  /exit            — Exit Mint'
            ].join('\n'));
            break;

        case '/fast': {
            if (!helpers.toggleFastMode || !helpers.setFastMode || !helpers.getFastMode) {
                appendMessage('error', 'Fast mode is not available in this UI.');
                break;
            }

            const option = (args[0] || '').toLowerCase();
            let enabled;
            if (option === 'on' || option === 'true' || option === '1') {
                enabled = helpers.setFastMode(true);
            } else if (option === 'off' || option === 'false' || option === '0') {
                enabled = helpers.setFastMode(false);
            } else if (option === 'status') {
                enabled = helpers.getFastMode();
            } else {
                enabled = helpers.toggleFastMode();
            }

            appendMessage('system', `Fast mode: ${enabled ? 'ON' : 'OFF'}`);
            break;
        }

        case '/learn': {
            const filePath = input.slice(command.length).trim();
            if (!filePath) {
                appendMessage('system', 'Usage: /learn <path-to-skill.md>');
                break;
            }

            try {
                const learned = learnSkillFile(filePath);
                appendMessage('system', [
                    `✓ Learned skill: ${learned.name}`,
                    `Path: ${learned.source_path}`,
                    learned.stored_length < learned.content_length
                        ? `Stored first ${learned.stored_length} of ${learned.content_length} characters.`
                        : `Stored ${learned.stored_length} characters.`
                ].join('\n'));
            } catch (err) {
                appendMessage('error', err && err.message ? err.message : String(err || 'Unknown error'));
            }
            break;
        }

        case '/image': {
            if (args.length === 0) {
                appendMessage('system', 'Usage: /image <path> [prompt]');
                break;
            }

            const imagePath = args[0];
            const prompt = args.slice(1).join(' ').trim();

            try {
                const image = loadImageAsDataUri(imagePath);
                if (helpers.attachImage) {
                    helpers.attachImage({ label: image.path, image });
                    if (prompt && helpers.setInputText) {
                        helpers.setInputText(prompt);
                    }
                    appendMessage('system', 'Attached image. Press Enter to send.');
                } else {
                    appendMessage('error', 'Image attachment is not available in this UI.');
                }
            } catch (err) {
                appendMessage('error', err && err.message ? err.message : String(err || 'Unknown error'));
            }
            break;
        }

        case '/paste': {
            try {
                const image = loadClipboardImageAsDataUri();
                if (helpers.attachImage) {
                    helpers.attachImage({ label: image.path, image });
                    const prompt = args.join(' ').trim();
                    if (prompt && helpers.setInputText) {
                        helpers.setInputText(prompt);
                    }
                    appendMessage('system', 'Attached clipboard image. Press Enter to send.');
                } else {
                    appendMessage('error', 'Image attachment is not available in this UI.');
                }
            } catch (err) {
                appendMessage('error', helpers.formatErrorMessage ? helpers.formatErrorMessage(err) : (err && err.message ? err.message : String(err || 'Unknown error')));
            }
            break;
        }

        case '/memory': {
            const subCommand = (args[0] || 'list').toLowerCase();
            const query = args.slice(1).join(' ').trim();

            if (subCommand === 'help') {
                appendMessage('system', [
                    'Memory Commands:',
                    '  /memory list [n]       — Show recent remembered interactions',
                    '  /memory search <query> — Search remembered interactions',
                    '  /memory skills        — Show learned skill files',
                    '  /memory skills delete <id|path|name> — Delete a learned skill',
                    '  /memory profile        — Show remembered profile fields',
                    '  /memory context [q]    — Show context Mint injects into prompts',
                    '  /memory delete <id>    — Delete one remembered interaction',
                    '  /memory export [path]  — Export memory snapshot as JSON',
                    '  /memory clear          — Clear episodic interaction memories'
                ].join('\n'));
                break;
            }

            if (subCommand === 'profile') {
                const profile = memoryStore.getAllProfile();
                appendMessage('system', Object.keys(profile).length
                    ? JSON.stringify(profile, null, 2)
                    : 'No profile memory stored yet.');
                break;
            }

            if (subCommand === 'skills') {
                if ((args[1] || '').toLowerCase() === 'delete') {
                    const identifier = args.slice(2).join(' ').trim();
                    if (!identifier) {
                        appendMessage('system', 'Usage: /memory skills delete <id|path|name>');
                        break;
                    }
                    const deleted = memoryStore.deleteLearnedSkill(identifier);
                    appendMessage('system', deleted > 0
                        ? `Deleted learned skill: ${identifier}`
                        : `Learned skill not found: ${identifier}`);
                    break;
                }

                const skills = memoryStore.getLearnedSkills(20);
                appendMessage('system', skills.length
                    ? [
                        'Learned skills:',
                        ...skills.map((skill) => `#${skill.id} ${skill.name}\n   ${skill.source_path}`)
                    ].join('\n')
                    : 'No learned skills stored yet.');
                break;
            }

            if (subCommand === 'context') {
                const ctx = memoryStore.getUserContext(query);
                appendMessage('system', ctx || 'No memory context stored yet.');
                break;
            }

            if (subCommand === 'search') {
                if (!query) {
                    appendMessage('system', 'Usage: /memory search <query>');
                    break;
                }
                const results = memoryStore.searchInteractions(query, 10);
                appendMessage('system', formatMemoryInteractions(results, `Search results for "${query}"`));
                break;
            }

            if (subCommand === 'export') {
                const exportPath = query
                    ? path.resolve(process.cwd(), query)
                    : path.join(process.cwd(), `mint-memory-export-${Date.now()}.json`);
                fs.writeFileSync(exportPath, JSON.stringify(memoryStore.exportMemorySnapshot(), null, 2), 'utf8');
                appendMessage('system', `Memory exported to: ${exportPath}`);
                break;
            }

            if (subCommand === 'delete') {
                const id = Number.parseInt(args[1] || '', 10);
                if (!Number.isFinite(id)) {
                    appendMessage('system', 'Usage: /memory delete <id>');
                    break;
                }
                const deleted = memoryStore.deleteInteractionMemory(id);
                appendMessage('system', deleted ? `Deleted memory #${id}.` : `Memory #${id} was not found.`);
                break;
            }

            if (subCommand === 'clear') {
                memoryStore.clearInteractionMemories();
                appendMessage('system', 'Cleared episodic interaction memories. Profile memory is unchanged.');
                break;
            }

            const limit = Number.parseInt(args[0] || '10', 10);
            const interactions = memoryStore.getRecentInteractions(Number.isFinite(limit) ? limit : 10);
            appendMessage('system', formatMemoryInteractions(interactions, 'Recent remembered interactions'));
            break;
        }

        case '/cd':
            if (args.length === 0) {
                appendMessage('system', `Current Directory: ${process.cwd()}`);
                break;
            }
            try {
                const newPath = path.resolve(process.cwd(), args[0]);
                if (fs.existsSync(newPath) && fs.lstatSync(newPath).isDirectory()) {
                    process.chdir(newPath);
                    if (updateWorkspace) updateWorkspace(newPath);
                    appendMessage('system', `✓ Directory changed to: ${newPath}`);
                } else {
                    appendMessage('error', `Directory not found: ${newPath}`);
                }
            } catch (err) {
                appendMessage('error', `Error: ${err.message}`);
            }
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
                appendCodeStep,
                setMode,
                streamAssistantSentences: helpers.streamAssistantSentences,
                streamMessage: helpers.streamMessage,
                askUser: () => Promise.resolve(''),
                history: await getChatTranscript()
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
            exitWithGoodbye(0);
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
