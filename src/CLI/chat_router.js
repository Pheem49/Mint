const fs = require('fs');
const path = require('path');
const { GoogleGenAI } = require('@google/genai');
const { executeCodeTask } = require('./code_agent');
const { readConfig } = require('../System/config_manager');

const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';

const CODE_KEYWORDS = [
    'code', 'repo', 'repository', 'project', 'workspace', 'file', 'files', 'readme',
    'package.json', 'bug', 'fix', 'refactor', 'test', 'tests', 'build', 'lint',
    'implement', 'feature', 'cli', 'function', 'module', 'component', 'diff'
];

const THAI_CODE_KEYWORDS = [
    'โค้ด', 'โปรเจค', 'โปรเจ็กต์', 'ไฟล์', 'รีโป', 'บั๊ก', 'แก้', 'ทดสอบ', 'เทสต์',
    'รีแฟกเตอร์', 'ฟีเจอร์', 'คอมโพเนนต์', 'ฟังก์ชัน', 'อ่าน', 'สำรวจ', 'โครงสร้าง'
];

const ROUTER_PROMPT = `You classify whether a chat message should be routed to a coding agent for the current local workspace.

Return JSON only:
{
  "route": "code" | "chat",
  "reason": "short explanation"
}

Choose "code" when the user is asking to inspect, edit, review, debug, explain, refactor, verify, or otherwise operate on the current project/workspace/codebase/files.
Choose "chat" for general conversation, factual Q&A, or non-code assistant tasks.`;

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

    const hasCodeKeyword = CODE_KEYWORDS.some(keyword => input.includes(keyword));
    const hasThaiCodeKeyword = THAI_CODE_KEYWORDS.some(keyword => input.includes(keyword));
    const referencesProject = /โปรเจคนี้|โปรเจ็กต์นี้|this project|this repo|this repository|codebase|workspace/.test(input);
    const asksForAction = /สำรวจ|ดู|แก้|เพิ่ม|ลบ|ปรับ|ตรวจ|วิเคราะห์|implement|inspect|explore|fix|update|change|refactor|review|explain|debug/.test(input);

    return workspaceLooksLikeCodebase(workspaceRoot) && (referencesProject || ((hasCodeKeyword || hasThaiCodeKeyword) && asksForAction));
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

async function detectCodeIntent(text, workspaceRoot = process.cwd()) {
    const input = (text || '').trim();
    if (!input) {
        return { route: 'chat', reason: 'Empty input.' };
    }

    if (input.startsWith('/code ')) {
        return { route: 'code', reason: 'Explicit /code command.' };
    }

    const heuristicRoute = detectCodeIntentHeuristic(input, workspaceRoot);
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
                        `Message: ${input}`
                    ].join('\n')
                }]
            }]
        });

        const textOutput = typeof response.text === 'function' ? response.text() : response.text;
        const parsed = JSON.parse(textOutput);
        const route = parsed.route === 'code' ? 'code' : 'chat';
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
    const { appendMessage, setThinking, requestApproval, setMode } = context;

    appendMessage('system', `Routing this request to Code Mode for workspace: ${process.cwd()}`);
    if (setMode) setMode('Code');

    let seconds = 0;
    setThinking(true, seconds);
    const timer = setInterval(() => {
        seconds++;
        setThinking(true, seconds);
    }, 1000);

    try {
        const result = await executeCodeTask(text, {
            cwd: process.cwd(),
            requestApproval,
            onProgress: (message) => appendMessage('system', `[Code] ${message}`)
        });
        clearInterval(timer);
        setThinking(false);
        appendMessage('assistant', [
            `Code Mode finished.`,
            result.summary,
            `Verification: ${result.verification}`
        ].join('\n'));
    } catch (error) {
        clearInterval(timer);
        setThinking(false);
        appendMessage('error', error.message);
    } finally {
        if (setMode) setMode('Chat');
    }
}

module.exports = {
    detectCodeIntent,
    runChatRoutedTask
};
