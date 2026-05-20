const fs = require('fs');
const path = require('path');
const { readConfig, writeConfig } = require('../System/config_manager');
const { installDaemon } = require('../System/daemon_manager');
const { runGmailAuth } = require('./gmail_auth');

const CUSTOM_MODEL_VALUE = '__custom_model__';
const ANTHROPIC_MODEL_CHOICES = [
    'claude-3-5-sonnet-latest',
    'claude-3-opus-latest',
    'claude-3-5-haiku-latest'
];
const OPENAI_MODEL_CHOICES = [
    'gpt-4o',
    'gpt-4o-mini',
    'o1-preview',
    'o1-mini'
];

function buildModelChoices(models, currentModel) {
    const choices = models.map(model => ({ name: model, value: model }));
    if (currentModel && !models.includes(currentModel)) {
        choices.push({ name: `${currentModel} (current)`, value: currentModel });
    }
    choices.push({ name: 'Custom...', value: CUSTOM_MODEL_VALUE });
    return choices;
}

function resolveCustomModelSelection(answers, modelKey, customKey, fallbackModel) {
    if (answers[modelKey] !== CUSTOM_MODEL_VALUE) return;
    const customModel = typeof answers[customKey] === 'string' ? answers[customKey].trim() : '';
    answers[modelKey] = customModel || fallbackModel;
    delete answers[customKey];
}

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
                { name: 'Google Calendar API', value: 'google_calendar', checked: config.pluginCalendarEnabled },
                { name: 'Gmail API', value: 'gmail', checked: config.pluginGmailEnabled },
                { name: 'Notion API', value: 'notion', checked: config.pluginNotionEnabled },
                new inquirer.Separator(),
                { name: 'Anthropic (Claude)', value: 'anthropic', checked: !!config.anthropicApiKey },
                { name: 'OpenAI (GPT-4o)', value: 'openai', checked: !!config.openaiApiKey },
                { name: 'Hugging Face', value: 'hf', checked: !!config.hfApiKey },
                { name: 'Local AI (LM Studio/Ollama)', value: 'local', checked: !!(config.localApiBaseUrl && config.localApiBaseUrl.length > 0) },
                new inquirer.Separator(),
                { name: 'Google Search API', value: 'google_search', checked: !!config.googleSearchApiKey },
                { name: 'Brave Search API', value: 'brave_search', checked: !!config.braveSearchApiKey },
                new inquirer.Separator(),
                { name: 'Skip for now', value: 'skip' }
            ]
        }
    ]);

    // 3. Configure selected items
    const dynamicQuestions = [];
    
    // Reset enabled flags if we are not skipping
    if (!selections.includes('skip')) {
        config.enableTelegramBridge = selections.includes('telegram');
        config.enableWhatsappBridge = selections.includes('whatsapp');
        config.enableDiscordBridge = selections.includes('discord');
        config.enableSlackBridge = selections.includes('slack');
        config.enableLineBridge = selections.includes('line');
    }

    // If "Skip for now" is selected or nothing is selected, we move to save
    if (selections.includes('skip')) {
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
        }

        if (selections.includes('telegram')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'telegramBotToken',
                message: 'Enter Telegram Bot Token:',
                default: config.telegramBotToken
            });
        }

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
        }

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
        }

        if (selections.includes('google_calendar')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'googleCalendarClientId',
                message: 'Enter Google Calendar OAuth Client ID:',
                default: config.googleCalendarClientId
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'googleCalendarClientSecret',
                message: 'Enter Google Calendar OAuth Client Secret:',
                default: config.googleCalendarClientSecret
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'googleCalendarRefreshToken',
                message: 'Enter Google Calendar Refresh Token:',
                default: config.googleCalendarRefreshToken
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'googleCalendarId',
                message: 'Enter Google Calendar ID:',
                default: config.googleCalendarId || 'primary'
            });
            config.pluginCalendarEnabled = true;
        } else {
            config.pluginCalendarEnabled = false;
        }

        if (selections.includes('gmail')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'gmailClientId',
                message: 'Enter Gmail OAuth Client ID:',
                default: config.gmailClientId
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'gmailClientSecret',
                message: 'Enter Gmail OAuth Client Secret:',
                default: config.gmailClientSecret
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'gmailRefreshToken',
                message: 'Enter Gmail Refresh Token:',
                default: config.gmailRefreshToken
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'gmailUserId',
                message: 'Enter Gmail User ID:',
                default: config.gmailUserId || 'me'
            });
            config.pluginGmailEnabled = true;
        } else {
            config.pluginGmailEnabled = false;
        }

        if (selections.includes('notion')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'notionApiKey',
                message: 'Enter Notion Internal Integration Secret:',
                default: config.notionApiKey
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'notionDatabaseId',
                message: 'Enter default Notion Database ID (optional):',
                default: config.notionDatabaseId
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'notionPageId',
                message: 'Enter default Notion Page ID (optional):',
                default: config.notionPageId
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'notionTitleProperty',
                message: 'Enter database title property name:',
                default: config.notionTitleProperty || 'Name'
            });
            config.pluginNotionEnabled = true;
        } else {
            config.pluginNotionEnabled = false;
        }

        if (selections.includes('anthropic')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'anthropicApiKey',
                message: 'Enter Anthropic API Key:',
                default: config.anthropicApiKey,
                validate: (input) => input.trim().length > 0 ? true : 'Anthropic API Key is required when Anthropic is selected.'
            });
            dynamicQuestions.push({
                type: 'list',
                name: 'anthropicModel',
                message: 'Select Anthropic model:',
                choices: buildModelChoices(ANTHROPIC_MODEL_CHOICES, config.anthropicModel),
                default: config.anthropicModel || 'claude-3-5-sonnet-latest'
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'anthropicModelCustom',
                message: 'Enter custom Anthropic model:',
                default: config.anthropicModel || 'claude-3-5-sonnet-latest',
                when: (answers) => answers.anthropicModel === CUSTOM_MODEL_VALUE,
                validate: (input) => input.trim().length > 0 ? true : 'Model name is required.'
            });
        }

        if (selections.includes('openai')) {
            dynamicQuestions.push({
                type: 'input',
                name: 'openaiApiKey',
                message: 'Enter OpenAI API Key:',
                default: config.openaiApiKey,
                validate: (input) => input.trim().length > 0 ? true : 'OpenAI API Key is required when OpenAI is selected.'
            });
            dynamicQuestions.push({
                type: 'list',
                name: 'openaiModel',
                message: 'Select OpenAI model:',
                choices: buildModelChoices(OPENAI_MODEL_CHOICES, config.openaiModel),
                default: config.openaiModel || 'gpt-4o'
            });
            dynamicQuestions.push({
                type: 'input',
                name: 'openaiModelCustom',
                message: 'Enter custom OpenAI model:',
                default: config.openaiModel || 'gpt-4o',
                when: (answers) => answers.openaiModel === CUSTOM_MODEL_VALUE,
                validate: (input) => input.trim().length > 0 ? true : 'Model name is required.'
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
        resolveCustomModelSelection(extraAnswers, 'anthropicModel', 'anthropicModelCustom', config.anthropicModel || 'claude-3-5-sonnet-latest');
        resolveCustomModelSelection(extraAnswers, 'openaiModel', 'openaiModelCustom', config.openaiModel || 'gpt-4o');
        config = { ...config, ...extraAnswers };
        
    }

    // Onboarding treats Gemini as the primary AI. Other providers are optional
    // configured backends that become available only when credentials are set.
    config.aiProvider = 'gemini';

    // Save configuration
    writeConfig(config);
    console.log('\n✅ Configuration saved successfully!');

    if (!selections.includes('skip') && selections.includes('gmail') && !config.gmailRefreshToken) {
        const { runGmailAuthNow } = await inquirer.prompt([
            {
                type: 'confirm',
                name: 'runGmailAuthNow',
                message: 'Gmail Refresh Token is empty. Start Gmail OAuth now?',
                default: true
            }
        ]);

        if (runGmailAuthNow) {
            console.log('\n🔐 Starting Gmail OAuth. Open the link below, sign in, and approve access.');
            try {
                const result = await runGmailAuth({
                    logger: console,
                    openBrowser: false
                });
                console.log(`✅ Gmail connected for ${result.userId}. Refresh token saved.`);
            } catch (err) {
                console.error(`❌ Gmail OAuth failed: ${err.message}`);
                console.log('You can retry later with: mint gmail auth');
            }
        } else {
            console.log('You can connect Gmail later with: mint gmail auth');
        }
    }

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
