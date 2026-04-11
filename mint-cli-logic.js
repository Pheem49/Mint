// Mint CLI Action Logic
const { openApp } = require('./src/Automation_Layer/open_app');
const { openWebsite, openSearch } = require('./src/Automation_Layer/open_website');
const { createFolder, openFile, deleteFile } = require('./src/Automation_Layer/file_operations');
const { indexFile } = require('./src/AI_Brain/knowledge_base');
const SystemAutomation = require('./src/System/system_automation');
const pluginManager = require('./src/Plugins/plugin_manager');

async function executeAction(action) {
    if (!action || action.type === 'none') return null;

    try {
        switch (action.type) {
            case 'open_url':
                openWebsite(action.target);
                return `Opened URL: ${action.target}`;
            case 'search':
                openSearch(action.target);
                return `Searching for: ${action.target}`;
            case 'open_app':
                openApp(action.target);
                return `Opening app: ${action.target}`;
            case 'create_folder':
                createFolder(action.target);
                return `Created folder: ${action.target}`;
            case 'open_file':
                await openFile(action.target);
                return `Opening: ${action.target}`;
            case 'delete_file':
                await deleteFile(action.target);
                return `Deleted: ${action.target}`;
            case 'learn_file':
                return await indexFile(action.target);
            case 'plugin':
                return await pluginManager.executePlugin(action.pluginName, action.target);
            case 'system_automation':
                return await handleSystemAutomation(action.target);
            default:
                return `Action ${action.type} is not yet fully supported in CLI.`;
        }
    } catch (err) {
        return `Error executing action: ${err.message}`;
    }
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
            throw new Error(`Unknown system command: ${target}`);
    }
}

module.exports = { executeAction };
