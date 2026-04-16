#!/usr/bin/env node
require('dotenv').config();
const { Command } = require('commander');
const { handleChat, resetChat } = require('./src/AI_Brain/Gemini_API');
const { runOnboarding } = require('./src/CLI/onboarding');
const { startAgent } = require('./src/AI_Brain/headless_agent');
const { displayFeatures } = require('./src/CLI/list_features');
const { readConfig, writeConfig } = require('./src/System/config_manager');
const readline = require('readline');

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
 * The Interactive Chat Loop
 */
async function startInteractiveChat(initialMessage = null) {
    console.clear();
    console.log(`${colors.mint}${colors.bright}`);
    console.log(`  __  __ _       _      _____ _      _____ `);
    console.log(` |  \\/  (_)     | |    / ____| |    |_   _|`);
    console.log(` | \\  / |_ _ __ | |_  | |    | |      | |  `);
    console.log(` | |\\/| | | '_ \\| __| | |    | |      | |  `);
    console.log(` | |  | | | | | | |_  | |____| |____ _| |_ `);
    console.log(` |_|  |_|_|_| |_|\\__|  \\_____|______|_____|`);
    console.log(`${colors.reset}`);
    console.log(`${colors.bright}Welcome to Mint Interactive AI!${colors.reset}`);
    console.log(`${colors.gray}Type 'exit' to leave. Type '/help' for slash commands.${colors.reset}\n`);

    const completer = (line) => {
        const completions = ['/models', '/config', '/clear', '/reset', '/exit', '/help'];
        const hits = completions.filter((c) => c.startsWith(line));
        // Show all completions if none found or if user just typed /
        return [hits.length ? hits : completions, line];
    };

    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
        completer: completer,
        prompt: `${colors.mint}${colors.bright}Mint > ${colors.reset}`
    });

    if (initialMessage) {
        await processInput(initialMessage, rl);
    }

    rl.prompt();

    rl.on('line', async (line) => {
        await processInput(line, rl);
        rl.prompt();
    });
}

async function processInput(line, rl) {
    const input = line.trim();

    if (input.toLowerCase() === 'exit' || input.toLowerCase() === 'quit') {
        console.log(`${colors.pink}Goodbye! See you again soon!${colors.reset}`);
        process.exit(0);
    }

    if (input.startsWith('/')) {
        await handleSlashCommand(input, rl);
        return;
    }

    if (!input) return;

    process.stdout.write(`${colors.gray}Mint is thinking...${colors.reset}\r`);

    try {
        const response = await handleChat(input);
        readline.clearLine(process.stdout, 0);
        readline.cursorTo(process.stdout, 0);

        console.log(`\n${colors.pink}${colors.bright}Mint: ${colors.reset}${response.response}\n`);

        // Execute Actions
        const { executeAction } = require('./mint-cli-logic');
        if (response.action && response.action.type !== 'none') {
            const result = await executeAction(response.action);
            if (result) console.log(`${colors.cyan}Action: ${result}${colors.reset}\n`);
        }

    } catch (err) {
        readline.clearLine(process.stdout, 0);
        readline.cursorTo(process.stdout, 0);
        console.error(`Error: ${err.message}`);
    }
}

async function handleSlashCommand(input, rl) {
    const parts = input.split(' ');
    const command = parts[0].toLowerCase();
    const args = parts.slice(1);

    switch (command) {
        case '/help':
        case '/?':
            console.log(`\n${colors.mint}${colors.bright}Mint Slash Commands:${colors.reset}`);
            console.log(`  ${colors.cyan}/models [name]${colors.reset}  : List or switch Gemini models`);
            console.log(`  ${colors.cyan}/config${colors.reset}         : Show current configuration`);
            console.log(`  ${colors.cyan}/clear${colors.reset}          : Clear screen and conversation`);
            console.log(`  ${colors.cyan}/reset${colors.reset}          : Reset conversation history`);
            console.log(`  ${colors.cyan}/exit${colors.reset}           : Close the interactive chat\n`);
            break;

        case '/models':
            const config = readConfig();
            if (args.length === 0) {
                console.log(`\n${colors.mint}Current Model:${colors.reset} ${config.geminiModel}`);
                console.log(`${colors.mint}Available Presets:${colors.reset}`);
                console.log(`  - gemini-3.1-flash-lite-preview (Default)`);
                console.log(`  - gemini-2.5-flash`);
                console.log(`  - gemini-3.1-flash-lite`);
                console.log(`  - ollama (Switch to local provider)`);
                console.log(`\n${colors.gray}Usage: /models <name> to switch${colors.reset}\n`);
            } else {
                const newModel = args[0];
                if (newModel === 'ollama') {
                    config.aiProvider = 'ollama';
                } else {
                    config.aiProvider = 'gemini';
                    config.geminiModel = newModel;
                }
                writeConfig(config);
                console.log(`\n${colors.cyan}✅ Switched to: ${newModel}${colors.reset}\n`);
            }
            break;

        case '/config':
            const currentCfg = readConfig();
            console.log(`\n${colors.mint}${colors.bright}Current Configuration:${colors.reset}`);
            console.log(`  - Provider  : ${currentCfg.aiProvider}`);
            console.log(`  - Model     : ${currentCfg.geminiModel}`);
            console.log(`  - Ollama    : ${currentCfg.ollamaModel}`);
            console.log(`  - Voice     : ${currentCfg.enableVoiceReply ? "ON" : "OFF"}`);
            console.log(`  - Language  : ${currentCfg.language}`);
            console.log(`  - API Key   : ${currentCfg.apiKey ? "SET (****)" : "NOT SET"}\n`);
            break;

        case '/clear':
            console.clear();
            resetChat();
            console.log(`${colors.mint}Screen and conversation history cleared.${colors.reset}\n`);
            break;

        case '/reset':
            resetChat();
            console.log(`${colors.mint}Conversation history reset.${colors.reset}\n`);
            break;

        case '/exit':
        case '/quit':
            console.log(`${colors.pink}Goodbye! See you again soon!${colors.reset}`);
            process.exit(0);
            break;

        default:
            console.log(`${colors.yellow}Unknown slash command: ${command}. Type /help for options.${colors.reset}\n`);
    }
}
