const TOOL_REGISTRY = Object.freeze({
    none: {
        permission: 'safe',
        required: [],
        description: 'No action.'
    },
    web_search: {
        permission: 'safe',
        required: ['query'],
        codeAgentOnly: true,
        description: 'Search the internet when outside knowledge is required.'
    },
    list_files: {
        permission: 'safe',
        required: [],
        codeAgentOnly: true,
        description: 'List files under a workspace-relative path.'
    },
    read_file: {
        permission: 'safe',
        required: ['path'],
        codeAgentOnly: true,
        description: 'Read a workspace file, optionally bounded by startLine/endLine.'
    },
    search_code: {
        permission: 'safe',
        required: ['query'],
        codeAgentOnly: true,
        description: 'Search text in the workspace.'
    },
    find_path: {
        permission: 'safe',
        required: ['query'],
        chatAction: true,
        description: 'Find files or folders by name.'
    },
    run_shell: {
        permission: 'approval',
        required: ['command'],
        codeAgentOnly: true,
        important: true,
        description: 'Run a non-destructive shell command after user approval.'
    },
    verify: {
        permission: 'approval',
        required: [],
        codeAgentOnly: true,
        important: true,
        description: 'Run test/build/lint verification commands after user approval.'
    },
    plan: {
        permission: 'approval',
        required: ['plan'],
        codeAgentOnly: true,
        description: 'Present a multi-file edit plan before changing files.'
    },
    apply_patch: {
        permission: 'approval',
        required: ['patch'],
        codeAgentOnly: true,
        important: true,
        description: 'Patch an existing file after user approval.'
    },
    write_file: {
        permission: 'approval',
        required: ['path', 'content'],
        codeAgentOnly: true,
        important: true,
        description: 'Create or replace a file after user approval.'
    },
    ask_user: {
        permission: 'safe',
        required: ['question'],
        codeAgentOnly: true,
        description: 'Ask the user for clarification.'
    },
    open_url: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Open a URL.'
    },
    search: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Open a web search.'
    },
    open_app: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Open a local application.'
    },
    web_automation: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        important: true,
        description: 'Perform browser automation.'
    },
    create_folder: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Create a folder.'
    },
    open_file: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Open a local file.'
    },
    open_folder: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Open a local folder.'
    },
    delete_file: {
        permission: 'dangerous',
        required: ['target'],
        chatAction: true,
        important: true,
        description: 'Delete a file only after explicit dangerous-action permission.'
    },
    clipboard_write: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Write text to clipboard.'
    },
    learn_file: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Index a file into the knowledge base.'
    },
    learn_folder: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        description: 'Index a folder into the knowledge base.'
    },
    system_info: {
        permission: 'safe',
        required: [],
        chatAction: true,
        description: 'Read local system info, or weather when target is a city.'
    },
    plugin: {
        permission: 'safe',
        required: ['pluginName', 'target'],
        chatAction: true,
        description: 'Run a Mint plugin.'
    },
    mcp_tool: {
        permission: 'safe',
        required: ['server', 'target'],
        chatAction: true,
        description: 'Call an MCP tool.'
    },
    mouse_click: {
        permission: 'safe',
        required: ['x', 'y'],
        chatAction: true,
        important: true,
        description: 'Click at screen coordinates.'
    },
    mouse_move: {
        permission: 'safe',
        required: ['x', 'y'],
        chatAction: true,
        description: 'Move the mouse.'
    },
    type_text: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        important: true,
        description: 'Type text into the active UI.'
    },
    key_tap: {
        permission: 'safe',
        required: ['target'],
        chatAction: true,
        important: true,
        description: 'Press a key.'
    },
    system_automation: {
        permission: 'approval',
        required: ['target'],
        chatAction: true,
        important: true,
        description: 'Change system settings after approval.'
    },
    finish: {
        permission: 'safe',
        required: ['summary'],
        codeAgentOnly: true,
        description: 'Finish the task and reply.'
    }
});

function getTool(name) {
    return TOOL_REGISTRY[name] || null;
}

function listToolNames(filter: any = {}) {
    return Object.entries(TOOL_REGISTRY)
        .filter(([, tool]: [string, any]) => {
            if (filter.chatAction === true && tool.chatAction !== true) return false;
            if (filter.codeAgent === true && tool.chatAction === true && tool.codeAgentOnly !== true) return true;
            return true;
        })
        .map(([name]) => name);
}

function listChatActionNames() {
    return Object.entries(TOOL_REGISTRY)
        .filter(([, tool]: [string, any]) => tool.chatAction === true)
        .map(([name]) => name);
}

function listCodeAgentActionNames() {
    return Object.entries(TOOL_REGISTRY)
        .filter(([, tool]: [string, any]) => tool.codeAgentOnly === true || tool.chatAction === true || tool.required)
        .map(([name]) => name);
}

function isEmptyToolValue(value) {
    if (value === undefined || value === null) return true;
    if (typeof value === 'string') return value.trim() === '';
    if (Array.isArray(value)) return value.length === 0;
    if (typeof value === 'object') return Object.keys(value).length === 0;
    return false;
}

function validateToolInput(action, input: any = {}) {
    const tool: any = getTool(action);
    if (!tool) {
        throw new Error(`Unsupported action: ${action}`);
    }

    const missing = (tool.required || []).filter(field => {
        if (!isEmptyToolValue(input[field])) return false;
        if (field === 'target' && (!isEmptyToolValue(input.path) || !isEmptyToolValue(input.query))) return false;
        if (field === 'query' && !isEmptyToolValue(input.target)) return false;
        return true;
    });
    if (missing.length > 0) {
        throw new Error(`Action "${action}" is missing required input field(s): ${missing.join(', ')}`);
    }

    if (action === 'apply_patch') {
        const patchInput = input.patch || {};
        if (!patchInput.path || !Array.isArray(patchInput.hunks) || patchInput.hunks.length === 0) {
            throw new Error('Action "apply_patch" requires input.patch.path and at least one hunk.');
        }
    }

    return tool;
}

function isImportantAction(action) {
    const tool = getTool(action);
    return !!(tool && tool.important);
}

function buildChatActionTypeUnion() {
    return ['none', ...listChatActionNames()].filter((name, index, arr) => arr.indexOf(name) === index).map(name => `"${name}"`).join(' | ');
}

function buildToolPromptSection() {
    const lines = ['\n\nAVAILABLE BUILT-IN ACTIONS:'];
    for (const name of listChatActionNames()) {
        const tool = getTool(name);
        lines.push(`- ${name}: ${tool.description}`);
    }
    return lines.join('\n');
}

export { TOOL_REGISTRY,
    getTool,
    listToolNames,
    listChatActionNames,
    listCodeAgentActionNames,
    validateToolInput,
    isImportantAction,
    buildChatActionTypeUnion,
    buildToolPromptSection
 }
