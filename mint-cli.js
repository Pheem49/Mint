#!/usr/bin/env node
require('dotenv').config();
const { Command } = require('commander');
const { handleChat, resetChat } = require('./src/AI_Brain/Gemini_API');
const { runOnboarding } = require('./src/CLI/onboarding');
const { startAgent } = require('./src/AI_Brain/headless_agent');
const { displayFeatures } = require('./src/CLI/list_features');
const { readConfig, writeConfig } = require('./src/System/config_manager');
const readline = require('readline');
const { createChatUI } = require('./src/CLI/chat_ui');

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
  .version('1.0.0');

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

program.parse(process.argv);

/**
 * The Interactive Chat Loop — Gemini-style TUI
 */
async function startInteractiveChat(initialMessage = null) {
    const { screen, appendMessage, setThinking, updateStatusModel, copyLastResponse } = createChatUI({
        onSubmit: async (text) => {
            if (text.startsWith('/')) {
                // Slash commands via fake rl-compatible object
                const fakeRl = { close: () => {} };
                appendMessage('user', text);
                await handleSlashCommandUI(text, appendMessage, updateStatusModel, copyLastResponse);
                return;
            }
            appendMessage('user', text);

            // Start thinking timer
            let seconds = 0;
            setThinking(true, seconds);
            const timer = setInterval(() => {
                seconds++;
                setThinking(true, seconds);
            }, 1000);

            try {
                const response = await handleChat(text);
                clearInterval(timer);
                setThinking(false);
                appendMessage('assistant', response.response);

                // Execute Actions
                const { executeAction } = require('./mint-cli-logic');
                if (response.action && response.action.type !== 'none') {
                    const result = await executeAction(response.action);
                    if (result) appendMessage('system', `Action: ${result}`);
                }
            } catch (err) {
                clearInterval(timer);
                setThinking(false);
                appendMessage('error', err.message);
            }
        },
        onExit: () => {
            screen.destroy();
            console.log(`\n${colors.pink}Goodbye! See you again soon!${colors.reset}\n`);
            process.exit(0);
        }
    });

    // Handle initial message if passed via CLI arg
    if (initialMessage) {
        appendMessage('user', initialMessage);
        let seconds = 0;
        setThinking(true, seconds);
        const timer = setInterval(() => { seconds++; setThinking(true, seconds); }, 1000);
        try {
            const response = await handleChat(initialMessage);
            clearInterval(timer);
            setThinking(false);
            appendMessage('assistant', response.response);
        } catch (err) {
            clearInterval(timer);
            setThinking(false);
            appendMessage('error', err.message);
        }
    }
}

/**
 * Handles slash commands within the TUI context
 */
async function handleSlashCommandUI(input, appendMessage, updateStatusModel, copyLastResponse) {
    const parts = input.split(' ');
    const command = parts[0].toLowerCase();
    const args = parts.slice(1);

    switch (command) {
        case '/help':
        case '/?':
            appendMessage('system', [
                'Mint Slash Commands:',
                '  /models [name]  — List or switch Gemini models',
                '  /config         — Show current configuration',
                '  /copy           — Copy last response to clipboard',
                '  /clear          — Clear conversation history',
                '  /reset          — Reset conversation history',
                '  /exit           — Exit Mint'
            ].join('\n'));
            break;

        case '/models':
            const config = readConfig();
            if (args.length === 0) {
                appendMessage('system', [
                    `Current Model: ${config.geminiModel}`,
                    'Available Presets:',
                    '  - gemini-3.1-flash-lite-preview (Default)',
                    '  - gemini-2.5-flash',
                    '  - gemini-3.1-flash-lite',
                    '  - ollama (local provider)',
                    'Usage: /models <name> to switch'
                ].join('\n'));
            } else {
                const { writeConfig } = require('./src/System/config_manager');
                const newModel = args[0];
                if (newModel === 'ollama') {
                    config.aiProvider = 'ollama';
                } else {
                    config.aiProvider = 'gemini';
                    config.geminiModel = newModel;
                }
                writeConfig(config);
                appendMessage('system', `✅ Switched to: ${newModel}`);
                if (updateStatusModel) updateStatusModel(newModel);
            }
            break;

        case '/config':
            const currentCfg = readConfig();
            appendMessage('system', [
                'Current Configuration:',
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

