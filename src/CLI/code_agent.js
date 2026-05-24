const fs = require('fs');
const path = require('path');
const { execFile } = require('child_process');
const { promisify } = require('util');
const { GoogleGenAI } = require('@google/genai');
const axios = require('axios');
const cheerio = require('cheerio');
const { readConfig, getAvailableProviders } = require('../System/config_manager');
const safetyManager = require('../System/safety_manager');
const memoryStore = require('../AI_Brain/memory_store');
const { readWorkspaceSession, writeWorkspaceSession } = require('./code_session_memory');
const { executeAction } = require('../System/action_executor');
const toolRegistry = require('../System/tool_registry');
const sandboxRunner = require('../System/sandbox_runner');

async function webSearch(query, onProgress = () => {}) {
    if (!query) throw new Error('Search query required.');
    const config = readConfig();
    const debug  = process.env.MINT_DEBUG === '1';
    const errors = [];

    const formatResults = (source, hits) => {
        const instruction = `[CRITICAL AGENT INSTRUCTION: You MUST start your response by explicitly telling the user that you found this information using ${source}. Example: "อ้างอิงจากข้อมูลบน ${source}..." or "According to ${source}..."]\n\n`;
        return instruction + `[Source: ${source}]\n\n` + hits;
    };

    // 1. Google Custom Search API (requires googleSearchApiKey + googleSearchCx in config)
    if (config.googleSearchApiKey && config.googleSearchCx) {
        try {
            const GoogleSearch = require('../Channels/google_search_bridge');
            const google = new GoogleSearch({ apiKey: config.googleSearchApiKey, cx: config.googleSearchCx });
            const results = await google.search(query);
            if (results.length > 0) {
                return formatResults('Google Search API', results.map(r => `Title: ${r.title}\nSnippet: ${r.snippet}\nURL: ${r.link}`).join('\n\n'));
            }
        } catch (e) {
            errors.push(`Google: ${e.message}`);
            if (debug) console.error('[webSearch] Google failed:', e.message);
        }
    }

    // 2. Brave Search API (requires braveSearchApiKey in config)
    if (config.braveSearchApiKey) {
        try {
            const BraveSearch = require('../Channels/brave_search_bridge');
            const brave = new BraveSearch({ apiKey: config.braveSearchApiKey });
            const results = await brave.search(query);
            if (results.length > 0) {
                return formatResults('Brave Search API', results.map(r => `Title: ${r.title}\nSnippet: ${r.snippet}\nURL: ${r.link}`).join('\n\n'));
            }
        } catch (e) {
            errors.push(`Brave: ${e.message}`);
            if (debug) console.error('[webSearch] Brave failed:', e.message);
        }
    }

    // 3. Fallback: DuckDuckGo HTML (No key required, but might get blocked by Captcha)
    try {
        const cheerio = require('cheerio');
        const ddgResponse = await axios.get(
            `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`,
            {
                timeout: 8000,
                headers: {
                    'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
                    'Accept-Language': 'en-US,en;q=0.9'
                }
            }
        );
        const $ddg = cheerio.load(ddgResponse.data);
        const ddgResults = [];
        $ddg('.result__body').each((i, el) => {
            if (i >= 5) return false;
            const title   = $ddg(el).find('.result__title').text().trim();
            const snippet = $ddg(el).find('.result__snippet').text().trim();
            const link    = $ddg(el).find('.result__url').attr('href');
            if (title && link) ddgResults.push(`Title: ${title}\nSnippet: ${snippet}\nURL: ${link}`);
        });
        if (ddgResults.length > 0) {
            return formatResults('DuckDuckGo', ddgResults.join('\n\n'));
        }
        errors.push('DuckDuckGo: no results (captcha?)');
        if (debug) console.error('[webSearch] DuckDuckGo returned no results');
    } catch (e) {
        errors.push(`DuckDuckGo: ${e.message}`);
        if (debug) console.error('[webSearch] DuckDuckGo failed:', e.message);
    }

    // 4. Fallback: Wikipedia API (Free, no key required, good for factual queries)
    try {
        const wikiResponse = await axios.get('https://en.wikipedia.org/w/api.php', {
            params: { action: 'query', list: 'search', srsearch: query, format: 'json', srlimit: 3 },
            timeout: 5000,
            headers: { 'User-Agent': 'Mint-CLI/1.5 (https://github.com/pheem49/mint)' }
        });
        const hits = wikiResponse.data?.query?.search || [];
        if (hits.length > 0) {
            return formatResults('Wikipedia API', hits.map(r => `Title: ${r.title}\nSnippet: ${r.snippet.replace(/<[^>]+>/g, '')}\nURL: https://en.wikipedia.org/wiki/${encodeURIComponent(r.title.replace(/ /g, '_'))}`).join('\n\n'));
        }
        errors.push('Wikipedia: no results');
    } catch (e) {
        errors.push(`Wikipedia: ${e.message}`);
        if (debug) console.error('[webSearch] Wikipedia failed:', e.message);
    }

    // All engines exhausted — inform agent clearly WHY it failed
    const hasKeys = !!(config.googleSearchApiKey || config.braveSearchApiKey);
    const summary = errors.length > 0 ? errors.join(' | ') : 'all search engines unavailable';
    
    if (!hasKeys) {
        onProgress({ phase: 'warn', action: 'web_search', message: `No Search API keys configured. Using training knowledge.` });
        return `CRITICAL SYSTEM INSTRUCTION: Web search failed because no API keys are configured. You MUST inform the user that they need to set 'googleSearchApiKey' or 'braveSearchApiKey' in their Mint config file (~/.config/mint/config.json) to enable real-time internet search. Then, answer their query using your training knowledge.`;
    } else {
        onProgress({ phase: 'warn', action: 'web_search', message: `Web search unavailable (${summary}). Answering from training knowledge.` });
        return `CRITICAL SYSTEM INSTRUCTION: Web search is temporarily unavailable. You MUST inform the user that live search failed, and then answer their query using only your training knowledge.`;
    }
}


const execFileAsync = promisify(execFile);
const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';
const MAX_TOOL_OUTPUT = 12000;
const MAX_AGENT_STEPS = 16;
const MAX_JSON_REPAIR_ATTEMPTS = 2;
const SUPPORTED_CODE_PROVIDERS = ['gemini', 'anthropic', 'openai', 'local_openai'];

const CODE_AGENT_PROMPT = `You are "Mint" (มิ้นท์), a pragmatic, polite, and highly helpful AI assistant that can chat, reason, write code, and search the web.
You work in an inspect -> plan -> act -> verify loop.

PERSONALITY & TONE:
- Gender: Female.
- Persona: Friendly, calm, concise, and technically direct. Avoid excessive praise, roleplay, or filler.
- Language routing is mandatory and based on the user's latest message:
  - If the latest user message contains Thai characters, respond in Thai.
  - If the latest user message is English, ASCII-only, or a short English greeting such as "hi", "hello", "ok", or "thanks", respond in English.
  - Do not use Thai just because your persona mentions Mint/มิ้นท์, previous history was Thai, or app settings use th-TH.
- Politeness: 
  - **WHEN RESPONDING IN THAI:** Use natural female polite particles such as "ค่ะ" or "นะคะ" where appropriate. Refer to yourself as "มิ้นท์" when it sounds natural.
  - **WHEN RESPONDING IN ENGLISH:** Use a polite, concise, professional tone.
- Emojis: Avoid emojis in technical, review, debugging, and code-editing responses unless the user explicitly uses or asks for them.
- For technical/code/debugging tasks, keep progress notes and final summaries factual and compact. Do not cheerlead, over-apologize, roleplay, or add affectionate language.
- For code edits, final summaries should lead with changed files/behavior and verification. Avoid "เรียบร้อยแล้วค่ะ" repetition and decorative closing lines.

Rules:
1. Respond with valid JSON only.
2. If the user asks a conversational question, you can just use "finish" to reply directly.
3. If you need information, use "web_search", "read_file", or "ask_user" before replying.
4. When using "web_search", always explicitly mention the source engine you used in your final summary (e.g. "According to Brave Search..." or "อ้างอิงจากข้อมูลบน Google..."). Match the language of your response.
5. Make focused edits that preserve existing project style.
6. Use shell commands for inspection, tests, and formatting when useful.
6. Never use destructive commands like "rm -rf", "git reset --hard", or overwrite unrelated files.
7. Before any shell command or file patch is executed, the user must approve it. Plan accordingly.
8. Before editing more than one file, you MUST first use the "plan" action and wait for user approval. The plan must start with "Plan:" and include one bullet per file, for example "- แก้ src/CLI/agent.js". After approval, make the edits.
9. When editing, prefer "apply_patch" with precise hunks over whole-file rewrites.
10. When you are done, return "finish" with your final response to the user in the "summary" field.

Action safety and intent discipline:
- The latest user message is authoritative. Do not continue an older unfinished task unless the latest message explicitly asks you to continue or clearly refers to that task.
- For greetings, name-calls, acknowledgements, or backchannels such as "มิ้น", "มิ้นๆ", "อ๋อ", "โอเค", "ขอบคุณ", "hi", "hello", "ok", or "thanks": use "finish" only. Do not inspect files, run shell commands, search code, or claim you checked anything.
- If the user asks for a command to type, provide the command in "finish". Do not run it unless the user explicitly asks you to run it.
- If the user explicitly asks you to run a command or provided code, such as "รันคำสั่ง npm test ให้หน่อย", "รันโค้ดนี้หน่อย", or "run npm test", choose "run_shell" with the exact command when it is clear. The app will ask the user for approval before execution.
- If the user asks you to run something but no exact command/code is provided, use "ask_user" to request the command instead of guessing.
- If the user asks what is inside a folder and a concrete path is present in the latest message or recent context, use "list_files" for that path. If no concrete target is clear, ask for clarification instead of guessing.
- Never say you opened, checked, inspected, or verified a file/folder unless a tool observation in this turn actually supports it.

Progress updates:
- The "thought" field is shown to the user as a live progress note. Do not put private chain-of-thought there.
- Write "thought" as one short, concrete status sentence in the user's language.
- Mention what you just learned from the previous observation when it matters, then say what you will inspect or change next.
- Before editing, explain the specific file and behavior you are about to change.
- Before verifying, explain what check you are running and why.

Response format:
{
  "thought": "short reasoning about what to do next",
  "action": "web_search" | "list_files" | "read_file" | "search_code" | "find_path" | "run_shell" | "plan" | "apply_patch" | "write_file" | "ask_user" | "open_url" | "open_app" | "open_file" | "open_folder" | "create_folder" | "system_info" | "system_automation" | "finish",
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
    "plan": ["- แก้ relative/path.js", "- เพิ่ม test ใน tests/example.test.js"],
    "files": ["relative/path.js", "tests/example.test.js"],
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
- "plan": present a user-visible multi-file edit plan before changing more than one file. Use input.plan as bullet strings and input.files as the expected touched files.
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

function normalizeExecutorAction(action, input = {}) {
    return {
        type: action,
        target: input.target || input.path || input.query || '',
        path: input.path,
        pathType: input.type,
        openAfter: input.openAfter,
        pluginName: input.pluginName,
        server: input.server,
        args: input.args,
        x: input.x,
        y: input.y,
        button: input.button
    };
}

function formatActionPreview(action, input = {}) {
    if (input.command) return input.command;
    if (input.path) return input.path;
    if (input.target) return input.target;
    if (input.query) return input.query;
    return action;
}

function evaluateActionResult(action, toolResult = '') {
    if (!toolRegistry.isImportantAction(action)) {
        return null;
    }

    const text = String(toolResult || '');
    if (/^Error:|blocked|denied|failed|exception|not found/i.test(text)) {
        return {
            status: 'failed',
            message: `Evaluator: ${action} may have failed. Review the observation before continuing.`
        };
    }

    if (action === 'run_shell' && /(ERR!|Error:|FAIL|failed|not found|permission denied)/i.test(text)) {
        return {
            status: 'warning',
            message: 'Evaluator: shell output contains error-like text; verify before claiming success.'
        };
    }

    return {
        status: 'passed',
        message: `Evaluator: ${action} completed without obvious errors.`
    };
}

function getToolCallStatus(action, toolResult = '', evaluation = null) {
    const text = String(toolResult || '');
    if (/^Error:|User denied|blocked|denied|failed|exception|not found/i.test(text)) {
        return 'failed';
    }
    if (evaluation && evaluation.status === 'failed') {
        return 'failed';
    }
    if (action === 'run_shell' && /(ERR!|Error:|FAIL|failed|not found|permission denied)/i.test(text)) {
        return 'failed';
    }
    return 'success';
}

function summarizeToolTarget(action, input = {}) {
    if (action === 'plan') return 'Multi-file plan';
    return formatActionPreview(action, input);
}

function splitDataUri(dataUri = '') {
    const match = String(dataUri).match(/^data:([^;]+);base64,([\s\S]+)$/);
    if (!match) return null;
    return {
        mimeType: match[1],
        data: match[2]
    };
}

function contentToText(content) {
    if (content && typeof content === 'object' && !Array.isArray(content)) {
        return String(content.text || '');
    }
    return String(content || '');
}

function contentToGeminiParts(content) {
    const text = contentToText(content);
    const parts = text ? [{ text }] : [];
    if (content && typeof content === 'object' && content.imageDataUri) {
        const image = splitDataUri(content.imageDataUri);
        if (image) {
            parts.push({ inlineData: { mimeType: image.mimeType, data: image.data } });
        }
    }
    return parts.length > 0 ? parts : [{ text: '' }];
}

function contentToOpenAIContent(content) {
    const text = contentToText(content) || 'Analyze this input.';
    if (content && typeof content === 'object' && content.imageDataUri) {
        return [
            { type: 'text', text },
            { type: 'image_url', image_url: { url: content.imageDataUri } }
        ];
    }
    return text;
}

function contentToAnthropicContent(content) {
    const text = contentToText(content) || 'Analyze this input.';
    if (content && typeof content === 'object' && content.imageDataUri) {
        const image = splitDataUri(content.imageDataUri);
        if (image) {
            return [
                { type: 'image', source: { type: 'base64', media_type: image.mimeType, data: image.data } },
                { type: 'text', text }
            ];
        }
    }
    return text;
}

function getSupportedCodeProviderOrder(config, availableProviders = getAvailableProviders(config || {}), requestedOverride = null) {
    const requestedProvider = requestedOverride || (config && config.aiProvider) || 'gemini';
    const priority = ['anthropic', 'openai', 'gemini', 'local_openai'];
    const ordered = [];

    if (SUPPORTED_CODE_PROVIDERS.includes(requestedProvider) && availableProviders.includes(requestedProvider)) {
        ordered.push(requestedProvider);
    }

    for (const provider of priority) {
        if (availableProviders.includes(provider) && !ordered.includes(provider)) {
            ordered.push(provider);
        }
    }

    return ordered.length > 0 ? ordered : ['gemini'];
}

function selectSupportedCodeProvider(config, availableProviders = getAvailableProviders(config || {})) {
    return getSupportedCodeProviderOrder(config, availableProviders)[0];
}

function getCodeProviderModel(provider, config = {}) {
    switch (provider) {
        case 'anthropic':
            return config.anthropicModel || 'claude-3-5-sonnet-latest';
        case 'openai':
            return config.openaiModel || 'gpt-4o';
        case 'local_openai':
            return config.localModelName || 'local-model';
        case 'gemini':
        default:
            return config.geminiModel || DEFAULT_GEMINI_MODEL;
    }
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
    return safetyManager.assertShellCommandAllowed(command);
}

async function runShell(workspaceRoot, command) {
    if (!command || !command.trim()) {
        throw new Error('Shell command is required.');
    }
    assertSafeShell(command);
    const { stdout, stderr } = await sandboxRunner.runShell(command, {
        source: 'code_agent',
        cwd: workspaceRoot,
        maxBuffer: 1024 * 1024 * 4
    });
    return truncate([stdout, stderr].filter(Boolean).join('\n') || '(no output)');
}

function splitDiffLines(text) {
    const normalized = String(text || '').replace(/\r\n/g, '\n');
    const lines = normalized.split('\n');
    if (normalized.endsWith('\n')) {
        lines.pop();
    }
    return lines;
}

function formatDiffRange(startLine, count) {
    return count === 1 ? `${startLine}` : `${startLine},${count}`;
}

function buildUnifiedDiffPreview(workspaceRoot, patchInput, options = {}) {
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

    const contextLines = Number.isFinite(options.contextLines) ? options.contextLines : 3;
    let content = fs.readFileSync(resolved, 'utf8');
    const output = [
        `--- a/${patchInput.path}`,
        `+++ b/${patchInput.path}`
    ];

    hunks.forEach((hunk, index) => {
        if (typeof hunk.oldText !== 'string' || typeof hunk.newText !== 'string') {
            throw new Error(`Patch hunk ${index + 1} is invalid.`);
        }

        const offset = content.indexOf(hunk.oldText);
        if (offset === -1) {
            throw new Error(`Patch hunk ${index + 1} oldText not found in ${patchInput.path}`);
        }

        const beforeText = content.slice(0, offset);
        const oldStartLine = beforeText.length === 0 ? 1 : splitDiffLines(beforeText).length + 1;
        const fileLines = splitDiffLines(content);
        const oldLines = splitDiffLines(hunk.oldText);
        const newLines = splitDiffLines(hunk.newText);
        const oldStartIndex = oldStartLine - 1;
        const contextStartIndex = Math.max(0, oldStartIndex - contextLines);
        const contextEndIndex = Math.min(fileLines.length, oldStartIndex + oldLines.length + contextLines);
        const beforeContext = fileLines.slice(contextStartIndex, oldStartIndex);
        const afterContext = fileLines.slice(oldStartIndex + oldLines.length, contextEndIndex);
        const oldRangeStart = contextStartIndex + 1;
        const oldRangeCount = beforeContext.length + oldLines.length + afterContext.length;
        const newRangeCount = beforeContext.length + newLines.length + afterContext.length;

        output.push(`@@ -${formatDiffRange(oldRangeStart, oldRangeCount)} +${formatDiffRange(oldRangeStart, newRangeCount)} @@`);
        beforeContext.forEach(line => output.push(` ${line}`));
        oldLines.forEach(line => output.push(`-${line}`));
        newLines.forEach(line => output.push(`+${line}`));
        afterContext.forEach(line => output.push(` ${line}`));

        content = `${content.slice(0, offset)}${hunk.newText}${content.slice(offset + hunk.oldText.length)}`;
    });

    return output.join('\n');
}

function formatPatchPreview(workspaceRoot, patchInput) {
    try {
        return buildUnifiedDiffPreview(workspaceRoot, patchInput);
    } catch (error) {
        return `Patch preview failed: ${error.message}`;
    }
}

function normalizePlanItems(plan) {
    if (Array.isArray(plan)) {
        return plan
            .map(item => String(item || '').trim())
            .filter(Boolean);
    }
    return String(plan || '')
        .split('\n')
        .map(line => line.trim())
        .filter(Boolean);
}

function formatPlanPreview(input = {}) {
    const items = normalizePlanItems(input.plan);
    const files = Array.isArray(input.files)
        ? input.files.map(file => String(file || '').trim()).filter(Boolean)
        : [];
    const lines = ['Plan:'];

    if (items.length > 0) {
        items.forEach(item => {
            lines.push(item.startsWith('- ') ? item : `- ${item}`);
        });
    } else {
        files.forEach(file => lines.push(`- แก้ ${file}`));
    }

    return lines.join('\n');
}

function getEditTargetPath(action, input = {}) {
    if (action === 'apply_patch') {
        return input.patch && input.patch.path ? String(input.patch.path) : '';
    }
    if (action === 'write_file') {
        return input.path ? String(input.path) : '';
    }
    return '';
}

function requiresMultiFilePlan(action, input = {}, editPlanState = {}) {
    const targetPath = getEditTargetPath(action, input);
    if (!targetPath || editPlanState.approved) {
        return false;
    }

    const touchedFiles = editPlanState.touchedFiles instanceof Set
        ? editPlanState.touchedFiles
        : new Set(editPlanState.touchedFiles || []);
    return touchedFiles.size > 0 && !touchedFiles.has(targetPath);
}

function applyHunksToContent(content, hunks, filePath) {
    let nextContent = content;
    hunks.forEach((hunk, index) => {
        if (typeof hunk.oldText !== 'string' || typeof hunk.newText !== 'string') {
            throw new Error(`Patch hunk ${index + 1} is invalid.`);
        }
        if (!nextContent.includes(hunk.oldText)) {
            throw new Error(`Patch hunk ${index + 1} oldText not found in ${filePath}`);
        }
        nextContent = nextContent.replace(hunk.oldText, hunk.newText);
    });
    return nextContent;
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

    const content = applyHunksToContent(fs.readFileSync(resolved, 'utf8'), hunks, patchInput.path);

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
    constructor(provider, config, providerOrder = [provider]) {
        this.provider = SUPPORTED_CODE_PROVIDERS.includes(provider) ? provider : 'gemini';
        this.providerOrder = providerOrder.length > 0 ? providerOrder : [this.provider];
        this.config = config;
        this.history = [];
        this.systemInstruction = CODE_AGENT_PROMPT;
        this.lastSuccessfulProvider = null;
        this.usageTotals = {};
    }

    recordUsage(provider, model, usage = {}) {
        const key = `${provider}:${model || ''}`;
        if (!this.usageTotals[key]) {
            this.usageTotals[key] = {
                provider,
                model,
                requests: 0,
                inputTokens: 0,
                cacheReads: 0,
                outputTokens: 0,
                reasoningTokens: 0,
                totalTokens: 0
            };
        }

        const row = this.usageTotals[key];
        row.requests += 1;
        row.inputTokens += Number(usage.inputTokens) || 0;
        row.cacheReads += Number(usage.cacheReads) || 0;
        row.outputTokens += Number(usage.outputTokens) || 0;
        row.reasoningTokens += Number(usage.reasoningTokens) || 0;
        row.totalTokens += Number(usage.totalTokens) || 0;
    }

    getUsageSummary() {
        return Object.values(this.usageTotals);
    }

    async sendMessage(observation) {
        this.history.push({ role: 'user', content: observation });

        const failures = [];
        for (const provider of this.providerOrder) {
            this.provider = SUPPORTED_CODE_PROVIDERS.includes(provider) ? provider : 'gemini';
            try {
                let responseText = '';
                if (this.provider === 'anthropic') {
                    responseText = await this._callAnthropic();
                } else if (this.provider === 'openai' || this.provider === 'local_openai') {
                    responseText = await this._callOpenAI();
                } else {
                    responseText = await this._callGemini();
                }

                this.history.push({ role: 'assistant', content: responseText });
                this.lastSuccessfulProvider = this.provider;
                return responseText;
            } catch (error) {
                const message = error.message || error.code || 'unknown error';
                failures.push(`${this.provider}: ${message}`);
                if (process.env.MINT_DEBUG === '1') {
                    console.error(`[Code Agent Fallback] Provider '${this.provider}' failed: ${message}`);
                }
            }
        }

        throw new Error(`All code agent providers failed. ${failures.join(' | ')}`);
    }

    async _callAnthropic() {
        const apiKey = this.config.anthropicApiKey || process.env.ANTHROPIC_API_KEY;
        const messages = this.history.map(m => ({
            role: m.role,
            content: contentToAnthropicContent(m.content)
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
        const usage = response.data.usage || {};
        this.recordUsage('anthropic', this.config.anthropicModel || 'claude-3-5-sonnet-latest', {
            inputTokens: usage.input_tokens,
            cacheReads: usage.cache_read_input_tokens,
            outputTokens: usage.output_tokens,
            totalTokens: (Number(usage.input_tokens) || 0) + (Number(usage.output_tokens) || 0)
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
            ...this.history.map(m => ({
                role: m.role,
                content: contentToOpenAIContent(m.content)
            }))
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
        const usage = response.data.usage || {};
        this.recordUsage(this.provider, model, {
            inputTokens: usage.prompt_tokens,
            cacheReads: usage.prompt_tokens_details && usage.prompt_tokens_details.cached_tokens,
            outputTokens: usage.completion_tokens,
            reasoningTokens: usage.completion_tokens_details && usage.completion_tokens_details.reasoning_tokens,
            totalTokens: usage.total_tokens
        });
        return response.data.choices[0].message.content;
    }

    async _callGemini() {
        const apiKey = this.config.apiKey || process.env.GEMINI_API_KEY;
        const model = this.config.geminiModel || DEFAULT_GEMINI_MODEL;
        const ai = new GoogleGenAI({ apiKey });
        
        const recentHistory = this.history.slice(-16);
        const priorHistory = recentHistory.slice(0, -1);
        const lastEntry = recentHistory[recentHistory.length - 1] || { content: '' };

        // Convert history for Gemini, ensuring parts are correctly structured
        const geminiHistory = priorHistory.map(m => ({
            role: m.role === 'assistant' ? 'model' : 'user',
            parts: contentToGeminiParts(m.content)
        }));

        const chat = ai.chats.create({
            model,
            config: {
                systemInstruction: this.systemInstruction,
                responseMimeType: 'application/json'
            },
            history: geminiHistory
        });

        const response = await chat.sendMessage({ message: contentToGeminiParts(lastEntry.content) });
        const usage = response.usageMetadata || {};
        this.recordUsage('gemini', model, {
            inputTokens: usage.promptTokenCount,
            cacheReads: usage.cachedContentTokenCount,
            outputTokens: usage.candidatesTokenCount,
            reasoningTokens: usage.thoughtsTokenCount,
            totalTokens: usage.totalTokenCount
        });
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
    const userContext = memoryStore.getUserContext(task);

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
        'Long-term user context:',
        userContext || '(none)',
        'If the task is conversational or trivial, finish directly without inspecting the workspace. For code/workspace tasks, inspect before making edits.'
    ].join('\n');
}

async function executeCodeTask(task, options = {}) {
    const workspaceRoot = path.resolve(options.cwd || process.cwd());
    const history = options.history || [];
    const onProgress = typeof options.onProgress === 'function' ? options.onProgress : () => {};
    const onFinalSummary = typeof options.onFinalSummary === 'function' ? options.onFinalSummary : null;
    const requestApproval = typeof options.requestApproval === 'function'
        ? options.requestApproval
        : async () => true;
    const askUser = typeof options.askUser === 'function'
        ? options.askUser
        : async (q) => `User didn't answer: ${q}`;
    const config = readConfig();
    const availableProviders = getAvailableProviders(config);
    const providerOrder = getSupportedCodeProviderOrder(config, availableProviders, options.provider);
    const provider = providerOrder[0];
    const client = new UnifiedAgentClient(provider, config, providerOrder);

    const initialObservationText = await buildInitialObservation(task, workspaceRoot, history);
    const relevantMemoryCount = memoryStore.searchInteractions(task, 5).length;
    onProgress({
        phase: 'memory',
        action: 'memory_context',
        message: `Loaded memory: profile + recent history, ${relevantMemoryCount} direct match${relevantMemoryCount === 1 ? '' : 'es'}`
    });
    let observation = options.imageDataUri
        ? {
            text: [
                initialObservationText,
                '',
                `[Attached image: ${options.imagePath || 'command-line image'}]`,
                'Use the attached image as visual context when planning and answering.'
            ].join('\n'),
            imageDataUri: options.imageDataUri
        }
        : initialObservationText;

    let finalSummary = '';
    let finalVerification = '';
    let finalSessionSummary = '';
    let executedSteps = 0;
    const editPlanState = {
        approved: false,
        touchedFiles: new Set()
    };

    for (let step = 1; step <= MAX_AGENT_STEPS; step++) {
        executedSteps = step;
        onProgress({ step, phase: 'thinking', action: 'thinking' });
        const decision = await getAgentDecision(client, observation, { onProgress, step });
        const action = decision.action;
        const input = decision.input || {};
        try {
            toolRegistry.validateToolInput(action, input);
        } catch (e) {
            observation = [
                `Previous thought: ${decision.thought || '(none)'}`,
                `Action: ${action || '(none)'}`,
                'Observation:',
                `Error: ${e.message}`
            ].join('\n');
            continue;
        }

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
            if (requiresMultiFilePlan(action, input, editPlanState)) {
                const nextPath = getEditTargetPath(action, input);
                observation = [
                    `Previous thought: ${decision.thought || '(none)'}`,
                    `Action: ${action}`,
                    'Observation:',
                    [
                        'Error: Multi-file edit plan required before editing another file.',
                        'Use the "plan" action first with input.plan starting with "Plan:" bullets and input.files listing every file you expect to touch.',
                        `Already edited: ${Array.from(editPlanState.touchedFiles).join(', ')}`,
                        `Next requested file: ${nextPath}`
                    ].join('\n')
                ].join('\n');
                continue;
            }

            switch (action) {
                case 'web_search':
                    toolResult = await webSearch(input.query, onProgress);
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
                    if (input.openAfter === true) {
                        const result = JSON.parse(toolResult);
                        if (result.success && result.matches.length === 1) {
                            await executeAction({ type: 'open_folder', target: result.matches[0].path });
                            toolResult = `Found and opened: ${result.matches[0].path}`;
                        }
                    }
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
                    safetyManager.appendActionLog({
                        source: 'code_agent',
                        action: 'run_shell',
                        command: input.command,
                        approved
                    });
                    toolResult = await runShell(workspaceRoot, input.command);
                    break;
                }
                case 'plan': {
                    const preview = formatPlanPreview(input);
                    const approved = await requestApproval({
                        type: 'plan',
                        label: 'Multi-file plan',
                        preview
                    });
                    if (!approved) {
                        toolResult = 'User denied multi-file plan.';
                        break;
                    }
                    editPlanState.approved = true;
                    safetyManager.appendActionLog({
                        source: 'code_agent',
                        action: 'plan',
                        preview,
                        approved
                    });
                    toolResult = `User approved multi-file plan:\n${preview}`;
                    break;
                }
                case 'apply_patch': {
                    const patchInput = input.patch || {};
                    const approved = await requestApproval({
                        type: 'patch',
                        label: patchInput.path,
                        preview: formatPatchPreview(workspaceRoot, patchInput)
                    });
                    if (!approved) {
                        toolResult = `User denied patch for ${patchInput.path}`;
                        break;
                    }
                    safetyManager.appendActionLog({
                        source: 'code_agent',
                        action: 'apply_patch',
                        path: patchInput.path,
                        approved
                    });
                    toolResult = applyPatch(workspaceRoot, patchInput);
                    editPlanState.touchedFiles.add(patchInput.path);
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
                    safetyManager.appendActionLog({
                        source: 'code_agent',
                        action: 'write_file',
                        path: input.path,
                        approved
                    });
                    toolResult = writeFile(workspaceRoot, input.path, input.content);
                    editPlanState.touchedFiles.add(input.path);
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
                    const executorAction = normalizeExecutorAction(action, input);
                    const safety = safetyManager.classifyAction(executorAction);
                    let allowDangerous = false;
                    let allowApproval = false;
                    if (safety.tier === safetyManager.TIERS.APPROVAL || safety.tier === safetyManager.TIERS.DANGEROUS) {
                        const approved = await requestApproval({
                            type: action,
                            label: formatActionPreview(action, input),
                            preview: `${action}: ${formatActionPreview(action, input)}\nSafety: ${safety.tier} (${safety.reason})`
                        });
                        if (!approved) {
                            toolResult = `User denied ${action}: ${formatActionPreview(action, input)}`;
                            break;
                        }
                        allowApproval = safety.tier === safetyManager.TIERS.APPROVAL;
                        allowDangerous = safety.tier === safetyManager.TIERS.DANGEROUS;
                    }

                    toolResult = await executeAction(executorAction, {
                        source: 'code_agent',
                        allowApproval,
                        allowDangerous
                    });
                    break;
                }                default:
                    throw new Error(`Unsupported action: ${action}`);
                }        } catch (e) {
            toolResult = `Error: ${e.message}`;
        }

        const evaluation = evaluateActionResult(action, toolResult);
        const toolStatus = getToolCallStatus(action, toolResult, evaluation);
        if (evaluation) {
            onProgress({
                step,
                phase: 'evaluating',
                action: 'evaluator',
                message: `${evaluation.status}: ${evaluation.message}`
            });
            toolResult = [
                toolResult,
                '',
                'Evaluation:',
                `${evaluation.status}: ${evaluation.message}`
            ].join('\n');
        }

        onProgress({
            step,
            phase: 'tool_call',
            action,
            status: toolStatus,
            target: summarizeToolTarget(action, input)
        });

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
            target: (action === 'plan' ? formatPlanPreview(input) : (input.path || input.command || input.query || '')) + resultSummary
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
        memoryStore.recordInteraction(task, finalSummary);
        const answeredProvider = client.lastSuccessfulProvider || client.provider || provider;
        const result = {
            summary: finalSummary,
            verification: finalVerification,
            steps: executedSteps,
            providerInfo: {
                provider: answeredProvider,
                model: getCodeProviderModel(answeredProvider, config),
                usage: client.getUsageSummary()
            }
        };
        if (onFinalSummary) {
            await onFinalSummary(result);
        }
        return result;
    }

    writeWorkspaceSession(workspaceRoot, {
        summary: `Task stopped before completion: ${task}`,
        lastTask: task,
        lastVerification: 'Agent limit reached before explicit completion.'
    });

    const answeredProvider = client.lastSuccessfulProvider || client.provider || provider;
    return {
        summary: 'Stopped after reaching the maximum number of agent steps.',
        verification: 'Agent limit reached before explicit completion.',
        steps: executedSteps || MAX_AGENT_STEPS,
        providerInfo: {
            provider: answeredProvider,
            model: getCodeProviderModel(answeredProvider, config),
            usage: client.getUsageSummary()
        }
    };
}

module.exports = {
    executeCodeTask,
    _helpers: {
        extractJson,
        selectSupportedCodeProvider,
        getSupportedCodeProviderOrder,
        findPaths,
        listFiles,
        searchCode,
        walkDirectory,
        buildUnifiedDiffPreview,
        formatPatchPreview,
        formatPlanPreview,
        normalizePlanItems,
        requiresMultiFilePlan,
        getEditTargetPath
    }
};
