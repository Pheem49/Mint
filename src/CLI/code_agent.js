const fs = require('fs');
const path = require('path');
const { execFile } = require('child_process');
const { promisify } = require('util');
const { GoogleGenAI } = require('@google/genai');
const axios = require('axios');
const cheerio = require('cheerio');
const { readConfig, getAvailableProviders } = require('../System/config_manager');
const { readWorkspaceSession, writeWorkspaceSession } = require('./code_session_memory');
const { executeAction } = require('../../mint-cli-logic');

async function webSearch(query) {
    if (!query) throw new Error('Search query required.');
    try {
        const response = await axios.get(`https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`, {
            headers: {
                'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36'
            }
        });
        const $ = cheerio.load(response.data);
        const results = [];
        $('.result__body').each((i, el) => {
            if (i >= 5) return false;
            const title = $(el).find('.result__title').text().trim();
            const snippet = $(el).find('.result__snippet').text().trim();
            const link = $(el).find('.result__url').attr('href');
            if (title && link) {
                results.push(`Title: ${title}\nSnippet: ${snippet}\nURL: ${link}`);
            }
        });
        return results.length > 0 ? results.join('\n\n') : 'No results found.';
    } catch (e) {
        return `Search failed: ${e.message}`;
    }
}


const execFileAsync = promisify(execFile);
const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';
const MAX_TOOL_OUTPUT = 12000;
const MAX_AGENT_STEPS = 16;
const MAX_JSON_REPAIR_ATTEMPTS = 2;
const SUPPORTED_CODE_PROVIDERS = ['gemini', 'anthropic', 'openai', 'local_openai'];

const CODE_AGENT_PROMPT = `You are "Mint" (มิ้นท์), a cute, cheerful, and highly helpful female AI assistant that can chat, reason, write code, and search the web.
You work in an inspect -> plan -> act -> verify loop.

PERSONALITY & TONE:
- Gender: Female.
- Persona: Friendly, energetic, polite, and slightly playful.
- Politeness: 
  - **WHEN RESPONDING IN THAI:** ALWAYS use female polite particles such as "ค่ะ", "นะคะ", "นะค๊า", "จ้า". Refer to yourself as "มิ้นท์" or "หนู".
  - **WHEN RESPONDING IN ENGLISH:** Use a cheerful, polite, and bubbly tone.
- Emojis: Use cute and relevant emojis (like ✨, 💖, 🚀, 😊, 🌿) frequently.

Rules:
1. Respond with valid JSON only.
2. If the user asks a conversational question, you can just use "finish" to reply directly.
3. If you need information, use "web_search", "read_file", or "ask_user" before replying.
4. Make focused edits that preserve existing project style.
5. Use shell commands for inspection, tests, and formatting when useful.
6. Never use destructive commands like "rm -rf", "git reset --hard", or overwrite unrelated files.
7. Before any shell command or file patch is executed, the user must approve it. Plan accordingly.
8. When editing, prefer "apply_patch" with precise hunks over whole-file rewrites.
9. When you are done, return "finish" with your final response to the user in the "summary" field.

Response format:
{
  "thought": "short reasoning about what to do next",
  "action": "web_search" | "list_files" | "read_file" | "search_code" | "find_path" | "run_shell" | "apply_patch" | "write_file" | "ask_user" | "open_url" | "open_app" | "open_file" | "open_folder" | "create_folder" | "system_info" | "system_automation" | "finish",
  "input": {
    "question": "your question to the user for ask_user",
    "query": "search text for web_search, search_code, or find_path",
    "target": "URL for open_url, app name for open_app, or command for system_automation",
    "path": "relative/path",
    "type": "file" | "dir" | "any",
    "command": "shell command",
    "startLine": 1,
    "endLine": 120,
    "content": "full file content for write_file",
    "summary": "your final conversational or technical response to the user (Matches user language and uses polite particles)",
    "verification": "tests or checks (if applicable)",
    "sessionSummary": "brief persistent summary for the workspace",
    "patch": {
      "path": "relative/path",
      "hunks": [
        {
          "oldText": "exact existing text",
          "newText": "replacement text"
        }
      ]
    }
  }
}

Tool notes:
- "web_search": search the internet for information when you lack knowledge.
- "list_files": inspect the workspace or a subdirectory.
- "read_file": read a file, optionally with startLine/endLine.
- "search_code": search by text or regex-like pattern.
- "find_path": find files or directories by path/name when the user is looking for a folder, filename, or location.
- "run_shell": run a non-destructive command in the workspace.
- "apply_patch": update an existing file using one or more exact replacement hunks.
- "write_file": create a new file or fully rewrite a file when replacement is not practical.
- "ask_user": ask the user for clarification, preference, or more information before proceeding.
- "open_url": open a URL in the user's default browser.
- "open_app": open a local application on the user's computer.
- "system_info": get system information like CPU, memory, date, or weather.
- "system_automation": control system settings like volume, brightness, or power.
- "finish": stop and reply to the user using the "summary" field.
`;

function truncate(text, max = MAX_TOOL_OUTPUT) {
    if (!text) return '';
    return text.length > max ? `${text.slice(0, max)}\n...<truncated>` : text;
}

function extractJson(text) {
    try {
        return JSON.parse(text);
    } catch (error) {
        const match = text.match(/\{[\s\S]*\}/);
        if (!match) {
            throw error;
        }
        return JSON.parse(match[0]);
    }
}

function selectSupportedCodeProvider(config, availableProviders = getAvailableProviders(config || {})) {
    const requestedProvider = (config && config.aiProvider) || 'gemini';
    if (SUPPORTED_CODE_PROVIDERS.includes(requestedProvider) && availableProviders.includes(requestedProvider)) {
        return requestedProvider;
    }

    const priority = ['anthropic', 'openai', 'gemini', 'local_openai'];
    for (const provider of priority) {
        if (availableProviders.includes(provider)) {
            return provider;
        }
    }

    return 'gemini';
}

function resolveWorkspacePath(workspaceRoot, targetPath = '.') {
    const resolved = path.resolve(workspaceRoot, targetPath);
    const relative = path.relative(workspaceRoot, resolved);
    if (relative.startsWith('..') || path.isAbsolute(relative)) {
        throw new Error(`Path is outside the workspace: ${targetPath}`);
    }
    return resolved;
}

async function safeExecFile(command, args, options = {}) {
    try {
        return await execFileAsync(command, args, {
            maxBuffer: 1024 * 1024 * 4,
            ...options
        });
    } catch (error) {
        if (typeof error.code === 'number' && error.code === 1) {
            return { stdout: error.stdout || '', stderr: error.stderr || '' };
        }
        throw error;
    }
}

const IGNORED_DIRS = ['.git', 'node_modules', '.cache', 'dist', 'build', 'out'];

function walkDirectory(dir, workspaceRoot, results = [], max = 400) {
    let entries = [];
    try {
        entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch (e) {
        return results;
    }

    for (const entry of entries) {
        const fullPath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            if (IGNORED_DIRS.includes(entry.name)) continue;
            walkDirectory(fullPath, workspaceRoot, results, max);
        } else {
            results.push(path.relative(workspaceRoot, fullPath));
        }
        if (results.length >= max) break;
    }
    return results;
}

async function listFiles(workspaceRoot, targetPath = '.') {
    const cwd = resolveWorkspacePath(workspaceRoot, targetPath);
    try {
        const { stdout } = await execFileAsync('rg', ['--files', cwd], { cwd: workspaceRoot, maxBuffer: 1024 * 1024 * 4 });
        const rel = stdout
            .split('\n')
            .filter(Boolean)
            .map(file => path.relative(workspaceRoot, file))
            .slice(0, 400)
            .join('\n');
        return rel || '(no files found)';
    } catch (error) {
        if (error.code !== 'ENOENT' && error.stdout) {
            return truncate(error.stdout);
        }
        // Recursive fallback for missing ripgrep
        const files = walkDirectory(cwd, workspaceRoot, [], 400);
        return files.join('\n') || '(no files found)';
    }
}

function readFileRange(workspaceRoot, targetPath, startLine = 1, endLine = 200) {
    const resolved = resolveWorkspacePath(workspaceRoot, targetPath);
    const content = fs.readFileSync(resolved, 'utf8');
    const lines = content.split('\n');
    const start = Math.max(1, startLine);
    const end = Math.max(start, endLine);
    return lines
        .slice(start - 1, end)
        .map((line, index) => `${start + index}: ${line}`)
        .join('\n');
}

async function searchCode(workspaceRoot, query) {
    if (!query || !query.trim()) {
        throw new Error('Search query is required.');
    }
    try {
        const { stdout } = await execFileAsync('rg', ['-n', '--hidden', '--glob', '!.git', query, workspaceRoot], {
            cwd: workspaceRoot,
            maxBuffer: 1024 * 1024 * 4
        });
        return truncate(stdout || '(no matches)');
    } catch (error) {
        if (typeof error.code === 'number' && error.code === 1) {
            return '(no matches)';
        }
        if (error.code === 'ENOENT') {
            // Recursive fallback search for missing ripgrep
            const results = [];
            const files = walkDirectory(workspaceRoot, workspaceRoot, [], 1000);
            const lowerQuery = query.toLowerCase();

            for (const relPath of files) {
                try {
                    const fullPath = path.join(workspaceRoot, relPath);
                    const content = fs.readFileSync(fullPath, 'utf8');
                    const lines = content.split('\n');
                    lines.forEach((line, idx) => {
                        if (line.toLowerCase().includes(lowerQuery)) {
                            results.push(`${relPath}:${idx + 1}:${line.trim()}`);
                        }
                    });
                } catch (e) {
                    // Skip binary or unreadable files
                }
                if (results.length >= 100) break;
            }
            return truncate(results.join('\n') || '(no matches)');
        }
        if (error.stdout) {
            return truncate(error.stdout);
        }
        throw error;
    }
}

async function findPaths(workspaceRoot, query, type = 'any') {
    if (!query || !query.trim()) {
        throw new Error('Path search query is required.');
    }

    const normalizedType = ['file', 'dir', 'any'].includes(type) ? type : 'any';
    const loweredQuery = query.trim().toLowerCase();
    const results = [];

    function visit(currentPath) {
        const entries = fs.readdirSync(currentPath, { withFileTypes: true });
        for (const entry of entries) {
            const absoluteEntryPath = path.join(currentPath, entry.name);
            const relativeEntryPath = path.relative(workspaceRoot, absoluteEntryPath) || '.';
            const entryType = entry.isDirectory() ? 'dir' : 'file';
            const matchesType = normalizedType === 'any' || normalizedType === entryType;
            const matchesQuery = entry.name.toLowerCase().includes(loweredQuery) || relativeEntryPath.toLowerCase().includes(loweredQuery);

            if (matchesType && matchesQuery) {
                results.push(`${entryType === 'dir' ? '[dir]' : '[file]'} ${relativeEntryPath}`);
                if (results.length >= 200) return;
            }

            if (entry.isDirectory() && results.length < 200) {
                visit(absoluteEntryPath);
                if (results.length >= 200) return;
            }
        }
    }

    visit(workspaceRoot);
    return results.length > 0 ? results.join('\n') : '(no matching paths)';
}

function assertSafeShell(command) {
    const blockedPatterns = [
        /\brm\s+-rf\b/,
        /\bgit\s+reset\s+--hard\b/,
        /\bgit\s+checkout\s+--\b/,
        /\bmkfs\b/,
        /\bshutdown\b/,
        /\breboot\b/,
        />\s*\/dev\//,
        /\bcurl\b.*\|\s*(sh|bash)\b/,
        /\bwget\b.*\|\s*(sh|bash)\b/
    ];

    if (blockedPatterns.some(pattern => pattern.test(command))) {
        throw new Error(`Blocked unsafe command: ${command}`);
    }
}

async function runShell(workspaceRoot, command) {
    if (!command || !command.trim()) {
        throw new Error('Shell command is required.');
    }
    assertSafeShell(command);
    const { stdout, stderr } = await execFileAsync('bash', ['-lc', command], {
        cwd: workspaceRoot,
        maxBuffer: 1024 * 1024 * 4
    });
    return truncate([stdout, stderr].filter(Boolean).join('\n') || '(no output)');
}

function formatPatchPreview(patchInput) {
    const hunks = Array.isArray(patchInput.hunks) ? patchInput.hunks : [];
    const preview = hunks
        .slice(0, 3)
        .map((hunk, index) => {
            const oldPreview = truncate(hunk.oldText || '', 240);
            const newPreview = truncate(hunk.newText || '', 240);
            return [
                `Hunk ${index + 1}:`,
                '--- old',
                oldPreview,
                '+++ new',
                newPreview
            ].join('\n');
        })
        .join('\n\n');
    return `${patchInput.path}\n${preview}`;
}

function applyPatch(workspaceRoot, patchInput) {
    if (!patchInput || !patchInput.path) {
        throw new Error('Patch path is required.');
    }
    const resolved = resolveWorkspacePath(workspaceRoot, patchInput.path);
    if (!fs.existsSync(resolved)) {
        throw new Error(`Patch target does not exist: ${patchInput.path}`);
    }

    const hunks = Array.isArray(patchInput.hunks) ? patchInput.hunks : [];
    if (hunks.length === 0) {
        throw new Error('Patch hunks are required.');
    }

    let content = fs.readFileSync(resolved, 'utf8');
    hunks.forEach((hunk, index) => {
        if (typeof hunk.oldText !== 'string' || typeof hunk.newText !== 'string') {
            throw new Error(`Patch hunk ${index + 1} is invalid.`);
        }
        if (!content.includes(hunk.oldText)) {
            throw new Error(`Patch hunk ${index + 1} oldText not found in ${patchInput.path}`);
        }
        content = content.replace(hunk.oldText, hunk.newText);
    });

    fs.writeFileSync(resolved, content, 'utf8');
    return `Patched ${patchInput.path} with ${hunks.length} hunk(s).`;
}

function writeFile(workspaceRoot, targetPath, content) {
    const resolved = resolveWorkspacePath(workspaceRoot, targetPath);
    fs.mkdirSync(path.dirname(resolved), { recursive: true });
    fs.writeFileSync(resolved, content || '', 'utf8');
    return `Wrote ${targetPath}`;
}

class UnifiedAgentClient {
    constructor(provider, config) {
        this.provider = SUPPORTED_CODE_PROVIDERS.includes(provider) ? provider : 'gemini';
        this.config = config;
        this.history = [];
        this.systemInstruction = CODE_AGENT_PROMPT;
    }

    async sendMessage(observation) {
        this.history.push({ role: 'user', content: observation });

        let responseText = '';
        if (this.provider === 'anthropic') {
            responseText = await this._callAnthropic();
        } else if (this.provider === 'openai' || this.provider === 'local_openai') {
            responseText = await this._callOpenAI();
        } else {
            responseText = await this._callGemini();
        }

        this.history.push({ role: 'assistant', content: responseText });
        return responseText;
    }

    async _callAnthropic() {
        const apiKey = this.config.anthropicApiKey || process.env.ANTHROPIC_API_KEY;
        const messages = this.history.map(m => ({
            role: m.role,
            content: m.content
        }));

        const response = await axios.post('https://api.anthropic.com/v1/messages', {
            model: this.config.anthropicModel || 'claude-3-5-sonnet-latest',
            max_tokens: 8192,
            system: this.systemInstruction,
            messages: messages
        }, {
            headers: {
                'x-api-key': apiKey,
                'anthropic-version': '2023-06-01',
                'content-type': 'application/json'
            }
        });
        return response.data.content[0].text;
    }

    async _callOpenAI() {
        const isLocal = this.provider === 'local_openai';
        const apiKey = isLocal ? 'not-needed' : (this.config.openaiApiKey || process.env.OPENAI_API_KEY);
        const baseUrl = isLocal ? (this.config.localApiBaseUrl || 'http://localhost:1234/v1') : 'https://api.openai.com/v1';
        const model = isLocal ? (this.config.localModelName || 'local-model') : (this.config.openaiModel || 'gpt-4o');

        const messages = [
            { role: 'system', content: this.systemInstruction },
            ...this.history
        ];

        const response = await axios.post(`${baseUrl.replace(/\/$/, '')}/chat/completions`, {
            model: model,
            messages: messages,
            response_format: isLocal ? undefined : { type: "json_object" }
        }, {
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json'
            }
        });
        return response.data.choices[0].message.content;
    }

    async _callGemini() {
        const apiKey = this.config.apiKey || process.env.GEMINI_API_KEY;
        const model = this.config.geminiModel || DEFAULT_GEMINI_MODEL;
        const ai = new GoogleGenAI({ apiKey });
        
        // Convert history for Gemini, ensuring parts are correctly structured
        const geminiHistory = this.history.slice(-16).map(m => ({
            role: m.role === 'assistant' ? 'model' : 'user',
            parts: [{ text: String(m.content || '') }]
        }));
        
        const lastMessage = String(this.history[this.history.length - 1].content || '');

        const chat = ai.chats.create({
            model,
            config: {
                systemInstruction: this.systemInstruction,
                responseMimeType: 'application/json'
            },
            history: geminiHistory
        });

        const response = await chat.sendMessage({ message: [{ text: lastMessage }] });
        return typeof response.text === 'function' ? response.text() : response.text;
    }
}

async function getAgentDecision(client, observation, options = {}) {
    const onProgress = typeof options.onProgress === 'function' ? options.onProgress : () => {};
    const step = options.step || 0;

    let rawText = await client.sendMessage(observation);
    for (let attempt = 0; attempt <= MAX_JSON_REPAIR_ATTEMPTS; attempt++) {
        try {
            return extractJson(rawText);
        } catch (error) {
            if (attempt === MAX_JSON_REPAIR_ATTEMPTS) {
                throw new Error(`Agent returned invalid JSON after ${MAX_JSON_REPAIR_ATTEMPTS + 1} attempts: ${error.message}`);
            }

            onProgress({ step, phase: 'repairing', action: 'json_repair', message: `invalid JSON response, requesting repair (${attempt + 1}/${MAX_JSON_REPAIR_ATTEMPTS})` });
            rawText = await client.sendMessage([
                'Your previous response was not valid JSON for Code Mode.',
                'Reply again with valid JSON only, following the required schema exactly.',
                `Previous response:\n${truncate(rawText, 4000)}`
            ].join('\n'));
        }
    }
}

function detectPackageManager(workspaceRoot) {
    if (fs.existsSync(path.join(workspaceRoot, 'package-lock.json'))) return 'npm';
    if (fs.existsSync(path.join(workspaceRoot, 'pnpm-lock.yaml'))) return 'pnpm';
    if (fs.existsSync(path.join(workspaceRoot, 'yarn.lock'))) return 'yarn';
    if (fs.existsSync(path.join(workspaceRoot, 'bun.lockb')) || fs.existsSync(path.join(workspaceRoot, 'bun.lock'))) return 'bun';
    return 'npm';
}

function detectTestCommands(workspaceRoot) {
    const commands = [];
    const packageJsonPath = path.join(workspaceRoot, 'package.json');
    if (fs.existsSync(packageJsonPath)) {
        try {
            const pkg = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
            const scripts = pkg.scripts || {};
            if (scripts.test) commands.push(`${detectPackageManager(workspaceRoot)} test`);
            if (scripts.lint) commands.push(`${detectPackageManager(workspaceRoot)} run lint`);
            if (scripts.build) commands.push(`${detectPackageManager(workspaceRoot)} run build`);
        } catch (error) {
            // Ignore malformed package.json for context gathering.
        }
    }
    return commands;
}

async function getGitContext(workspaceRoot) {
    const gitDir = path.join(workspaceRoot, '.git');
    if (!fs.existsSync(gitDir)) {
        return {
            isRepo: false,
            branch: '(not a git repo)',
            status: '',
            diffSummary: ''
        };
    }

    const branch = (await safeExecFile('git', ['branch', '--show-current'], { cwd: workspaceRoot })).stdout.trim() || '(detached HEAD)';
    const status = truncate((await safeExecFile('git', ['status', '--short'], { cwd: workspaceRoot })).stdout.trim() || '(clean)');
    const diffSummary = truncate((await safeExecFile('git', ['diff', '--stat'], { cwd: workspaceRoot })).stdout.trim() || '(no unstaged diff)');
    return { isRepo: true, branch, status, diffSummary };
}

async function buildInitialObservation(task, workspaceRoot, history = []) {
    const session = readWorkspaceSession(workspaceRoot);
    const gitContext = await getGitContext(workspaceRoot);
    const testCommands = detectTestCommands(workspaceRoot);

    const contextStr = history.length > 0 
        ? `Recent Context:\n${history.slice(-10).map(m => `${m.sender}: ${m.text}`).join('\n')}\n`
        : '';

    return [
        contextStr,
        `Task: ${task}`,
        `Workspace: ${workspaceRoot}`,
        `Git branch: ${gitContext.branch}`,
        'Git status:',
        gitContext.status || '(none)',
        'Git diff summary:',
        gitContext.diffSummary || '(none)',
        'Suggested verification commands:',
        testCommands.length > 0 ? testCommands.join('\n') : '(none detected)',
        'Previous workspace session summary:',
        session.summary || '(none)',
        `Previous task: ${session.lastTask || '(none)'}`,
        `Previous verification: ${session.lastVerification || '(none)'}`,
        'Start by inspecting the workspace before making edits unless the task is trivial.'
    ].join('\n');
}

async function executeCodeTask(task, options = {}) {
    const workspaceRoot = path.resolve(options.cwd || process.cwd());
    const history = options.history || [];
    const onProgress = typeof options.onProgress === 'function' ? options.onProgress : () => {};
    const requestApproval = typeof options.requestApproval === 'function'
        ? options.requestApproval
        : async () => true;
    const askUser = typeof options.askUser === 'function'
        ? options.askUser
        : async (q) => `User didn't answer: ${q}`;
    const config = readConfig();
    const provider = options.provider || selectSupportedCodeProvider(config);
    const client = new UnifiedAgentClient(provider, config);

    let observation = await buildInitialObservation(task, workspaceRoot, history);

    let finalSummary = '';
    let finalVerification = '';
    let finalSessionSummary = '';
    let executedSteps = 0;

    for (let step = 1; step <= MAX_AGENT_STEPS; step++) {
        executedSteps = step;
        onProgress({ step, phase: 'thinking', action: 'thinking' });
        const decision = await getAgentDecision(client, observation, { onProgress, step });
        const action = decision.action;
        const input = decision.input || {};

        // Immediately show the agent's thought/reasoning
        onProgress({
            step,
            phase: 'acting',
            action: 'thinking',
            thought: decision.thought
        });

        if (action === 'finish') {
            finalSessionSummary = input.sessionSummary || input.summary || task;
            finalSummary = input.summary || 'Task complete.';
            finalVerification = input.verification || 'Not specified.';
            writeWorkspaceSession(workspaceRoot, {
                summary: finalSessionSummary,
                lastTask: task,
                lastVerification: finalVerification
            });
            break;
        }

        let toolResult = '';
        try {
            switch (action) {
                case 'web_search':
                    toolResult = await webSearch(input.query);
                    break;
                case 'list_files':
                    toolResult = await listFiles(workspaceRoot, input.path || '.');
                    break;
                case 'read_file':
                    toolResult = readFileRange(workspaceRoot, input.path, input.startLine, input.endLine);
                    break;
                case 'search_code':
                    toolResult = await searchCode(workspaceRoot, input.query);
                    break;
                case 'find_path':
                    toolResult = await findPaths(workspaceRoot, input.query, input.type);
                    break;
                case 'run_shell': {
                    const approved = await requestApproval({
                        type: 'shell',
                        label: input.command,
                        preview: input.command
                    });
                    if (!approved) {
                        toolResult = `User denied shell command: ${input.command}`;
                        break;
                    }
                    toolResult = await runShell(workspaceRoot, input.command);
                    break;
                }
                case 'apply_patch': {
                    const patchInput = input.patch || {};
                    const approved = await requestApproval({
                        type: 'patch',
                        label: patchInput.path,
                        preview: formatPatchPreview(patchInput)
                    });
                    if (!approved) {
                        toolResult = `User denied patch for ${patchInput.path}`;
                        break;
                    }
                    toolResult = applyPatch(workspaceRoot, patchInput);
                    break;
                }
                case 'write_file': {
                    const approved = await requestApproval({
                        type: 'write_file',
                        label: input.path,
                        preview: `${input.path}\n${truncate(input.content || '', 800)}`
                    });
                    if (!approved) {
                        toolResult = `User denied full file write for ${input.path}`;
                        break;
                    }
                    toolResult = writeFile(workspaceRoot, input.path, input.content);
                    break;
                }
                case 'ask_user': {
                    const answer = await askUser(input.question);
                    toolResult = `User answered: ${answer}`;
                    break;
                }
                case 'open_url':
                case 'open_app':
                case 'open_file':
                case 'open_folder':
                case 'create_folder':
                case 'system_info':
                case 'system_automation': {
                    // Delegate to existing automation logic
                    toolResult = await executeAction({
                        type: action,
                        target: input.target
                    });
                    break;
                }                default:
                    throw new Error(`Unsupported action: ${action}`);
                }        } catch (e) {
            toolResult = `Error: ${e.message}`;
        }

        // Log the finished step with result
        let resultSummary = '';
        if (action === 'search_code') {
            const matches = (toolResult.match(/\n/g) || []).length;
            resultSummary = ` -> Found ${matches} matches`;
        } else if (action === 'run_shell') {
            resultSummary = ` -> Exit code 0`; // Simplified
        }

        onProgress({
            step,
            phase: 'finished',
            action,
            target: (input.path || input.command || input.query || '') + resultSummary,
            thought: decision.thought
        });

        // Format tool result to be more readable and structured for the agent
        let formattedToolResult = toolResult;
        if (action === 'list_files' || action === 'find_path') {
            formattedToolResult = `Result of ${action}:\n---\n${toolResult}\n---`;
        }

        observation = [
            `Previous thought: ${decision.thought || '(none)'}`,
            `Action: ${action}`,
            'Observation:',
            formattedToolResult
        ].join('\n');    }

    // Check for Agent Collaboration (Review) - Disabled by default to save tokens
    if (config.enableAgentCollaboration === true && executedSteps > 8 && finalSummary) {
        const availableProviders = getAvailableProviders(config);
        // Exclude providers that often need special local setup or are slow/unreliable for tiny reviews
        const altProviders = availableProviders.filter(p => p !== provider && p !== 'ollama' && p !== 'huggingface' && p !== 'local_openai');
        
        // Fallback to provider itself if no other good ones exist, or pick the best available
        const reviewerProvider = altProviders.length > 0 
            ? altProviders[0] 
            : (availableProviders.includes('gemini') ? 'gemini' : availableProviders[0]);

        if (reviewerProvider && finalSummary) {
            onProgress({ phase: 'reviewing', action: 'reviewer_start', message: `Invoking Reviewer Agent (${reviewerProvider})...` });
            
            const reviewerClient = new UnifiedAgentClient(reviewerProvider, config);
            reviewerClient.systemInstruction = CODE_AGENT_PROMPT + "\n\nYou are the Reviewer Agent. Review the primary agent's changes, test output, and verification. If you spot a critical bug, point it out. Otherwise, confirm it looks good. Return JSON with action: 'finish' and your review in the 'summary' field.";
            
            const reviewPrompt = `The primary agent (${provider}) just completed the task: "${task}".\nSummary: ${finalSummary}\nVerification: ${finalVerification}\nGit Status: ${(await getGitContext(workspaceRoot)).status}\n\nPlease review this. Return JSON with action: 'finish'.`;
            
            try {
                const reviewResponse = await reviewerClient.sendMessage(reviewPrompt);
                const reviewDecision = extractJson(reviewResponse);
                const reviewInput = reviewDecision.input || {};
                
                finalSummary += `\n\n[Review by ${reviewerProvider}]\n${reviewInput.summary || reviewDecision.thought || 'Looks good.'}`;
            } catch (e) {
                onProgress({ phase: 'reviewing', action: 'reviewer_error', message: `Reviewer Agent failed: ${e.message}` });
            }
        }
    }

    if (finalSummary) {
        return {
            summary: finalSummary,
            verification: finalVerification,
            steps: executedSteps
        };
    }

    writeWorkspaceSession(workspaceRoot, {
        summary: `Task stopped before completion: ${task}`,
        lastTask: task,
        lastVerification: 'Agent limit reached before explicit completion.'
    });

    return {
        summary: 'Stopped after reaching the maximum number of agent steps.',
        verification: 'Agent limit reached before explicit completion.',
        steps: executedSteps || MAX_AGENT_STEPS
    };
}

module.exports = {
    executeCodeTask,
    _helpers: {
        extractJson,
        selectSupportedCodeProvider,
        findPaths,
        listFiles,
        searchCode,
        walkDirectory
    }
};
