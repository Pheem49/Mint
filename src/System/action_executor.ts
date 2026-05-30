let electronClipboard = null;
try {
    ({ clipboard: electronClipboard } = require('electron'));
} catch (_) {
    electronClipboard = {
        writeText: () => {}
    };
}
import { openApp  } from '../Automation_Layer/open_app'
import { openWebsite, openSearch  } from '../Automation_Layer/open_website'
import { performWebAutomation  } from '../Automation_Layer/browser_automation'
import { createFolder, openFile, deleteFile, findPath  } from '../Automation_Layer/file_operations'
import { indexFile, indexFolder  } from '../AI_Brain/knowledge_base'
import { getSystemInfo, getWeather  } from './system_info'
import pluginManager from '../Plugins/plugin_manager'
import mcpManager from '../Plugins/mcp_manager'
import SystemAutomation from './system_automation'
import * as safetyManager from './safety_manager'
import * as toolRegistry from './tool_registry'
import granularAutomation from './granular_automation'
import * as os from 'os'
import * as path from 'path'

async function executeAction(action, options: any = {}) {
    if (process.env.MINT_DEBUG === '1') {
        console.log("Executing action:", action);
    }
    toolRegistry.validateToolInput(action.type, action);
    const clipboard = options.clipboard || electronClipboard;
    const safety = safetyManager.assertActionAllowed(action, {
        allowApproval: options.allowApproval === true,
        allowDangerous: options.allowDangerous === true
    });
    safetyManager.appendActionLog({
        source: options.source || 'action_executor',
        action: action.type,
        target: action.target || action.path || '',
        tier: safety.tier,
        approved: options.allowApproval === true || options.allowDangerous === true || safety.tier === safetyManager.TIERS.SAFE
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
            safetyManager.assertPathCapability(action.target, 'write', {
                defaultBase: path.join(os.homedir(), 'Desktop')
            });
            createFolder(action.target);
            break;
        case 'open_file': {
            safetyManager.assertPathCapability(action.target, 'read');
            const fileRes = await openFile(action.target);
            return fileRes || `Successfully opened file: ${action.target} ✅`;
        }
        case 'open_folder': {
            safetyManager.assertPathCapability(action.target, 'read');
            const folderRes = await openFile(action.target);
            return folderRes || `Successfully opened folder: ${action.target} ✅`;
        }
        case 'delete_file':
            safetyManager.assertPathCapability(action.target, 'write');
            await deleteFile(action.target);
            break;
        case 'find_path':
            return await executeFindPath(action);
        case 'clipboard_write':
            clipboard.writeText(action.target);
            break;
        case 'learn_file':
            safetyManager.assertPathCapability(action.target, 'read');
            return await indexFile(action.target);
        case 'learn_folder':
            safetyManager.assertPathCapability(action.target, 'read');
            return await indexFolder(action.target);
        case 'system_info':
            return await handleSystemInfo(action.target);
        case 'mcp_tool': {
            const mcpResult = await mcpManager.callTool(action.server, action.target, action.args);
            return JSON.stringify(mcpResult.content);
        }
        case 'mouse_move': {
            return await granularAutomation.mouseMove(action.x, action.y);
        }
        case 'mouse_click': {
            return await granularAutomation.mouseClick(action.x, action.y, action.button || 1);
        }
        case 'type_text': {
            return await granularAutomation.typeText(action.target);
        }
        case 'key_tap': {
            return await granularAutomation.keyTap(action.target);
        }
        case 'plugin':
            return await pluginManager.executePlugin(action.pluginName, action.target);
        case 'system_automation':
            return await handleSystemAutomation(action.target);
        default:
            return undefined;
    }
}

async function handleSystemInfo(target = '') {
    const query = String(target || '').trim();
    if (query) {
        const weather = await getWeather(query);
        return JSON.stringify({
            type: 'weather',
            target: query,
            ...weather
        });
    }
    return JSON.stringify({
        type: 'system_info',
        data: getSystemInfo()
    });
}

async function executeFindPath(action) {
    const result = findPath(action.target, {
        type: action.pathType,
        maxResults: 10,
        roots: safetyManager.getAllowedRoots('read')
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

export { executeAction, handleSystemAutomation, handleSystemInfo  }
