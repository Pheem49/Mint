
import * as fs from 'fs'
import * as path from 'path'

import { colors, exitWithGoodbye  } from './cli_colors'
import { formatMemoryInteractions  } from './cli_formatters'
import { learnSkillFile  } from './skill_manager'
import { loadImageAsDataUri, loadClipboardImageAsDataUri  } from './image_input'
import { runChatRoutedTask  } from './chat_router'
import { getChatTranscript, resetChat  } from '../AI_Brain/Gemini_API'
import * as memoryStore from '../AI_Brain/memory_store'
import * as agentOrchestrator from '../AI_Brain/agent_orchestrator'
import { readConfig, writeConfig  } from '../System/config_manager'
import pkg from '../../package.json'

/**
 * Handles all slash commands entered inside the interactive TUI.
 *
 * @param {string}   input             Full slash command string (e.g. "/memory list")
 * @param {Function} appendMessage
 * @param {Function} updateStatusModel
 * @param {Function} copyLastResponse
 * @param {Function} setThinking
 * @param {Function} requestApproval
 * @param {Function} setMode
 * @param {Function} appendCodeStep
 * @param {Function} updateWorkspace
 * @param {object}   helpers           Extra helpers injected from interactive_chat
 * @returns {Promise<object|undefined>}  May return { lastResponseText } for some commands
 */
async function handleSlashCommandUI(
    input,
    appendMessage,
    updateStatusModel,
    copyLastResponse,
    setThinking,
    requestApproval,
    setMode,
    appendCodeStep,
    updateWorkspace,
    helpers: any = {}
) {
    const parts   = input.split(' ');
    const command = parts[0].toLowerCase();
    const args    = parts.slice(1);

    switch (command) {
        // ------------------------------------------------------------------ /help
        case '/help':
        case '/?':
            appendMessage('system', [
                'Mint Slash Commands:',
                '  /image <path> [prompt] — Attach an image from your computer',
                '  /paste [prompt]   — Attach an image from your clipboard',
                '  /fast [on|off]    — Hide or show thinking/progress output',
                '  /summarize [path] [--json] — Summarize repository structure',
                '  /symbols [path] [--json] [--limit n] — Build a source symbol index',
                '  /semantic-code index|search <query> — Embed and search code semantically',
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

        // ------------------------------------------------------------------ /fast
        case '/fast': {
            if (!helpers.toggleFastMode || !helpers.setFastMode || !helpers.getFastMode) {
                appendMessage('error', 'Fast mode is not available in this UI.');
                break;
            }
            const option = (args[0] || '').toLowerCase();
            let enabled;
            if      (option === 'on'  || option === 'true'  || option === '1') enabled = helpers.setFastMode(true);
            else if (option === 'off' || option === 'false' || option === '0') enabled = helpers.setFastMode(false);
            else if (option === 'status') enabled = helpers.getFastMode();
            else enabled = helpers.toggleFastMode();

            appendMessage('system', `Fast mode: ${enabled ? 'ON' : 'OFF'}`);
            break;
        }

        // ------------------------------------------------------------------ /summarize
        case '/summarize':
        case '/summary': {
            if (typeof helpers.sendRepoSummaryMessage !== 'function') {
                appendMessage('error', 'Repository summary is not available in this UI.');
                break;
            }
            const responseText = await helpers.sendRepoSummaryMessage({
                rawArgs:       input.slice(command.length).trim(),
                appendMessage,
                streamMessage: helpers.streamMessage,
                setThinking
            });
            return { lastResponseText: responseText };
        }

        // ------------------------------------------------------------------ /symbols
        case '/symbols':
        case '/symbol-index': {
            if (typeof helpers.sendSymbolIndexMessage !== 'function') {
                appendMessage('error', 'Symbol index is not available in this UI.');
                break;
            }
            const responseText = await helpers.sendSymbolIndexMessage({
                rawArgs:       input.slice(command.length).trim(),
                appendMessage,
                streamMessage: helpers.streamMessage,
                setThinking
            });
            return { lastResponseText: responseText };
        }

        // ------------------------------------------------------------------ /semantic-code
        case '/semantic-code':
        case '/semantic': {
            if (typeof helpers.sendSemanticCodeMessage !== 'function') {
                appendMessage('error', 'Semantic code search is not available in this UI.');
                break;
            }
            const responseText = await helpers.sendSemanticCodeMessage({
                rawArgs:       input.slice(command.length).trim(),
                appendMessage,
                streamMessage: helpers.streamMessage,
                setThinking,
                appendCodeStep
            });
            return { lastResponseText: responseText };
        }

        // ------------------------------------------------------------------ /learn
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

        // ------------------------------------------------------------------ /image
        case '/image': {
            if (args.length === 0) {
                appendMessage('system', 'Usage: /image <path> [prompt]');
                break;
            }
            const imagePath = args[0];
            const prompt    = args.slice(1).join(' ').trim();
            try {
                const image = loadImageAsDataUri(imagePath);
                if (helpers.attachImage) {
                    if (prompt && helpers.setInputText) helpers.setInputText(prompt);
                    helpers.attachImage({ label: image.path, image });
                    appendMessage('system', 'Attached image. Press Enter to send.');
                } else {
                    appendMessage('error', 'Image attachment is not available in this UI.');
                }
            } catch (err) {
                appendMessage('error', err && err.message ? err.message : String(err || 'Unknown error'));
            }
            break;
        }

        // ------------------------------------------------------------------ /paste
        case '/paste': {
            try {
                const image = loadClipboardImageAsDataUri();
                if (helpers.attachImage) {
                    const prompt = args.join(' ').trim();
                    if (prompt && helpers.setInputText) helpers.setInputText(prompt);
                    helpers.attachImage({ label: image.path, image });
                    appendMessage('system', 'Attached clipboard image. Press Enter to send.');
                } else {
                    appendMessage('error', 'Image attachment is not available in this UI.');
                }
            } catch (err) {
                const msg = helpers.formatErrorMessage
                    ? helpers.formatErrorMessage(err)
                    : (err && err.message ? err.message : String(err || 'Unknown error'));
                appendMessage('error', msg);
            }
            break;
        }

        // ------------------------------------------------------------------ /memory
        case '/memory': {
            const subCommand = (args[0] || 'list').toLowerCase();
            const query      = args.slice(1).join(' ').trim();

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
                    ? ['Learned skills:', ...skills.map(s => `#${s.id} ${s.name}\n   ${s.source_path}`)].join('\n')
                    : 'No learned skills stored yet.');
                break;
            }

            if (subCommand === 'context') {
                const ctx = memoryStore.getUserContext(query);
                appendMessage('system', ctx || 'No memory context stored yet.');
                break;
            }

            if (subCommand === 'search') {
                if (!query) { appendMessage('system', 'Usage: /memory search <query>'); break; }
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
                if (!Number.isFinite(id)) { appendMessage('system', 'Usage: /memory delete <id>'); break; }
                const deleted = memoryStore.deleteInteractionMemory(id);
                appendMessage('system', deleted ? `Deleted memory #${id}.` : `Memory #${id} was not found.`);
                break;
            }

            if (subCommand === 'clear') {
                memoryStore.clearInteractionMemories();
                appendMessage('system', 'Cleared episodic interaction memories. Profile memory is unchanged.');
                break;
            }

            // Default: list recent
            const limit = Number.parseInt(args[0] || '10', 10);
            const interactions = memoryStore.getRecentInteractions(Number.isFinite(limit) ? limit : 10);
            appendMessage('system', formatMemoryInteractions(interactions, 'Recent remembered interactions'));
            break;
        }

        // ------------------------------------------------------------------ /cd
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

        // ------------------------------------------------------------------ /models
        case '/model':
        case '/models': {
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
                const newModel   = args[0];
                let newProvider  = 'gemini';

                if      (newModel === 'ollama')                        newProvider = 'ollama';
                else if (newModel === 'anthropic')                     newProvider = 'anthropic';
                else if (newModel === 'openai')                        newProvider = 'openai';
                else if (newModel === 'huggingface')                   newProvider = 'huggingface';
                else if (newModel === 'local' || newModel === 'local_openai') newProvider = 'local_openai';
                else if (newModel.startsWith('gpt-'))   { newProvider = 'openai';    config.openaiModel    = newModel; }
                else if (newModel.startsWith('claude-')) { newProvider = 'anthropic'; config.anthropicModel = newModel; }
                else                                    { newProvider = 'gemini';    config.geminiModel    = newModel; }

                config.aiProvider = newProvider;
                writeConfig(config);
                appendMessage('system', `✅ Switched to: ${newProvider} ${newProvider === 'gemini' ? `(${newModel})` : ''}`);
                if (updateStatusModel) updateStatusModel(newProvider === 'gemini' ? newModel : newProvider);
            }
            break;
        }

        // ------------------------------------------------------------------ /code
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
                streamMessage:            helpers.streamMessage,
                askUser:                  () => Promise.resolve(''),
                history:                  await getChatTranscript()
            });
            break;

        // ------------------------------------------------------------------ /config
        case '/config': {
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
        }

        // ------------------------------------------------------------------ /copy
        case '/copy':
            if (copyLastResponse && copyLastResponse()) {
                appendMessage('system', '✓ Last response copied to clipboard.');
            } else {
                appendMessage('system', '✖ Nothing to copy, or xclip/xsel not installed.');
            }
            break;

        // ------------------------------------------------------------------ /clear /reset
        case '/clear':
        case '/reset':
            resetChat();
            appendMessage('system', 'Conversation history cleared.');
            break;

        // ------------------------------------------------------------------ /exit
        case '/exit':
        case '/quit':
            exitWithGoodbye(0);
            break;

        // ------------------------------------------------------------------ default
        default:
            appendMessage('system', `Unknown command: ${command}. Type /help for options.`);
    }
}

export { handleSlashCommandUI  }
