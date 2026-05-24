const fs = require('fs');
const path = require('path');
const { GoogleGenAI } = require('@google/genai');
const { executeCodeTask, _helpers: codeAgentHelpers } = require('./code_agent');
const { readConfig, getAvailableProviders } = require('../System/config_manager');

const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';

const CODE_KEYWORDS = [
    'code', 'repo', 'repository', 'project', 'workspace', 'file', 'files', 'readme',
    'package.json', 'bug', 'fix', 'refactor', 'test', 'tests', 'build', 'lint',
    'implement', 'feature', 'cli', 'function', 'module', 'component', 'diff',
    'list', 'show', 'ls', 'dir', 'directory', 'folders' // เพิ่มคำเหล่านี้ค่ะ
];


const THAI_CODE_KEYWORDS = [
    'โค้ด', 'โปรเจค', 'โปรเจ็กต์', 'ไฟล์', 'รีโป', 'บั๊ก', 'แก้', 'ทดสอบ', 'เทสต์',
    'รีแฟกเตอร์', 'ฟีเจอร์', 'คอมโพเนนต์', 'ฟังก์ชัน', 'อ่าน', 'สำรวจ', 'โครงสร้าง',
    'ไดเรกทอรี', 'โฟลเดอร์'
];

const ROUTER_PROMPT = `You classify whether a chat message should be routed to a coding agent for the current local workspace.

Return JSON only:
{
  "route": "code" | "chat",
  "reason": "short explanation"
}

Choose "code" when the user is asking to inspect, edit, review, debug, explain, refactor, verify, or otherwise operate on the current project/workspace/codebase/files.
Choose "chat" for general conversation, factual Q&A, small/simple requests, non-code assistant tasks, or direct file-system actions like finding/opening a folder or file by name.
Only choose "code" for substantial coding work that likely needs multiple steps, workspace inspection, edits, verification, or project-wide reasoning.`;

function isDirectFilesystemActionRequest(text) {
    const input = (text || '').trim().toLowerCase();
    if (!input) return false;

    const filesystemActionPattern = /(open|find|locate|search for|look for|หา|ค้นหา|เปิด)/;
    const filesystemTargetPattern = /(folder|directory|dir|file|โฟลเดอร์|ไฟล์|ไดเรกทอรี)/;
    const codeOperationPattern = /(inspect|review|refactor|debug|implement|edit|change|fix|explain|analyze|สำรวจ|รีวิว|รีแฟกเตอร์|แก้|อธิบาย|วิเคราะห์)/;

    return filesystemActionPattern.test(input) && filesystemTargetPattern.test(input) && !codeOperationPattern.test(input);
}

function workspaceLooksLikeCodebase(workspaceRoot) {
    const markers = [
        'package.json',
        '.git',
        'src',
        'README.md',
        'tsconfig.json',
        'Cargo.toml',
        'pyproject.toml'
    ];
    return markers.some(marker => fs.existsSync(path.join(workspaceRoot, marker)));
}

function detectCodeIntentHeuristic(text, workspaceRoot = process.cwd()) {
    const input = (text || '').trim().toLowerCase();
    if (!input) return false;
    if (input.startsWith('/code ')) return true;
    if (isDirectFilesystemActionRequest(input)) return false;

    return isLargeCodeTaskRequest(input, workspaceRoot);
}

function isLargeCodeTaskRequest(text, workspaceRoot = process.cwd()) {
    const input = (text || '').trim().toLowerCase();
    if (!input) return false;
    if (!workspaceLooksLikeCodebase(workspaceRoot)) return false;

    const hasCodeKeyword = CODE_KEYWORDS.some(keyword => input.includes(keyword));
    const hasThaiCodeKeyword = THAI_CODE_KEYWORDS.some(keyword => input.includes(keyword));
    const referencesProject = /โปรเจคนี้|โปรเจ็กต์นี้|this project|this repo|this repository|codebase|workspace|โฟลเดอร์นี้|ในนี้/.test(input);
    const asksForAction = /สำรวจ|ดู|แก้|เพิ่ม|ลบ|ปรับ|ตรวจ|วิเคราะห์|ลิสต์|โชว์|แสดง|มี|implement|inspect|explore|fix|update|change|refactor|review|explain|debug|list|show/.test(input);
    const strongTaskSignal = /failing tests?|run tests?|verify|verification|bug|issue|error|refactor|implement|feature|patch|edit|modify|analyze the project|แก้บั๊ก|รันเทสต์|ทดสอบ|ตรวจสอบ|ยืนยันผล|รีแฟกเตอร์|เพิ่มฟีเจอร์|แก้โค้ด|วิเคราะห์โปรเจค/.test(input);
    const multiStepSignal = /and|then|พร้อม|แล้ว|จากนั้น|ทั้ง|ทั่วทั้ง|ทั้งโปรเจค|project-wide|entire project|whole project/.test(input);

    // If they ask for files/folder content specifically, it's a code task because Chat can't do it accurately
    const isListFilesRequest = (hasCodeKeyword || hasThaiCodeKeyword) && /มี|โชว์|แสดง|ลิสต์|list|show|what|anything|อะไรบ้าง/.test(input) && /ไฟล์|file|folder|dir|โฟลเดอร์/.test(input);

    if (isListFilesRequest) return true;
    if (referencesProject && strongTaskSignal) return true;
    if ((hasCodeKeyword || hasThaiCodeKeyword) && asksForAction && strongTaskSignal) return true;
    if ((hasCodeKeyword || hasThaiCodeKeyword) && multiStepSignal && asksForAction) return true;
    return false;
}

function getRouterClient() {
    const config = readConfig();
    const apiKey = (config.apiKey || process.env.GEMINI_API_KEY || '').trim();
    if (!apiKey) return null;
    return {
        ai: new GoogleGenAI({ apiKey }),
        model: (config.geminiModel || DEFAULT_GEMINI_MODEL).trim() || DEFAULT_GEMINI_MODEL
    };
}

function summarizeWorkspace(workspaceRoot) {
    const candidates = ['package.json', 'README.md', 'src', '.git'];
    return candidates
        .filter(item => fs.existsSync(path.join(workspaceRoot, item)))
        .join(', ') || '(no obvious code markers)';
}

async function detectCodeIntent(text, workspaceRoot = process.cwd(), history = []) {
    const input = (text || '').trim();
    if (!input) {
        return { route: 'chat', reason: 'Empty input.' };
    }

    if (input.startsWith('/code ')) {
        return { route: 'code', reason: 'Explicit /code command.' };
    }

    if (isDirectFilesystemActionRequest(input)) {
        return { route: 'chat', reason: 'Direct file-system action request.' };
    }

    const heuristicRoute = detectCodeIntentHeuristic(input, workspaceRoot);
    if (!heuristicRoute) {
        return {
            route: 'chat',
            reason: 'No substantial code intent detected.'
        };
    }

    const routerClient = getRouterClient();
    if (!routerClient) {
        return {
            route: heuristicRoute ? 'code' : 'chat',
            reason: heuristicRoute ? 'Heuristic code intent match.' : 'Heuristic chat fallback.'
        };
    }

    try {
        const response = await routerClient.ai.models.generateContent({
            model: routerClient.model,
            config: {
                systemInstruction: ROUTER_PROMPT,
                responseMimeType: 'application/json'
            },
            contents: [{
                role: 'user',
                parts: [{
                    text: [
                        `Workspace: ${workspaceRoot}`,
                        `Workspace markers: ${summarizeWorkspace(workspaceRoot)}`,
                        `Context (Last 5 turns): ${history.slice(-10).map(m => `${m.sender}: ${m.text}`).join('\n')}`,
                        `Current Message: ${input}`
                    ].join('\n')
                }]
            }]
        });

        const textOutput = typeof response.text === 'function' ? response.text() : response.text;
        const parsed = JSON.parse(textOutput);
        const route = parsed.route === 'code' ? 'code' : 'chat';
        if (route === 'code' && !isLargeCodeTaskRequest(input, workspaceRoot)) {
            return {
                route: 'chat',
                reason: 'Request looks small enough for normal chat.'
            };
        }
        return {
            route,
            reason: parsed.reason || (route === 'code' ? 'Model classified as code.' : 'Model classified as chat.')
        };
    } catch (error) {
        return {
            route: heuristicRoute ? 'code' : 'chat',
            reason: heuristicRoute ? 'Heuristic fallback after router error.' : 'Chat fallback after router error.'
        };
    }
}

async function runChatRoutedTask(input, context) {
    const text = input.startsWith('/code ') ? input.slice('/code '.length).trim() : input;
    const { appendMessage, setThinking, requestApproval, askUser, setMode, history } = context;

    const config = readConfig();
    const availableProviders = getAvailableProviders(config);
    const preferredProvider = codeAgentHelpers.selectSupportedCodeProvider(config, availableProviders);

    appendMessage('system', `Routing this request to Code Mode for workspace: ${process.cwd()} using [${preferredProvider}]`);
    if (setMode) setMode('Code');

    let seconds = 0;
    setThinking(true, seconds);
    const timer = setInterval(() => {
        seconds++;
        setThinking(true, seconds);
    }, 1000);

    try {
        let streamedFinalSummary = false;
        const result = await executeCodeTask(text, {
            cwd: process.cwd(),
            requestApproval,
            askUser,
            provider: preferredProvider,
            history: history,
            onProgress: (info) => {
                if (context.appendCodeStep) {
                    context.appendCodeStep(info);
                } else {
                    appendMessage('system', `[Code] ${typeof info === 'string' ? info : (info.action || info.phase)}`);
                }
            },
            onFinalSummary: async (info) => {
                if (typeof context.streamAssistantSentences !== 'function') {
                    return;
                }
                clearInterval(timer);
                setThinking(false);
                streamedFinalSummary = true;
                await context.streamAssistantSentences(info.summary, appendMessage, { providerInfo: info.providerInfo }, context.streamMessage);
            }
        });
        clearInterval(timer);
        setThinking(false);
        if (!streamedFinalSummary) {
            appendMessage('assistant', [
                `Code Mode finished.`,
                result.summary,
                `Verification: ${result.verification}`
            ].join('\n'), { providerInfo: result.providerInfo });
        }
    } catch (error) {
        clearInterval(timer);
        setThinking(false);
        appendMessage('error', error.message);
    } finally {
        if (setMode) setMode('Agent');
    }
}

module.exports = {
    detectCodeIntent,
    runChatRoutedTask,
    _helpers: {
        detectCodeIntentHeuristic,
        isDirectFilesystemActionRequest,
        isLargeCodeTaskRequest
    }
};
