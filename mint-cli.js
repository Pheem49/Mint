#!/usr/bin/env node
require('dotenv').config();
const { Command } = require('commander');
const { handleChat } = require('./src/AI_Brain/Gemini_API');
const { runOnboarding } = require('./src/CLI/onboarding');
const { startAgent } = require('./src/AI_Brain/headless_agent');
const { displayFeatures } = require('./src/CLI/list_features');
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
  .action(async () => {
    await startAgent();
  });

// List Command
program
  .command('list')
  .description('Show list of Mint features and commands')
  .action(() => {
    displayFeatures();
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
    console.log(`${colors.gray}Type 'exit' to leave. Type 'list' for commands.${colors.reset}\n`);

    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
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
