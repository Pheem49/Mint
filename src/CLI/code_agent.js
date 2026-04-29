const fs = require('fs');
const path = require('path');
const { execFile } = require('child_process');
const { promisify } = require('util');
const { GoogleGenAI } = require('@google/genai');
const { readConfig } = require('../System/config_manager');
const { readWorkspaceSession, writeWorkspaceSession } = require('./code_session_memory');

const execFileAsync = promisify(execFile);
const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';
const MAX_TOOL_OUTPUT = 12000;
const MAX_AGENT_STEPS = 16;

const CODE_AGENT_PROMPT = `You are Mint Code Mode, a careful coding agent for a local workspace.

You help with software development tasks inside the provided working directory.
Work in an inspect -> plan -> act -> verify loop.

Rules:
1. Respond with valid JSON only.
2. Prefer reading files and searching before editing.
3. Make focused edits that preserve existing project style.
4. Use shell commands for inspection, tests, and formatting when useful.
5. Never use destructive commands like "rm -rf", "git reset --hard", or overwrite unrelated files.
6. Before any shell command or file patch is executed, the user must approve it. Plan accordingly.
7. When editing, prefer "apply_patch" with precise hunks over whole-file rewrites.
8. Use "write_file" only for new files or when a full rewrite is clearly safer.
9. When you are done, return "finish" with a concise summary, verification, and an updated session summary.

Response format:
{
  "thought": "short reasoning",
  "action": "list_files" | "read_file" | "search_code" | "run_shell" | "apply_patch" | "write_file" | "finish",
  "input": {
    "path": "relative/path",
    "query": "search text",
    "command": "shell command",
    "startLine": 1,
    "endLine": 120,
    "content": "full file content for write_file",
    "summary": "final summary",
    "verification": "tests or checks",
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
- "list_files": inspect the workspace or a subdirectory.
- "read_file": read a file, optionally with startLine/endLine.
- "search_code": search by text or regex-like pattern.
- "run_shell": run a non-destructive command in the workspace.
- "apply_patch": update an existing file using one or more exact replacement hunks.
- "write_file": create a new file or fully rewrite a file when replacement is not practical.
- "finish": stop once the task is complete or blocked.
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
        const entries = fs.readdirSync(cwd, { withFileTypes: true })
            .slice(0, 200)
            .map(entry => `${entry.isDirectory() ? '[dir]' : '[file]'} ${path.relative(workspaceRoot, path.join(cwd, entry.name))}`)
            .join('\n');
        return entries || '(empty directory)';
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
        if (error.stdout) {
            return truncate(error.stdout);
        }
        throw error;
    }
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

function getAiClientAndModel() {
    const config = readConfig();
    const apiKey = (config.apiKey || process.env.GEMINI_API_KEY || '').trim();
    if (!apiKey) {
        throw new Error("Missing Gemini API key. Run 'mint onboard' first.");
    }
    return {
        ai: new GoogleGenAI({ apiKey }),
        model: (config.geminiModel || DEFAULT_GEMINI_MODEL).trim() || DEFAULT_GEMINI_MODEL
    };
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

async function buildInitialObservation(task, workspaceRoot) {
    const session = readWorkspaceSession(workspaceRoot);
    const gitContext = await getGitContext(workspaceRoot);
    const testCommands = detectTestCommands(workspaceRoot);

    return [
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
    const onProgress = typeof options.onProgress === 'function' ? options.onProgress : () => {};
    const requestApproval = typeof options.requestApproval === 'function'
        ? options.requestApproval
        : async () => true;
    const { ai, model } = getAiClientAndModel();

    const chat = ai.chats.create({
        model,
        config: {
            systemInstruction: CODE_AGENT_PROMPT,
            responseMimeType: 'application/json'
        },
        history: []
    });

    let observation = await buildInitialObservation(task, workspaceRoot);

    for (let step = 1; step <= MAX_AGENT_STEPS; step++) {
        onProgress(`Step ${step}: thinking`);
        const response = await chat.sendMessage({ message: [{ text: observation }] });
        const text = typeof response.text === 'function' ? response.text() : response.text;
        const decision = extractJson(text);
        const action = decision.action;
        const input = decision.input || {};

        onProgress(`Step ${step}: ${action}${input.path ? ` ${input.path}` : input.command ? ` ${input.command}` : ''}`);

        if (action === 'finish') {
            const sessionSummary = input.sessionSummary || input.summary || task;
            writeWorkspaceSession(workspaceRoot, {
                summary: sessionSummary,
                lastTask: task,
                lastVerification: input.verification || 'Not specified.'
            });
            return {
                summary: input.summary || 'Task complete.',
                verification: input.verification || 'Not specified.',
                steps: step
            };
        }

        let toolResult = '';
        switch (action) {
            case 'list_files':
                toolResult = await listFiles(workspaceRoot, input.path || '.');
                break;
            case 'read_file':
                toolResult = readFileRange(workspaceRoot, input.path, input.startLine, input.endLine);
                break;
            case 'search_code':
                toolResult = await searchCode(workspaceRoot, input.query);
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
            default:
                throw new Error(`Unsupported action: ${action}`);
        }

        observation = [
            `Previous thought: ${decision.thought || '(none)'}`,
            `Action: ${action}`,
            'Observation:',
            toolResult
        ].join('\n');
    }

    writeWorkspaceSession(workspaceRoot, {
        summary: `Task stopped before completion: ${task}`,
        lastTask: task,
        lastVerification: 'Agent limit reached before explicit completion.'
    });

    return {
        summary: 'Stopped after reaching the maximum number of agent steps.',
        verification: 'Agent limit reached before explicit completion.',
        steps: MAX_AGENT_STEPS
    };
}

module.exports = { executeCodeTask };
