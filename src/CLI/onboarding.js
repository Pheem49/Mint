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

    let config = readConfig();

    // 1. Basic Setup (Gemini is mandatory for core features)
    const basicAnswers = await inquirer.prompt([
        {
            type: 'input',
            name: 'apiKey',
            message: 'Enter your Google Gemini API Key:',
            default: config.apiKey || undefined,
            validate: (input) => input.trim().length > 0 ? true : 'API Key is required.'
        },
        {
            type: 'list',
            name: 'geminiModel',
            message: 'Select primary Gemini model:',
            choices: [
                'gemini-2.5-flash',
                'gemini-3.1-flash-lite-preview',
                'gemini-1.5-pro'
            ],
            default: config.geminiModel || 'gemini-2.5-flash'
        }
    ]);

    config = { ...config, ...basicAnswers };

    // 2. Interactive Channel/Provider Selection (QuickStart Style)
    const { selections } = await inquirer.prompt([
        {
            type: 'checkbox',
            name: 'selections',
            message: 'Select channels/providers to configure (QuickStart):',
            pageSize: 20,
            choices: [
                { name: 'Telegram (Bot API)', value: 'telegram', checked: config.enableTelegramBridge },
                { name: 'WhatsApp (QR link)', value: 'whatsapp', checked: config.enableWhatsappBridge },
                { name: 'Discord (Bot API)', value: 'discord', checked: config.enableDiscordBridge },
                { name: 'Slack (Socket Mode)', value: 'slack', checked: config.enableSlackBridge },
                { name: 'LINE (Messaging API)', value: 'line', checked: config.enableLineBridge },
                { name: 'Google Chat', value: 'gchat', disabled: 'Coming Soon' },
                new inquirer.Separator(),
                { name: 'Anthropic (Claude)', value: 'anthropic' },
                { name: 'OpenAI (GPT-4o)', value: 'openai' },
                { name: 'Hugging Face', value: 'hf' },
                { name: 'Local AI (LM Studio/Ollama)', value: 'local' },
                new inquirer.Separator(),
                { name: 'Google Search API', value: 'google_search' },
                { name: 'Brave Search API', value: 'brave_search' },
                new inquirer.Separator(),
                { name: 'Skip for now', value: 'skip' }
            ]
        }
    ]);

    // 3. Configure selected items
    const dynamicQuestions = [];
    
    // If "Skip for now" is selected or nothing is selected, we move to save
    if (selections.length === 0 || selections.includes('skip')) {
        console.log('\n⏩ Skipping optional configuration...');
    } else {
        if (selections.includes('google_search')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'googleSearchApiKey',
                message: 'Enter Google Search API Key:',
                default: config.googleSearchApiKey
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'googleSearchCx',
                message: 'Enter Google Search CX (Engine ID):',
                default: config.googleSearchCx
            });
        }

        if (selections.includes('brave_search')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'braveSearchApiKey',
                message: 'Enter Brave Search API Key:',
                default: config.braveSearchApiKey
            });
        }
        if (selections.includes('discord')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'discordBotToken',
                message: 'Enter Discord Bot Token:',
                default: config.discordBotToken
            });
            config.enableDiscordBridge = true;
        }

        if (selections.includes('telegram')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'telegramBotToken',
                message: 'Enter Telegram Bot Token:',
                default: config.telegramBotToken
            });
            config.enableTelegramBridge = true;
        } else { config.enableTelegramBridge = false; }

        if (selections.includes('whatsapp')) {
            config.enableWhatsappBridge = true;
        } else { config.enableWhatsappBridge = false; }

        if (selections.includes('slack')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'slackBotToken',
                message: 'Enter Slack Bot Token (xoxb-...):',
                default: config.slackBotToken
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'slackAppToken',
                message: 'Enter Slack App Token (xapp-...):',
                default: config.slackAppToken
            });
            config.enableSlackBridge = true;
        } else { config.enableSlackBridge = false; }

        if (selections.includes('line')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'lineChannelAccessToken',
                message: 'Enter LINE Channel Access Token:',
                default: config.lineChannelAccessToken
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'lineChannelSecret',
                message: 'Enter LINE Channel Secret:',
                default: config.lineChannelSecret
            });
            dynamicQuestions.push({
                type: 'number',
                name: 'lineWebhookPort',
                message: 'Enter LINE Webhook Port (Local):',
                default: config.lineWebhookPort || 3000
            });
            config.enableLineBridge = true;
        } else { config.enableLineBridge = false; }

        if (selections.includes('anthropic')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'anthropicApiKey',
                message: 'Enter Anthropic API Key:',
                default: config.anthropicApiKey
            });
        }

        if (selections.includes('openai')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'openaiApiKey',
                message: 'Enter OpenAI API Key:',
                default: config.openaiApiKey
            });
        }

        if (selections.includes('hf')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'hfApiKey',
                message: 'Enter Hugging Face API Key:',
                default: config.hfApiKey
            });
        }

        if (selections.includes('local')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'localApiBaseUrl',
                message: 'Enter Local AI Base URL:',
                default: config.localApiBaseUrl || 'http://localhost:1234/v1'
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'localModelName',
                message: 'Enter Local Model Name:',
                default: config.localModelName || 'local-model'
            });
        }
    }

    if (dynamicQuestions.length > 0) {
        const extraAnswers = await inquirer.prompt(dynamicQuestions);
        config = { ...config, ...extraAnswers };
    }

    // Save configuration
    writeConfig(config);
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
