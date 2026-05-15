const { clipboard: electronClipboard } = require('electron');
const { openApp } = require('../Automation_Layer/open_app');
const { openWebsite, openSearch } = require('../Automation_Layer/open_website');
const { performWebAutomation } = require('../Automation_Layer/browser_automation');
const { createFolder, openFile, deleteFile, findPath } = require('../Automation_Layer/file_operations');
const { indexFile, indexFolder } = require('../AI_Brain/knowledge_base');
const pluginManager = require('../Plugins/plugin_manager');
const mcpManager = require('../Plugins/mcp_manager');
const granularAutomation = require('./granular_automation');
const SystemAutomation = require('./system_automation');
const safetyManager = require('./safety_manager');

async function executeAction(action, options = {}) {
    console.log("Executing action:", action);
    const clipboard = options.clipboard || electronClipboard;
    const safety = safetyManager.assertActionAllowed(action, {
        allowDangerous: options.allowDangerous === true
    });
    safetyManager.appendActionLog({
        source: options.source || 'action_executor',
        action: action.type,
        target: action.target || action.path || '',
        tier: safety.tier,
        approved: options.allowDangerous === true || safety.tier !== safetyManager.TIERS.DANGEROUS
    });

    switch (action.type) {
        case 'open_url':
            openWebsite(action.target);
            break;
        case 'search':
            openSearch(action.target);
            break;
        case 'open_app':
            openApp(action.target);
            break;
        case 'web_automation':
            return await performWebAutomation(action.target);
        case 'create_folder':
            createFolder(action.target);
            break;
        case 'open_file': {
            const fileRes = await openFile(action.target);
            return fileRes || `Successfully opened file: ${action.target} ✅`;
        }
        case 'open_folder': {
            const folderRes = await openFile(action.target);
            return folderRes || `Successfully opened folder: ${action.target} ✅`;
        }
        case 'delete_file':
            await deleteFile(action.target);
            break;
        case 'find_path':
            return await executeFindPath(action);
        case 'clipboard_write':
            clipboard.writeText(action.target);
            break;
        case 'learn_file':
            return await indexFile(action.target);
        case 'learn_folder':
            return await indexFolder(action.target);
        case 'mcp_tool': {
            const mcpResult = await mcpManager.callTool(action.server, action.target, action.args);
            return JSON.stringify(mcpResult.content);
        }
        case 'mouse_move':
            return await granularAutomation.mouseMove(action.x, action.y);
        case 'mouse_click':
            return await granularAutomation.mouseClick(action.x, action.y, action.button || 1);
        case 'type_text':
            return await granularAutomation.typeText(action.target);
        case 'key_tap':
            return await granularAutomation.keyTap(action.target);
        case 'plugin':
            return await pluginManager.executePlugin(action.pluginName, action.target);
        case 'system_automation':
            return await handleSystemAutomation(action.target);
        default:
            return undefined;
    }
}

async function executeFindPath(action) {
    const result = findPath(action.target, {
        type: action.pathType,
        maxResults: 10
    });
    if (!result.success) {
        return result.message;
    }

    if (action.openAfter === true) {
        if (result.matches.length === 1) {
            const match = result.matches[0];
            const openResult = await openFile(match.path);
            return openResult || `Successfully found and opened ${match.type === 'dir' ? 'folder' : 'file'}: ${match.path} ✅`;
        }
        return `Found multiple matches for "${action.target}". Please be more specific:\n${result.matches.map(m => `- [${m.type}] ${m.path}`).join('\n')}`;
    }

    return `Found matches for "${action.target}":\n${result.matches.map(m => `- [${m.type}] ${m.path}`).join('\n')}`;
}

async function handleSystemAutomation(target) {
    const [cmd, value] = target.split(':');
    switch (cmd) {
        case 'volume':
            return await SystemAutomation.setVolume(parseInt(value));
        case 'mute':
            return await SystemAutomation.mute();
        case 'brightness':
            return await SystemAutomation.setBrightness(parseInt(value));
        case 'sleep':
            return await SystemAutomation.sleep();
        case 'restart':
            return await SystemAutomation.restart();
        case 'shutdown':
            return await SystemAutomation.shutdown();
        case 'minimize_all':
            return await SystemAutomation.minimizeAll();
        default:
            if (SystemAutomation[target]) {
                return await SystemAutomation[target]();
            }
            throw new Error(`Unknown system automation command: ${target}`);
    }
}

module.exports = { executeAction, handleSystemAutomation };
