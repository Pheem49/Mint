const fs = require('fs');
const path = require('path');
const { readConfig, writeConfig } = require('../System/config_manager');
const { installDaemon } = require('../System/daemon_manager');

/**
 * Onboarding Wizard for Mint CLI
 */
async function runOnboarding(options = {}) {
    // Dynamic import for ESM-only inquirer in CommonJS
    const inquirer = (await import('inquirer')).default;
    
    console.log('\nWelcome to Mint Onboarding! Let\'s get you set up.\n');

    const config = readConfig();

    const questions = [
        {
            type: 'input',
            name: 'apiKey',
            message: 'Please enter your Google Gemini API Key:',
            default: config.apiKey || undefined,
            validate: (input) => input.length > 0 ? true : 'API Key is required.'
        },
        {
            type: 'list',
            name: 'geminiModel',
            message: 'Select the Gemini model to use:',
            choices: [
                'gemini-3.1-flash-lite-preview',
                'gemini-2.0-flash',
                'gemini-2.0-pro-exp-02-05'
            ],
            default: config.geminiModel || 'gemini-3.1-flash-lite-preview'
        }
    ];

    const answers = await inquirer.prompt(questions);

    // Save configuration
    const newConfig = { ...config, ...answers };
    writeConfig(newConfig);
    console.log('\n✅ Configuration saved successfully!');

    // Install Daemon if requested
    if (options.installDaemon) {
        console.log('\n🚀 Installing Mint Background Agent (Daemon)...');
        try {
            const result = await installDaemon();
            console.log(`✅ ${result}`);
        } catch (err) {
            console.error(`❌ Failed to install daemon: ${err.message}`);
        }
    }

    console.log('\nAll set! You can now use "mint chat" to start talking to me.\n');
}

module.exports = { runOnboarding };
