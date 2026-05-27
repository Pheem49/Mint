const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFile, execFileSync } = require('child_process');
const { promisify } = require('util');
const axios = require('axios');
const cheerio = require('cheerio');
const { readConfig, getAvailableProviders, CONFIG_DIR } = require('../System/config_manager');
const safetyManager = require('../System/safety_manager');
const memoryStore = require('../AI_Brain/memory_store');
const { readWorkspaceSession, writeWorkspaceSession } = require('./code_session_memory');
const { executeAction } = require('../System/action_executor');
const toolRegistry = require('../System/tool_registry');
const sandboxRunner = require('../System/sandbox_runner');
const providerAdapter = require('../AI_Brain/provider_adapter');
const taskManager = require('../System/task_manager');

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
const MAX_TOOL_OUTPUT = 12000;
const MAX_AGENT_STEPS = 16;
const MAX_JSON_REPAIR_ATTEMPTS = 2;
const DEFAULT_VERIFICATION_BUDGET = 2;
const MINT_CONFIG_DIR = CONFIG_DIR || path.join(os.homedir(), '.config', 'mint');
const PLAN_FILE_PATH = path.join(MINT_CONFIG_DIR, 'mint_plan.md');
const PLAN_FILE_LABEL = path.join('~', '.config', 'mint', 'mint_plan.md');
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
  - This language routing applies to user-facing final answers and ask_user questions.
  - Internal progress notes, the JSON "thought" field, and "plan" action bullet text MUST be written in English.
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
8. Before editing more than one file, you MUST first use the "plan" action and wait for user approval. The plan must be written in English, start with "Plan:", and include one bullet per file, for example "- Update src/CLI/agent.js". After approval, make the edits.
9. When editing, prefer "apply_patch" with precise hunks over whole-file rewrites.
10. Before any "apply_patch" or "write_file" action, the "thought" field MUST explicitly name the file you will edit and why that file is the right target. If the file is under "scratch/" or "tests/fixtures/", call that out and explain why editing disposable/test fixture content is intentional.
11. When you are done, return "finish" with your final response to the user in the "summary" field.

Action safety and intent discipline:
- The latest user message is authoritative. Do not continue an older unfinished task unless the latest message explicitly asks you to continue or clearly refers to that task.
- For greetings, name-calls, acknowledgements, or backchannels such as "มิ้น", "มิ้นๆ", "อ๋อ", "โอเค", "ขอบคุณ", "hi", "hello", "ok", or "thanks": use "finish" only. Do not inspect files, run shell commands, search code, or claim you checked anything.
- If the user asks for a command to type, provide the command in "finish". Do not run it unless the user explicitly asks you to run it.
- If the user asks not to edit or says this is read-only analysis (for example "ห้ามแก้ไฟล์", "ไม่ต้องแก้", "แค่อ่าน", "แค่สรุป", "do not edit", "no edits", "read only"), do not use "plan", "apply_patch", "write_file", "create_folder", "delete_file", "clipboard_write", or system-changing actions. Inspect with read/search tools and finish with a summary only.
- If the user explicitly asks to search keywords, method names, class names, or symbols, use "search_code" before repeatedly reading more file ranges. Prefer a scoped search with input.path instead of scanning the whole workspace when the likely area is clear.
- Search scope heuristics: choose input.path only when that path is visible in the current workspace context or was named by the user. If the repo layout is unclear, use list_files on "." first, then choose the narrowest existing directory. Common scopes include "src", "app", "lib", "packages", "tests", and project-specific folders; in this Mint repo, CLI/terminal/command/approval/chat agent questions usually start in "src/CLI", desktop UI/renderer/settings/widget questions in "src/UI", system/config/safety questions in "src/System", and plugin questions in "src/Plugins". If a scoped search path is missing or finds no useful matches, search the whole workspace.
- If the user explicitly asks you to run a command or provided code, such as "รันคำสั่ง npm test ให้หน่อย", "รันโค้ดนี้หน่อย", or "run npm test", choose "run_shell" with the exact command when it is clear. The app will ask the user for approval before execution.
- If the user asks you to run something but no exact command/code is provided, use "ask_user" to request the command instead of guessing.
- If the user asks what is inside a folder and a concrete path is present in the latest message or recent context, use "list_files" for that path. If no concrete target is clear, ask for clarification instead of guessing.
- Never say you opened, checked, inspected, or verified a file/folder unless a tool observation in this turn actually supports it.

Progress updates:
- The "thought" field is shown to the user as a live progress note. Do not put private chain-of-thought there.
- Write "thought" as one short, concrete status sentence in English, even when the user writes in Thai or another language.
- Mention what you just learned from the previous observation when it matters, then say what you will inspect or change next.
- Before editing, explain the specific file and behavior you are about to change.
- Before verifying, explain what check you are running and why.

Response format:
{
  "thought": "short reasoning about what to do next",
  "action": "web_search" | "list_files" | "read_file" | "search_code" | "find_path" | "run_shell" | "verify" | "plan" | "apply_patch" | "write_file" | "ask_user" | "open_url" | "search" | "open_app" | "web_automation" | "open_file" | "open_folder" | "create_folder" | "delete_file" | "clipboard_write" | "learn_file" | "learn_folder" | "system_info" | "plugin" | "mcp_tool" | "mouse_move" | "mouse_click" | "type_text" | "key_tap" | "system_automation" | "finish",
  "input": {
    "question": "your question to the user for ask_user",
    "query": "search text for web_search, search_code, or find_path",
    "target": "URL for open_url, app name for open_app, or command for system_automation",
    "path": "relative/path",
    "type": "file" | "dir" | "any",
    "command": "shell command",
    "commands": ["npm test", "npm run build"],
    "startLine": 1,
    "endLine": 120,
    "content": "full file content for write_file",
    "plan": ["- Update relative/path.js", "- Add tests in tests/example.test.js"],
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
- "search_code": search by text or regex-like pattern. Optionally set input.path to a relative file or directory to avoid scanning the whole workspace; use the search scope heuristics above when the user did not name a path.
- "find_path": find files or directories by path/name when the user is looking for a folder, filename, or location.
- "run_shell": run a non-destructive command in the workspace.
- "verify": run the detected or provided test/build/lint commands. If verification fails, inspect the output, patch the issue, and verify again within the remaining budget.
- "plan": present a user-visible multi-file edit plan before changing more than one file. Use English input.plan bullet strings and input.files as the expected touched files.
- "apply_patch": update an existing file using one or more exact replacement hunks.
- "write_file": create a new file or fully rewrite a file when replacement is not practical.
- "ask_user": ask the user for clarification, preference, or more information before proceeding.
- "open_url": open a URL in the user's default browser.
- "open_app": open a local application on the user's computer.
- "system_info": get system information like CPU, memory, date, or weather.
- "system_automation": control system settings like volume, brightness, or power.
- "plugin": run a configured Mint plugin.
- "mcp_tool": call a configured MCP tool.
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
    if (action === 'search_code') {
        const query = input.query || 'search';
        return input.path ? `${query} in ${input.path}` : query;
    }
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

function getSupportedCodeProviderOrder(config, availableProviders = getAvailableProviders(config || {}), requestedOverride = null) {
    return providerAdapter.getProviderAttemptOrder(config || {}, {
        supported: SUPPORTED_CODE_PROVIDERS,
        availableProviders,
        requested: requestedOverride || (config && config.aiProvider) || 'gemini',
        priority: ['anthropic', 'openai', 'gemini', 'local_openai']
    });
}

function selectSupportedCodeProvider(config, availableProviders = getAvailableProviders(config || {})) {
    return getSupportedCodeProviderOrder(config, availableProviders)[0];
}

function getCodeProviderModel(provider, config = {}) {
    return providerAdapter.getProviderModel(provider, config);
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

async function searchCode(workspaceRoot, query, targetPath = '.') {
    if (!query || !query.trim()) {
        throw new Error('Search query is required.');
    }
    const searchRoot = resolveWorkspacePath(workspaceRoot, targetPath || '.');
    if (!fs.existsSync(searchRoot)) {
        throw new Error(`Search path does not exist: ${targetPath}`);
    }
    try {
        const { stdout } = await execFileAsync('rg', ['-n', '--hidden', '--glob', '!.git', query, searchRoot], {
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

async function runVerificationCommands(workspaceRoot, commands = [], options = {}) {
    const detected = detectTestCommands(workspaceRoot);
    const requested = Array.isArray(commands)
        ? commands.map(command => String(command || '').trim()).filter(Boolean)
        : [];
    const commandList = requested.length > 0 ? requested : detected;

    if (commandList.length === 0) {
        return {
            passed: true,
            output: 'No verification commands detected.'
        };
    }

    const requestApproval = typeof options.requestApproval === 'function'
        ? options.requestApproval
        : async () => true;
    const budget = Number.isFinite(options.budget) ? options.budget : DEFAULT_VERIFICATION_BUDGET;
    const attempt = Number.isFinite(options.attempt) ? options.attempt : 1;
    const lines = [
        `Verification attempt ${attempt}/${budget}`,
        `Commands: ${commandList.join(' && ')}`
    ];

    for (const command of commandList) {
        const approved = await requestApproval({
            type: 'verify',
            label: command,
            preview: command
        });
        if (!approved) {
            lines.push(`SKIP ${command}: User denied verification command.`);
            return {
                passed: false,
                output: lines.join('\n')
            };
        }

        try {
            const output = await runShell(workspaceRoot, command);
            lines.push(`PASS ${command}`);
            if (output && output !== '(no output)') {
                lines.push(truncate(output, 4000));
            }
        } catch (error) {
            lines.push(`FAIL ${command}`);
            lines.push(truncate([error.stdout, error.stderr, error.message].filter(Boolean).join('\n'), 6000));
            return {
                passed: false,
                output: lines.join('\n')
            };
        }
    }

    return {
        passed: true,
        output: lines.join('\n')
    };
}

function splitDiffLines(text) {
    const normalized = String(text || '').replace(/\r\n/g, '\n');
    const lines = normalized.split('\n');
    if (normalized.endsWith('\n')) {
        lines.pop();
    }
    return lines;
}

function normalizeGitNoIndexDiff(stdout, targetPath) {
    const lines = String(stdout || '').replace(/\r\n/g, '\n').split('\n');
    const filtered = [];
    for (const line of lines) {
        if (!line) continue;
        if (line.startsWith('diff --git ') || line.startsWith('index ')) continue;
        if (line.startsWith('--- ')) {
            filtered.push(`--- a/${targetPath}`);
            continue;
        }
        if (line.startsWith('+++ ')) {
            filtered.push(`+++ b/${targetPath}`);
            continue;
        }
        filtered.push(line);
    }
    return filtered.join('\n');
}

function buildSimpleFullFileDiff(targetPath, previousContent = '', nextContent = '') {
    const previousLines = splitDiffLines(previousContent);
    const nextLines = splitDiffLines(nextContent || '');
    const oldRange = previousLines.length || 0;
    const newRange = nextLines.length || 0;
    const output = [
        `--- a/${targetPath}`,
        `+++ b/${targetPath}`,
        `@@ -1,${oldRange} +1,${newRange} @@`
    ];

    previousLines.forEach(line => output.push(`-${line}`));
    nextLines.forEach(line => output.push(`+${line}`));
    return output.join('\n');
}

function buildContentDiffPreview(targetPath, previousContent = '', nextContent = '') {
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-diff-'));
    const oldPath = path.join(tempDir, 'old');
    const newPath = path.join(tempDir, 'new');

    try {
        fs.writeFileSync(oldPath, previousContent || '', 'utf8');
        fs.writeFileSync(newPath, nextContent || '', 'utf8');
        try {
            const stdout = execFileSync('git', ['diff', '--no-index', '--', oldPath, newPath], {
                encoding: 'utf8',
                maxBuffer: 1024 * 1024 * 4
            });
            return normalizeGitNoIndexDiff(stdout, targetPath);
        } catch (error) {
            const stdout = error.stdout || '';
            if (stdout) return normalizeGitNoIndexDiff(stdout, targetPath);
            return buildSimpleFullFileDiff(targetPath, previousContent, nextContent);
        }
    } finally {
        fs.rmSync(tempDir, { recursive: true, force: true });
    }
}

function buildPatchedContent(workspaceRoot, patchInput) {
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

    const previousContent = fs.readFileSync(resolved, 'utf8');
    return {
        previousContent,
        nextContent: applyHunksToContent(previousContent, hunks, patchInput.path)
    };
}

function buildUnifiedDiffPreview(workspaceRoot, patchInput, options = {}) {
    const { previousContent, nextContent } = buildPatchedContent(workspaceRoot, patchInput);
    return buildContentDiffPreview(patchInput.path, previousContent, nextContent);
}

function formatPatchPreview(workspaceRoot, patchInput) {
    try {
        return buildUnifiedDiffPreview(workspaceRoot, patchInput);
    } catch (error) {
        return `Patch preview failed: ${error.message}`;
    }
}

function buildFullFileDiffPreview(workspaceRoot, targetPath, nextContent = '') {
    if (!targetPath) {
        throw new Error('Write path is required.');
    }

    const resolved = resolveWorkspacePath(workspaceRoot, targetPath);
    const previousContent = fs.existsSync(resolved) ? fs.readFileSync(resolved, 'utf8') : '';
    return buildContentDiffPreview(targetPath, previousContent, nextContent || '');
}

function formatWritePreview(workspaceRoot, targetPath, content) {
    try {
        return buildFullFileDiffPreview(workspaceRoot, targetPath, content);
    } catch (error) {
        return `Write preview failed: ${error.message}\n${targetPath}\n${truncate(content || '', 800)}`;
    }
}

function normalizeRelativePathForWarning(targetPath = '') {
    return String(targetPath || '').replace(/\\/g, '/').replace(/^\.\/+/, '');
}

function contentLooksLikeGuide(text = '') {
    return /(guide|installation|publish|npm|registry|setup|documentation|คู่มือ|ติดตั้ง|เผยแพร่)/i.test(String(text || ''));
}

function contentLooksLikeBio(text = '') {
    return /(bio|biography|profile|created by|assistant|ประวัติ|โปรไฟล์)/i.test(String(text || ''));
}

function contentLooksLikeConfig(text = '') {
    return /(apiKey|token|secret|config|settings|\.env|clientSecret|refreshToken)/i.test(String(text || ''));
}

function buildApprovalWarnings(targetPath = '', nextContent = '') {
    const normalized = normalizeRelativePathForWarning(targetPath);
    const basename = path.basename(normalized).toLowerCase();
    const warnings = [];

    if (normalized.startsWith('scratch/')) {
        warnings.push('Target is under scratch/, which is usually disposable/test content. Confirm this is intentional.');
    }
    if (normalized.startsWith('tests/fixtures/') || normalized.includes('/tests/fixtures/')) {
        warnings.push('Target is under tests/fixtures/, so this may change test fixture behavior.');
    }
    if (/bio|profile|about/.test(basename) && contentLooksLikeGuide(nextContent)) {
        warnings.push('File name looks like profile/bio content, but the new content looks like a guide or publishing document.');
    }
    if (/(guide|readme|docs?|manual)/.test(basename) && contentLooksLikeBio(nextContent)) {
        warnings.push('File name looks like documentation, but the new content looks like biography/profile content.');
    }
    if (!/(config|settings|env|secret|token)/.test(basename) && contentLooksLikeConfig(nextContent)) {
        warnings.push('New content appears to include config/secret-like terms; verify this file is the right place.');
    }

    return warnings;
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

function normalizePlanItemLanguage(item) {
    let text = String(item || '').trim();
    const hasBullet = text.startsWith('- ');
    if (hasBullet) text = text.slice(2).trim();

    const replacements = [
        [/^แก้\s+(.+)$/i, 'Update $1'],
        [/^แก้ไข\s+(.+)$/i, 'Update $1'],
        [/^อัปเดต\s+(.+)$/i, 'Update $1'],
        [/^ปรับ\s+(.+)$/i, 'Update $1'],
        [/^สร้าง\s+(.+)$/i, 'Create $1'],
        [/^เพิ่ม\s+(.+)$/i, 'Add $1'],
        [/^ลบ\s+(.+)$/i, 'Remove $1'],
        [/^ตรวจสอบ\s+(.+)$/i, 'Verify $1'],
        [/^ทดสอบ\s+(.+)$/i, 'Test $1']
    ];

    for (const [pattern, replacement] of replacements) {
        if (pattern.test(text)) {
            text = text.replace(pattern, replacement);
            break;
        }
    }

    return hasBullet ? `- ${text}` : text;
}

function formatPlanPreview(input = {}) {
    const items = normalizePlanItems(input.plan);
    const files = Array.isArray(input.files)
        ? input.files.map(file => String(file || '').trim()).filter(Boolean)
        : [];
    const lines = ['Plan:'];

    if (items.length > 0) {
        items.forEach(item => {
            const normalizedItem = normalizePlanItemLanguage(item);
            lines.push(normalizedItem.startsWith('- ') ? normalizedItem : `- ${normalizedItem}`);
        });
    } else {
        files.forEach(file => lines.push(`- Update ${file}`));
    }

    return lines.join('\n');
}

function formatPlanApprovalSummary(input = {}) {
    const items = normalizePlanItems(input.plan);
    const files = Array.isArray(input.files)
        ? input.files.map(file => String(file || '').trim()).filter(Boolean)
        : [];
    if (files.length > 0) {
        return `${items.length || files.length} planned changes across ${files.length} files.`;
    }
    return `${items.length || 1} planned change${(items.length || 1) === 1 ? '' : 's'} prepared.`;
}

function formatPlanMarkdown(input = {}, context = {}) {
    const preview = formatPlanPreview(input);
    const files = Array.isArray(input.files)
        ? input.files.map(file => String(file || '').trim()).filter(Boolean)
        : [];
    const task = String(context.task || input.task || '').trim();
    const createdAt = context.createdAt || new Date().toISOString();
    const approvalStatus = context.approvalStatus || 'Pending user approval';
    const approvalTime = context.approvalTime || '';
    const lines = [
        '# Mint Plan',
        '',
        `Created: ${createdAt}`
    ];

    if (task) {
        lines.push('', '## Task', '', task);
    }

    lines.push('', '## Plan', '', preview);

    if (files.length > 0) {
        lines.push('', '## Expected Files', '');
        files.forEach(file => lines.push(`- ${file}`));
    }

    lines.push(
        '',
        '## Approval',
        '',
        `Status: ${approvalStatus}`
    );

    if (approvalTime) {
        lines.push(`${approvalStatus}: ${approvalTime}`);
    }

    lines.push('');

    return lines.join('\n');
}

function writePlanFile(workspaceRoot, input = {}, context = {}) {
    const planPath = context.planPath || PLAN_FILE_PATH;
    const content = formatPlanMarkdown(input, context);
    fs.mkdirSync(path.dirname(planPath), { recursive: true });
    fs.writeFileSync(planPath, content, 'utf8');
    return {
        path: planPath,
        content
    };
}

function updatePlanApprovalStatus(planFile, input = {}, context = {}) {
    const content = formatPlanMarkdown(input, context);
    fs.mkdirSync(path.dirname(planFile.path), { recursive: true });
    fs.writeFileSync(planFile.path, content, 'utf8');
    return {
        ...planFile,
        content
    };
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

function getMissingPlanFiles(editPlanState = {}) {
    const expectedFiles = editPlanState.expectedFiles instanceof Set
        ? editPlanState.expectedFiles
        : new Set(editPlanState.expectedFiles || []);
    const touchedFiles = editPlanState.touchedFiles instanceof Set
        ? editPlanState.touchedFiles
        : new Set(editPlanState.touchedFiles || []);

    return Array.from(expectedFiles).filter(file => file && !touchedFiles.has(file));
}

function isReadOnlyTask(task = '') {
    const text = String(task || '').toLowerCase();
    return /(?:ห้ามแก้|ไม่ต้องแก้|อย่าแก้|ไม่แก้ไฟล์|ห้ามเขียน|แค่อ่าน|อ่านอย่างเดียว|แค่สรุป|สรุปอย่างเดียว|แค่อธิบาย|อธิบายอย่างเดียว|do not edit|don't edit|no edits?|read[-\s]?only|only read|only summarize|summari[sz]e only|do not modify|don't modify|no changes?|analysis only)/i.test(text);
}

function isWriteLikeAction(action) {
    return new Set([
        'plan',
        'apply_patch',
        'write_file',
        'create_folder',
        'delete_file',
        'clipboard_write',
        'system_automation',
        'mouse_move',
        'mouse_click',
        'type_text',
        'key_tap'
    ]).has(action);
}

function validateEditExplanation(action, input = {}, thought = '') {
    const targetPath = getEditTargetPath(action, input);
    if (!targetPath) return { ok: true };

    const text = String(thought || '').toLowerCase();
    const normalized = normalizeRelativePathForWarning(targetPath).toLowerCase();
    const basename = path.basename(normalized).toLowerCase();
    const mentionsTarget = text.includes(normalized) || (basename && text.includes(basename));
    const explainsWhy = /(because|why|so that|in order|to update|to change|to edit|เพื่อ|เพราะ|เนื่องจาก|จะปรับ|จะแก้|อัปเดต|แก้ไข)/i.test(thought || '');
    if (!mentionsTarget || !explainsWhy) {
        return {
            ok: false,
            message: `Before editing ${targetPath}, explain in the thought field which file you will edit and why this is the correct target.`
        };
    }

    const sensitiveScratchPath = normalized.startsWith('scratch/') ||
        normalized.startsWith('tests/fixtures/') ||
        normalized.includes('/tests/fixtures/');
    const mentionsSensitiveLocation = /(scratch|fixture|test fixture|tests\/fixtures|ทดลอง|fixture)/i.test(thought || '');
    const marksIntentional = /(intentional|intentionally|disposable|test content|test fixture|ตั้งใจ|ชั่วคราว|เนื้อหาทดลอง|ไฟล์ทดสอบ)/i.test(thought || '');
    if (sensitiveScratchPath && !(mentionsSensitiveLocation && marksIntentional)) {
        return {
            ok: false,
            message: `Before editing ${targetPath}, explicitly mention that it is under scratch/ or tests/fixtures/ and why editing that disposable/test fixture content is intentional.`
        };
    }

    return { ok: true };
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
    const client = new providerAdapter.AgentProviderClient({
        provider,
        config,
        providerOrder,
        systemInstruction: CODE_AGENT_PROMPT,
        responseMimeType: 'application/json',
        maxTokens: 8192
    });

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
    const readOnlyTask = isReadOnlyTask(task);
    const editPlanState = {
        approved: false,
        touchedFiles: new Set(),
        expectedFiles: new Set()
    };
    let verificationAttempts = 0;
    const verificationBudget = Number.isFinite(options.verificationBudget)
        ? options.verificationBudget
        : DEFAULT_VERIFICATION_BUDGET;

    if (options.taskId) {
        taskManager.addCheckpoint(options.taskId, {
            phase: 'code_agent_start',
            message: task,
            provider,
            providerOrder
        });
    }

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

        if (action === 'finish') {
            const missingPlanFiles = getMissingPlanFiles(editPlanState);
            if (missingPlanFiles.length > 0) {
                observation = [
                    `Previous thought: ${decision.thought || '(none)'}`,
                    'Action: finish',
                    'Observation:',
                    [
                        'Error: Approved plan is not complete yet.',
                        `Missing planned file edits: ${missingPlanFiles.join(', ')}`,
                        'Complete every file listed in the approved plan before finishing, or create a new plan if the scope changed.'
                    ].join('\n')
                ].join('\n');
                continue;
            }
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
            if (readOnlyTask && isWriteLikeAction(action)) {
                observation = [
                    `Previous thought: ${decision.thought || '(none)'}`,
                    `Action: ${action}`,
                    'Observation:',
                    [
                        'Error: The latest user request is read-only and explicitly forbids edits or changes.',
                        'Do not create a plan or request approval for edits.',
                        'Use read_file/search_code/find_path as needed, then finish with an analysis summary.'
                    ].join('\n')
                ].join('\n');
                continue;
            }

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

            if (action === 'apply_patch' || action === 'write_file') {
                const explanation = validateEditExplanation(action, input, decision.thought);
                if (!explanation.ok) {
                    observation = [
                        `Previous thought: ${decision.thought || '(none)'}`,
                        `Action: ${action}`,
                        'Observation:',
                        `Error: ${explanation.message}`
                    ].join('\n');
                    continue;
                }
            }

            // Show progress only after the action passes local validation, so retry attempts do not spam near-duplicate notes.
            onProgress({
                step,
                phase: 'acting',
                action: 'thinking',
                thought: decision.thought
            });

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
                    toolResult = await searchCode(workspaceRoot, input.query, input.path || '.');
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
                case 'verify': {
                    verificationAttempts += 1;
                    const result = await runVerificationCommands(workspaceRoot, input.commands, {
                        requestApproval,
                        budget: verificationBudget,
                        attempt: verificationAttempts
                    });
                    toolResult = result.output;
                    if (options.taskId) {
                        taskManager.addCheckpoint(options.taskId, {
                            phase: 'verification',
                            attempt: verificationAttempts,
                            passed: result.passed,
                            output: truncate(result.output, 4000)
                        });
                    }
                    if (!result.passed && verificationAttempts >= verificationBudget) {
                        toolResult += '\nVerification budget exhausted. Finish with the remaining failure clearly explained.';
                    }
                    break;
                }
                case 'plan': {
                    const createdAt = new Date().toISOString();
                    let planFile = writePlanFile(workspaceRoot, input, { task, createdAt });
                    const approved = await requestApproval({
                        type: 'plan',
                        label: PLAN_FILE_LABEL,
                        preview: planFile.content,
                        summary: formatPlanApprovalSummary(input),
                        openPath: planFile.path
                    });
                    if (!approved) {
                        planFile = updatePlanApprovalStatus(planFile, input, {
                            task,
                            createdAt,
                            approvalStatus: 'Denied',
                            approvalTime: new Date().toISOString()
                        });
                        toolResult = 'User denied multi-file plan.';
                        break;
                    }
                    planFile = updatePlanApprovalStatus(planFile, input, {
                        task,
                        createdAt,
                        approvalStatus: 'Approved',
                        approvalTime: new Date().toISOString()
                    });
                    editPlanState.approved = true;
                    editPlanState.expectedFiles = new Set(
                        Array.isArray(input.files)
                            ? input.files.map(file => String(file || '').trim()).filter(Boolean)
                            : []
                    );
                    safetyManager.appendActionLog({
                        source: 'code_agent',
                        action: 'plan',
                        path: planFile.path,
                        preview: planFile.content,
                        approved
                    });
                    toolResult = `User approved multi-file plan at ${PLAN_FILE_LABEL}:\n${planFile.content}`;
                    break;
                }
                case 'apply_patch': {
                    const patchInput = input.patch || {};
                    let patchWarnings = [];
                    try {
                        patchWarnings = buildApprovalWarnings(
                            patchInput.path,
                            buildPatchedContent(workspaceRoot, patchInput).nextContent
                        );
                    } catch (_) {
                        patchWarnings = buildApprovalWarnings(patchInput.path, '');
                    }
                    const approved = await requestApproval({
                        type: 'patch',
                        label: patchInput.path,
                        preview: formatPatchPreview(workspaceRoot, patchInput),
                        warnings: patchWarnings
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
                        preview: formatWritePreview(workspaceRoot, input.path, input.content),
                        warnings: buildApprovalWarnings(input.path, input.content)
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
                case 'search':
                case 'open_app':
                case 'web_automation':
                case 'open_file':
                case 'open_folder':
                case 'create_folder':
                case 'delete_file':
                case 'clipboard_write':
                case 'learn_file':
                case 'learn_folder':
                case 'system_info':
                case 'plugin':
                case 'mcp_tool':
                case 'mouse_move':
                case 'mouse_click':
                case 'type_text':
                case 'key_tap':
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
            target: summarizeToolTarget(action, input) + resultSummary
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
    if (config.enableAgentCollaboration === true && !readOnlyTask && executedSteps > 8 && finalSummary) {
        const availableProviders = getAvailableProviders(config);
        // Exclude providers that often need special local setup or are slow/unreliable for tiny reviews
        const altProviders = availableProviders.filter(p => p !== provider && p !== 'ollama' && p !== 'huggingface' && p !== 'local_openai');
        
        // Fallback to provider itself if no other good ones exist, or pick the best available
        const reviewerProvider = altProviders.length > 0 
            ? altProviders[0] 
            : (availableProviders.includes('gemini') ? 'gemini' : availableProviders[0]);

        if (reviewerProvider && finalSummary) {
            onProgress({ phase: 'reviewing', action: 'reviewer_start', message: `Invoking Reviewer Agent (${reviewerProvider})...` });
            
            const reviewerClient = new providerAdapter.AgentProviderClient({
                provider: reviewerProvider,
                config,
                providerOrder: [reviewerProvider],
                systemInstruction: CODE_AGENT_PROMPT,
                responseMimeType: 'application/json',
                maxTokens: 4096
            });
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
        runVerificationCommands,
        walkDirectory,
        buildUnifiedDiffPreview,
        buildFullFileDiffPreview,
        buildApprovalWarnings,
        validateEditExplanation,
        formatPatchPreview,
        formatWritePreview,
        formatPlanPreview,
        formatPlanApprovalSummary,
        formatPlanMarkdown,
        writePlanFile,
        updatePlanApprovalStatus,
        normalizePlanItems,
        normalizePlanItemLanguage,
        requiresMultiFilePlan,
        getMissingPlanFiles,
        isReadOnlyTask,
        isWriteLikeAction,
        getEditTargetPath,
        PLAN_FILE_PATH
    }
};
