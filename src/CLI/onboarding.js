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
            message: 'Enter your Google Gemini API Key (Required for basic features):',
            default: config.apiKey || undefined,
            validate: (input) => input.trim().length > 0 ? true : 'API Key is required.'
        },
        {
            type: 'list',
            name: 'geminiModelChoice',
            message: 'Select the primary Gemini model to use:',
            choices: [
                'gemini-2.5-flash',
                'gemini-2.0-pro-exp-02-05',
                'gemini-3.1-flash-lite-preview',
                'gemini-3.1-flash-lite',
                'Custom model name'
            ],
            default: config.geminiModel || 'gemini-2.5-flash'
        },
        {
            type: 'input',
            name: 'customGeminiModel',
            message: 'Enter your custom Gemini model name:',
            when: (answers) => answers.geminiModelChoice === 'Custom model name',
            validate: (input) => input.trim().length > 0 ? true : 'Please enter a valid model name.'
        },
        {
            type: 'input',
            name: 'anthropicApiKey',
            message: 'Enter your Anthropic API Key (Optional, press Enter to skip):',
            default: config.anthropicApiKey || ''
        },
        {
            type: 'input',
            name: 'openaiApiKey',
            message: 'Enter your OpenAI API Key (Optional, press Enter to skip):',
            default: config.openaiApiKey || ''
        },
        {
            type: 'input',
            name: 'hfApiKey',
            message: 'Enter your Hugging Face API Key (Optional, press Enter to skip):',
            default: config.hfApiKey || ''
        },
        {
            type: 'input',
            name: 'localApiBaseUrl',
            message: 'Enter your Local AI (LM Studio/OpenAI Compatible) Base URL (Optional, press Enter to skip):',
            default: config.localApiBaseUrl || ''
        },
        {
            type: 'input',
            name: 'localModelName',
            message: 'Enter your Local Model Name (Optional, press Enter to skip):',
            default: config.localModelName || ''
        }
    ];

    const answers = await inquirer.prompt(questions);
    
    // Resolve custom gemini model if selected
    const geminiModel = answers.geminiModelChoice === 'Custom model name' 
        ? answers.customGeminiModel 
        : answers.geminiModelChoice;

    // Remove temporary choice fields before saving
    delete answers.geminiModelChoice;
    delete answers.customGeminiModel;

    // Save configuration
    const newConfig = { ...config, ...answers, geminiModel };
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
