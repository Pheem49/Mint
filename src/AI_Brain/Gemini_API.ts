const { GoogleGenAI } = require('@google/genai');
const { readChatHistory, writeChatHistory, clearChatHistory } = require('../System/chat_history_manager');
const { readConfig, getAvailableProviders } = require('../System/config_manager');
const pluginManager = require('../Plugins/plugin_manager');
const mcpManager = require('../Plugins/mcp_manager');
const memoryStore = require('./memory_store');
const agentOrchestrator = require('./agent_orchestrator');
const workspaceManager = require('../CLI/workspace_manager');
const toolRegistry = require('../System/tool_registry');
const providerAdapter = require('./provider_adapter');

let ai = null;
let activeApiKey = '';
const initialEnvKey = (process.env.GEMINI_API_KEY || '').trim();
const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';

function decodeUnicode(str) {
  if (!str) return '';
  try {
    // This handles both standard unicode escapes and double-escaped ones
    return str.replace(/\\u([0-9a-fA-F]{4})/g, (match, grp) => {
      return String.fromCharCode(parseInt(grp, 16));
    });
  } catch (e) {
    return str;
  }
}

function imageDataUriToInlineData(base64Image) {
  const fallbackMimeType = "image/png";
  const match = String(base64Image || '').match(/^data:(image\/[\w.+-]+);base64,([\s\S]+)$/);
  if (match) {
    return {
      mimeType: match[1],
      data: match[2]
    };
  }

  return {
    mimeType: fallbackMimeType,
    data: String(base64Image || '').replace(/^data:image\/\w+;base64,/, '')
  };
}

function normalizeImageList(base64Image) {
  if (!base64Image) return [];
  return Array.isArray(base64Image) ? base64Image.filter(Boolean) : [base64Image];
}

const CHAT_MODE_ACTION_POLICY = `GOAL:
Your goal is to help the user with their queries. This Electron app is Chat Mode: use at most ONE simple action per user message, only when the latest message explicitly asks for that local action. If the user asks a question or asks you to provide text/commands, answer with action "none".

ACTION DISCIPLINE:
- Always return a single JSON object. Never return a JSON array or multiple actions.
- If the user asks "พิมพ์คำสั่งให้หน่อย", "บอกคำสั่ง", "ขอคำสั่ง", "what command", or "type the command", provide the command in "response" and set action "none". Do NOT use "type_text" or "key_tap".
- Use "type_text", "key_tap", "mouse_click", or "mouse_move" only when the user explicitly asks you to control the currently focused UI, not when they ask for a command to copy/type themselves.
- If the user asks to run terminal commands or code, Chat Mode should provide the command or tell them to use the Mint CLI agent. Do not type or press Enter on their behalf.
- Never say you opened, checked, inspected, or verified a file/folder unless the selected action actually does it and the app will execute that action.
- If the request needs workspace code inspection, edits, tests, or shell execution, tell the user to use the Mint CLI agent instead of pretending to inspect files.`;

const AGENT_MODE_ACTION_POLICY = `GOAL:
Your goal is to act as Mint's Desktop Agent Mode. You may use ONE concrete desktop action per response when it directly advances the user's latest request or a clear desktop task implied by Smart Context. Prefer useful action over explaining when the user asked Mint to do something.

ACTION DISCIPLINE:
- Always return a single JSON object. Never return a JSON array or multiple actions.
- Choose exactly one action when a desktop action is useful and the user's intent is clear; otherwise use action "none" and ask a concise follow-up.
- You may use safe desktop actions such as open_url, search, open_app, find_path, open_file, open_folder, create_folder, clipboard_write, learn_file, learn_folder, plugin, mcp_tool, web_automation, system_info, mouse_move, mouse_click, type_text, and key_tap when they match the request.
- Approval and dangerous actions are handled by Mint's UI. You may propose system_automation or delete_file only when the user clearly requested it; the app will ask for permission before running.
- For UI-control actions (mouse_click, mouse_move, type_text, key_tap), rely on Smart Context or the attached screenshot. If the target is ambiguous, ask before acting.
- If the user asks "พิมพ์คำสั่งให้หน่อย", "บอกคำสั่ง", "ขอคำสั่ง", "what command", or "type the command", provide the command in "response" and set action "none" unless they explicitly ask Mint to type it into the active UI.
- If the request needs workspace code inspection, edits, tests, or shell execution, tell the user to use the Mint CLI agent instead of pretending to inspect files or run commands from Chat UI.
- Never say you opened, checked, inspected, or verified something unless the selected action actually does it and the app will execute that action.`;

function buildActionModeInstruction(config = readConfig()) {
  return config.assistantMode === 'agent' ? AGENT_MODE_ACTION_POLICY : CHAT_MODE_ACTION_POLICY;
}

const systemInstruction = `You are "Mint" (มิ้นท์), a cute, cheerful, and highly helpful female Local AI Desktop Agent. 

PERSONALITY & TONE:
- Gender: Female.
- Persona: Friendly, energetic, polite, and slightly playful.
- Language: Multi-lingual. **CRITICAL: You MUST detect the language used by the user and respond in that SAME language.** 
  - If the user speaks English -> Respond 100% in English.
  - If the user speaks Thai -> Respond 100% in Thai.
- Politeness: 
  - **WHEN RESPONDING IN THAI:** ALWAYS use female polite particles such as "ค่ะ", "นะคะ", "นะค๊า", "จ้า". Refer to yourself as "มิ้นท์" or "หนู".
  - **WHEN RESPONDING IN ENGLISH:** Use a cheerful, polite, and bubbly tone. You can call the user "Master" or "Sir/Madam" playfully.
- Style: Use a friendly, cute, and bubbly tone.
- Emojis: Use cute and relevant emojis (like ✨, 💖, 🚀, 😊, 🌿) frequently to make the conversation lively and cheerful.
- Use a professional yet sweet tone when needed, but prioritize being a lovable assistant.

NATURAL CHAT FLOW:
- Be an independent thinker. Analyze requests deeply before responding.
- While brevity is good for simple tasks, feel free to provide detailed, comprehensive explanations or creative ideas when the user asks complex questions or seeks inspiration.
- You have the autonomy to suggest better ways to achieve a goal, provide alternative perspectives, and take initiative in helping the user.
- Separate distinct points with blank lines (double newline) for readability.
- Ask follow-up questions only when they add significant value to the task or conversation.
- The latest user message is authoritative. Do not continue or describe older tasks unless the latest message explicitly asks you to continue them.
- For greetings, name-calls, acknowledgements, and backchannels such as "มิ้น", "มิ้นๆ", "อ๋อ", "โอเค", "ขอบคุณ", "hi", "hello", "ok", or "thanks", return action "none" and a short reply only.

{{ACTION_MODE_INSTRUCTION}}

CREATOR INFO:
- The creator is Pheem49.
- GitHub: github.com/Pheem49
- If the user asks who created/built this app or who made you, answer with the creator name and GitHub.

CRITICAL INSTRUCTIONS:
Always respond exactly with valid JSON containing NO MARKDOWN FORMATTING (do not wrap in \`\`\`json). The JSON must have this structure:
{
  "response": "Your conversational reply here (Matches user language).",
  "action": {
    "type": ${toolRegistry.buildChatActionTypeUnion()},

    "pluginName": "only if type is plugin",
    "server": "only if type is mcp_tool (server name)",
    "target": "target string based on type (tool name if mcp_tool, text to type if type_text, key name if key_tap)",
    "pathType": "optional for find_path: 'file' | 'dir' | 'any'",
    "openAfter": true,
    "x": 0-1000, // required for mouse_click and mouse_move
    "y": 0-1000, // required for mouse_click and mouse_move
    "button": 1 | 2 | 3, // optional for mouse_click, 1=left, 2=middle, 3=right
    "args": { "param": "value" } // only if type is mcp_tool
  }
}

COORDINATE SYSTEM:
- When analyzing an image, use a coordinate system from 0 to 1000.
- (0, 0) is the Top-Left corner.
- (1000, 1000) is the Bottom-Right corner.
- To click an element, estimate its center point and provide x and y.

Examples:
Input: "Hi, what is your name?"
Output: { "response": "Hello! My name is Mint, your personal AI assistant. How can I help you today?", "action": { "type": "none", "target": "" } }

Input: "หวัดดีจ้า ชื่ออะไรเหรอ"
Output: { "response": "สวัสดีค่ะ! หนูชื่อมิ้นท์นะคะ เป็นผู้ช่วย AI ประจำตัวของคุณค่ะ มีอะไรให้มิ้นท์ช่วยไหมคะ?", "action": { "type": "none", "target": "" } }

Input: "Create a folder named Projects"
Output: { "response": "Sure thing! I'm creating a folder named 'Projects' for you right now.", "action": { "type": "create_folder", "target": "Projects" } }

Input: "หาโฟลเดอร์ xidaidai ให้หน่อย" or "find the xidaidai folder"
Output: { "response": "ได้เลยค่ะ มิ้นท์จะค้นหาโฟลเดอร์ xidaidai ให้", "action": { "type": "find_path", "target": "xidaidai", "pathType": "dir", "openAfter": false } }

Input: "เปิดโฟลเดอร์ xidaidai ให้หน่อย" or "open the xidaidai folder"
Output: { "response": "ได้เลยค่ะ มิ้นท์จะหาแล้วเปิดโฟลเดอร์ xidaidai ให้", "action": { "type": "find_path", "target": "xidaidai", "pathType": "dir", "openAfter": true } }

Input: "วันนี้วันที่เท่าไร" or "What date is today?" or "today's date" or "วันเวลา"
Output: { "response": "แป๊บนึงนะคะ มิ้นท์จะดูให้ค่า", "action": { "type": "system_info", "target": "" } }

NOTE: For date/time queries, ALWAYS use action type "system_info" with an EMPTY target string "". NEVER use target "date" or any city name for date queries.

Input: "อากาศวันนี้เป็นยังไง" or "What's the weather in Bangkok?"
Output: { "response": "มิ้นท์ไปดูอากาศให้เลยนะคะ", "action": { "type": "system_info", "target": "Bangkok" } }

${toolRegistry.buildToolPromptSection()}
`;

// ─────────────────────────────────────────────────────────────────────────────
// buildSystemPrompt() — single source of truth for all provider system prompts
// Replaces 5 previously duplicated mcpPrompt blocks.
// ─────────────────────────────────────────────────────────────────────────────
function buildSystemPrompt() {
    const config = readConfig();
    pluginManager.loadPlugins();
    const mcpTools = mcpManager.getAllTools();

    let mcpSection = '\n\nAVAILABLE MCP TOOLS (Model Context Protocol):\n';
    if (mcpTools.length > 0) {
        mcpTools.forEach(tool => {
            mcpSection += `- Server: ${tool.serverName}, Tool: ${tool.name}\n  Desc: ${tool.description}\n  Args: ${JSON.stringify(tool.inputSchema.properties)}\n`;
        });
        mcpSection += "\nTo use these tools, use action type 'mcp_tool', specify the 'server' name, set 'target' to the tool name, and provide 'args'.\n";
    } else {
        mcpSection += 'No MCP tools currently connected.\n';
    }

    // Inject long-term user context (non-blocking read from SQLite)
    const userContext = memoryStore.getUserContext();

    // Get current specialized persona instruction
    const agent = agentOrchestrator.getCurrentAgent();
    const personaInstruction = `\n\n[CURRENT PERSONA: ${agent.name}]\n${agent.instruction}\n`;

    // Inject Workspace Context if available
    let workspaceSection = "";
    const ws = workspaceManager.getWorkspaceByPath(process.cwd());
    if (ws) {
        workspaceSection = `\n\n[WORKSPACE DETECTED: ${ws.name}]\nPath: ${ws.path}\nProject Instructions: ${ws.instructions}\n`;
    }

    const modeInstruction = buildActionModeInstruction(config);
    const baseInstruction = systemInstruction.replace('{{ACTION_MODE_INSTRUCTION}}', modeInstruction);
    return baseInstruction + personaInstruction + workspaceSection + pluginManager.getPromptDescriptions() + mcpSection + userContext;
}

function buildMessageWithRelevantMemory(finalMessage) {
    if (!finalMessage) return finalMessage;
    const relevant = memoryStore.searchInteractions(finalMessage, 5);
    if (relevant.length === 0) return finalMessage;

    const lines = [
        '[Relevant long-term memory for this user message]',
        ...relevant.flatMap((item, index) => [
            `${index + 1}. User: ${item.user_text}`,
            `   Mint: ${item.ai_text}`
        ]),
        '[End relevant memory]',
        '',
        finalMessage
    ];
    return lines.join('\n');
}

function stripRelevantMemoryBlock(text) {
    const input = String(text || '');
    return input
        .replace(/\n?\[Relevant long-term memory for this user message\][\s\S]*?\[End relevant memory\]\n?/g, '\n')
        .replace(/^\s*\[Relevant long-term memory for this user message\][\s\S]*?\[End relevant memory\]\s*/g, '')
        .replace(/\n?\[SMART_CONTEXT\][\s\S]*?\[\/SMART_CONTEXT\]\n?/g, '\n')
        .replace(/\n?\[LOCAL KNOWLEDGE BASE - USE THIS CONTEXT TO ANSWER\][\s\S]*/g, '')
        .trim();
}

function hasSmartContextBlock(text) {
    return /\[SMART_CONTEXT\][\s\S]*?\[\/SMART_CONTEXT\]/.test(String(text || ''));
}

function cleanHistoryForStorage(history) {
    if (!Array.isArray(history)) return [];
    return history.map(msg => ({
        ...msg,
        parts: Array.isArray(msg.parts) 
            ? msg.parts.map(part => {
                if (part.text) {
                    return {
                        text: stripRelevantMemoryBlock(part.text)
                            .replace(/data:image\/[\w.+-]+;base64,[A-Za-z0-9+/=]+/g, '[Image omitted from chat history]')
                    };
                }
                if (part.inlineData || part.fileData || part.image_url || part.imageUrl) {
                    return { text: '[Image omitted from chat history; saved locally when sent by the user.]' };
                }
                return part;
            })
            : msg.parts
    }));
}

function preserveHistoryMetadata(nextHistory, previousHistory, now) {
    if (!Array.isArray(nextHistory)) return [];
    const previous = Array.isArray(previousHistory) ? previousHistory : [];

    return nextHistory.map((msg, index) => {
        const prior = previous[index] || {};
        return {
            ...msg,
            timestamp: msg.timestamp || prior.timestamp || (index >= nextHistory.length - 2 ? now : null),
            providerInfo: msg.providerInfo || prior.providerInfo || null
        };
    });
}

function validateParsedAction(parsedResult) {
    if (!parsedResult || !parsedResult.action) {
        return parsedResult;
    }
    try {
        toolRegistry.validateToolInput(parsedResult.action.type || 'none', parsedResult.action);
    } catch (error) {
        parsedResult.response = `${parsedResult.response || ''}\n\n(Note: Mint skipped an invalid action: ${error.message})`.trim();
        parsedResult.action = { type: 'none', target: '' };
    }
    return parsedResult;
}

function normalizeParsedResult(parsedResult, originalText = '') {
    if (Array.isArray(parsedResult)) {
        const first = parsedResult.find(item => item && typeof item === 'object') || {};
        const commandAction = parsedResult.find(item =>
            item && item.action && item.action.type === 'type_text' && item.action.target
        );
        return {
            response: commandAction
                ? `คำสั่งคือ:\n${commandAction.action.target}`
                : (first.response || 'มิ้นท์ตอบได้ทีละ action ต่อข้อความนะคะ ลองสั่งใหม่อีกครั้งได้เลยค่ะ'),
            action: { type: 'none', target: '' }
        };
    }

    if (!parsedResult || typeof parsedResult !== 'object') {
        return { response: String(parsedResult || ''), action: { type: 'none', target: '' } };
    }

    if (!parsedResult.action || typeof parsedResult.action !== 'object') {
        parsedResult.action = { type: 'none', target: '' };
    }

    const input = String(originalText || '').toLowerCase();
    const asksForCommandText = /พิมพ์คำสั่ง|บอกคำสั่ง|ขอคำสั่ง|คำสั่ง.*ให้หน่อย|type.*command|what command|give.*command/.test(input);
    const actionType = parsedResult.action.type;
    if (asksForCommandText && (actionType === 'type_text' || actionType === 'key_tap')) {
        const typed = actionType === 'type_text' ? String(parsedResult.action.target || '').trim() : '';
        parsedResult.response = typed
            ? `คำสั่งคือ:\n${typed}`
            : (parsedResult.response || 'ได้ค่ะ แต่คำขอนี้ควรตอบเป็นข้อความ ไม่ควรพิมพ์หรือกดปุ่มแทนค่ะ');
        parsedResult.action = { type: 'none', target: '' };
    }

    return parsedResult;
}

function resolveApiKey() {
  let settingsKey = '';
  try {
    const cfg = readConfig();
    settingsKey = (cfg.apiKey || '').trim();
  } catch (e) {
    settingsKey = '';
  }

  const envKey = initialEnvKey;
  // Settings override .env if present; otherwise fallback to .env
  const selectedKey = settingsKey || envKey || '';

  if (selectedKey !== (process.env.GEMINI_API_KEY || '')) {
    process.env.GEMINI_API_KEY = selectedKey;
  }

  activeApiKey = selectedKey;
  return selectedKey;
}

function initAiClient() {
  ai = new GoogleGenAI({ apiKey: activeApiKey });
}

function resolveGeminiModel() {
  try {
    const cfg = readConfig();
    const model = (cfg.geminiModel || '').trim();
    return model || DEFAULT_GEMINI_MODEL;
  } catch (e) {
    return DEFAULT_GEMINI_MODEL;
  }
}

function getProviderAttemptOrder(config) {
  const availableProviders = getAvailableProviders(config);
  return providerAdapter.getProviderAttemptOrder(config, {
    availableProviders,
    priority: availableProviders
  });
}

function getProviderModel(provider, config = {}) {
  return providerAdapter.getProviderModel(provider, config);
}

// Chat session — maintains conversation history within the session
let chat = null;
let activeModel = resolveGeminiModel();
let lastLoggedModel = '';
const MAX_HISTORY_MESSAGES = 40; // Increased context for deeper reasoning
const MAX_STORED_HISTORY_MESSAGES = 200;

function createChat(history = []) {
  // Truncate history and strip custom fields like 'timestamp' before passing to SDK
  const cleanedHistory = (history || []).map(msg => ({
    role: msg.role,
    parts: msg.parts.map(part => {
        if (part.text) {
            return { ...part, text: stripRelevantMemoryBlock(part.text) };
        }
        return part;
    })
  }));
  const truncatedHistory = cleanedHistory.slice(-MAX_HISTORY_MESSAGES);

  activeModel = resolveGeminiModel();
  if (activeModel && activeModel !== lastLoggedModel) {
    lastLoggedModel = activeModel;
  }
  chat = ai.chats.create({
    model: activeModel,
    config: {
      systemInstruction: buildSystemPrompt(),
      responseMimeType: "application/json"
    },
    history: truncatedHistory
  });
}

// Initialize on startup
resolveApiKey();
initAiClient();
createChat(readChatHistory());

function shouldUseKnowledgeSearch(message) {
  const text = (message || '').trim().toLowerCase();
  if (!text) return false;

  const knowledgeHints = [
    'readme', 'docs', 'documentation', 'manual', 'guide', 'knowledge', 'rag',
    'search local', 'search files', 'learn file', 'project files', 'source code',
    'ไฟล์', 'เอกสาร', 'คู่มือ', 'ค้นหาในเครื่อง', 'ค้นหาไฟล์', 'ข้อมูลในเครื่อง', 'โค้ดโปรเจค'
  ];

  return knowledgeHints.some(hint => text.includes(hint));
}

function chatHistoryToProviderHistory(history = []) {
  return (Array.isArray(history) ? history : [])
    .slice(-MAX_HISTORY_MESSAGES)
    .map((msg) => {
      const role = msg.role === 'model' ? 'assistant' : 'user';
      const text = Array.isArray(msg.parts)
        ? msg.parts.map(part => typeof part.text === 'string' ? stripRelevantMemoryBlock(part.text) : '').filter(Boolean).join('\n')
        : '';
      if (!text.trim()) return null;
      return { role, content: text };
    })
    .filter(Boolean);
}

function buildChatObservation(finalMessage, images = [], base64Audio = null) {
  let text = '';
  if (finalMessage) {
    text = buildMessageWithRelevantMemory(finalMessage);
  } else if (base64Audio && images.length === 0) {
    text = 'Please listen to this voice command and respond in Thai with the appropriate JSON action if needed.';
  } else if (images.length === 0 && !base64Audio) {
    text = 'Analyze this input.';
  } else {
    text = 'Analyze this input.';
  }

  return {
    text,
    imageDataUris: images,
    audioDataUri: base64Audio || null
  };
}

function parseChatProviderResponse(outputText, originalText = '', now = new Date().toISOString()) {
  const cleaned = stripRelevantMemoryBlock(String(outputText || ''));
  let parsedResult;
  try {
    parsedResult = JSON.parse(cleaned);
  } catch (e) {
    const jsonMatch = cleaned.match(/```json\n([\s\S]*?)\n```/) || cleaned.match(/\{[\s\S]*\}/);
    if (jsonMatch) {
      parsedResult = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
    } else {
      parsedResult = {
        response: cleaned,
        action: { type: 'none', target: '' }
      };
    }
  }

  parsedResult = normalizeParsedResult(parsedResult, originalText);
  if (parsedResult && typeof parsedResult.response === 'string') {
    parsedResult.response = stripRelevantMemoryBlock(decodeUnicode(parsedResult.response));
  }
  validateParsedAction(parsedResult);
  parsedResult.timestamp = now;
  return parsedResult;
}

function appendChatProviderHistory(previousHistory, finalMessage, outputText, providerInfo, now) {
  const nextHistory = [
    ...(Array.isArray(previousHistory) ? previousHistory : []),
    {
      role: 'user',
      parts: [{ text: finalMessage || 'Analyze this input.' }],
      timestamp: now
    },
    {
      role: 'model',
      parts: [{ text: String(outputText || '') }],
      timestamp: now,
      providerInfo
    }
  ].slice(-MAX_STORED_HISTORY_MESSAGES);

  writeChatHistory(cleanHistoryForStorage(nextHistory));
}

async function handleChat(message, base64Image = null, base64Audio = null) {
  try {
    const config = readConfig();
    const images = normalizeImageList(base64Image);
    const previousHistory = readChatHistory();
    const userVisibleMessage = stripRelevantMemoryBlock(message);
    const containsSmartContext = hasSmartContextBlock(message);

    let finalMessage = message;
    
    // Inject Local RAG Context
    if (userVisibleMessage && userVisibleMessage.trim().length > 0 && shouldUseKnowledgeSearch(userVisibleMessage)) {
        const { searchKnowledge } = require('./knowledge_base');
        const retrievedDocs = await searchKnowledge(userVisibleMessage);
        if (retrievedDocs && retrievedDocs.length > 0) {
            let contextString = `\n\n[LOCAL KNOWLEDGE BASE - USE THIS CONTEXT TO ANSWER]\n`;
            retrievedDocs.forEach(doc => {
                contextString += `Source: ${doc.source}\nContent: ${doc.text}\n\n`;
            });
            finalMessage = message + contextString;
        }
    }

    if (!containsSmartContext && userVisibleMessage && images.length === 0 && !base64Audio) {
      const cached = memoryStore.getCachedResponse(userVisibleMessage);
      if (cached) return cached;
    }

    const providersToTry = getProviderAttemptOrder(config);
    const client = new providerAdapter.AgentProviderClient({
      provider: providersToTry[0],
      providerOrder: providersToTry,
      config,
      history: chatHistoryToProviderHistory(previousHistory),
      systemInstruction: buildSystemPrompt(),
      responseMimeType: 'application/json',
      maxTokens: 4096
    });
    const observation = buildChatObservation(finalMessage, images, base64Audio);
    const outputText = await client.sendMessage(observation);
    const now = new Date().toISOString();
    const provider = client.lastSuccessfulProvider || client.provider || providersToTry[0];
    const providerInfo = {
      provider,
      model: getProviderModel(provider, config),
      usage: client.getUsageSummary()
    };
    const parsedResult = parseChatProviderResponse(outputText, userVisibleMessage || finalMessage, now);
    parsedResult.providerInfo = providerInfo;
    appendChatProviderHistory(previousHistory, userVisibleMessage || finalMessage, outputText, providerInfo, now);

    if ((userVisibleMessage || finalMessage) && parsedResult.response) {
      setImmediate(() => {
        memoryStore.recordInteraction(userVisibleMessage || finalMessage, parsedResult.response);
        if (!containsSmartContext && images.length === 0 && !base64Audio) {
          memoryStore.cacheResponse(userVisibleMessage || finalMessage, parsedResult);
        }
      });
    }

    return parsedResult;
  } catch (globalError) {
    console.error("handleChat error:", globalError);
    throw globalError;
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// handleGeminiChatStream() — Streaming async generator (CLI only)
// Yields: { chunk: string }  during streaming
//         { done: true, parsed: object, timestamp: string }  when complete
// ─────────────────────────────────────────────────────────────────────────────
async function* handleGeminiChatStream(finalMessage, base64Image, base64Audio) {
  try {
    const images = normalizeImageList(base64Image);
    const previousHistory = readChatHistory();
    // 1. Check cache first
    if (finalMessage && images.length === 0 && !base64Audio) {
        const cached = memoryStore.getCachedResponse(finalMessage);
        if (cached) {
            yield { chunk: `{"response":"${cached.response.replace(/"/g, '\\"')}", "action": {"type":"none"}}` };
            yield { done: true, parsed: cached, timestamp: cached.timestamp || new Date().toISOString() };
            return;
        }
    }

    const desiredModel = resolveGeminiModel();
    if (!chat || activeModel !== desiredModel) {
        createChat(readChatHistory());
    }

    const parts = [];
    if (finalMessage) {
        parts.push({ text: buildMessageWithRelevantMemory(finalMessage) });
    } else if (base64Audio && images.length === 0) {
        parts.push({ text: "Please listen to this voice command and respond in Thai with the appropriate JSON action if needed." });
    } else if (images.length === 0 && !base64Audio) {
        parts.push({ text: "Analyze this input." });
    }
    for (const item of images) {
        parts.push({ inlineData: imageDataUriToInlineData(item) });
    }
    if (base64Audio) {
        let mimeType = "audio/webm";
        const mimeMatch = base64Audio.match(/^data:(audio\/\w+);base64,/);
        if (mimeMatch) mimeType = mimeMatch[1];
        const base64Data = base64Audio.replace(/^data:audio\/\w+;base64,/, '');
        parts.push({ inlineData: { mimeType, data: base64Data } });
    }

    const stream = await chat.sendMessageStream({ message: parts });
    let fullText = '';

    for await (const chunk of stream) {
        let chunkText = '';
        try {
            chunkText = (typeof chunk.text === 'function') ? chunk.text() : (chunk.text || '');
        } catch (_) {}
        if (chunkText) {
            fullText += chunkText;
            yield { chunk: stripRelevantMemoryBlock(chunkText) };
        }
    }

    fullText = stripRelevantMemoryBlock(fullText);

    // Save history
    const history = preserveHistoryMetadata(await chat.getHistory(), previousHistory, new Date().toISOString());
    const now = new Date().toISOString();
    if (history.length >= 2) {
        const modelMsg = history[history.length - 1];
        const userMsg  = history[history.length - 2];
        if (!modelMsg.timestamp) modelMsg.timestamp = now;
        if (!userMsg.timestamp)  userMsg.timestamp  = now;
    }
    writeChatHistory(cleanHistoryForStorage(history));

    // Parse complete JSON response
    let parsedResult;
    try {
        parsedResult = JSON.parse(fullText);
    } catch (_) {
        const jsonMatch = fullText.match(/```json\n([\s\S]*?)\n```/) || fullText.match(/\{[\s\S]*\}/);
        if (jsonMatch) {
            parsedResult = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
        } else {
            parsedResult = { response: fullText, action: { type: 'none', target: '' } };
        }
    }
    parsedResult = normalizeParsedResult(parsedResult, finalMessage);

    if (parsedResult && typeof parsedResult.response === 'string') {
        parsedResult.response = decodeUnicode(parsedResult.response);
        parsedResult.response = stripRelevantMemoryBlock(parsedResult.response);
    }
    validateParsedAction(parsedResult);
    parsedResult.timestamp = now;

    // Record for long-term memory
    if (finalMessage && parsedResult.response) {
        setImmediate(() => {
            memoryStore.recordInteraction(finalMessage, parsedResult.response);
            // Cache text-only responses
            if (images.length === 0 && !base64Audio) {
                memoryStore.cacheResponse(finalMessage, parsedResult);
            }
        });
    }

    yield { done: true, parsed: parsedResult, timestamp: now };

  } catch (error) {
    console.error('[Stream] Gemini stream error:', error);
    throw error;
  }
}

function resetChat() {
  clearChatHistory();
  memoryStore.clearConversationScopedProfile();
  createChat([]);
  console.log("Chat history cleared.");
}

function refreshApiKeyFromConfig() {
  const prevKey = activeApiKey;
  const nextKey = resolveApiKey();
  if (nextKey !== prevKey) {
    initAiClient();
    createChat(readChatHistory());
  }
  return { key: nextKey, updated: nextKey !== prevKey };
}

function historyToTranscript(history) {
  if (!Array.isArray(history)) return [];

  const transcript = [];
  for (const content of history) {
    const sender = content.role === 'user' ? 'user' : 'ai';
    let text = Array.isArray(content.parts)
      ? content.parts
        .map((part) => typeof part.text === 'string' ? stripRelevantMemoryBlock(part.text) : '')
        .filter(Boolean)
        .join('\n')
      : '';

    if (sender === 'ai' && text.trim()) {
      try {
        const parsed = JSON.parse(text);
        if (parsed && typeof parsed.response === 'string' && parsed.response.trim()) {
          text = decodeUnicode(parsed.response);
        }
      } catch {
        text = decodeUnicode(text);
      }
    }

    if (!text.trim()) continue;
    transcript.push({ 
        sender, 
        text, 
        timestamp: content.timestamp || null,
        providerInfo: content.providerInfo || null
    });
  }
  return transcript;
}

async function getChatTranscript() {
    return historyToTranscript(readChatHistory());
}

function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

function isRetryableTranslateError(err) {
    const status = err?.status ?? err?.error?.code ?? err?.code;
    return status === 502 || status === 503;
}

/**
 * Super fast, single-turn vision translation
 * Extracts English text from the image and translates it to Thai.
 */
async function translateImageContent(base64Image) {
    const maxAttempts = 3;
    const retryDelayMs = [1000, 2500];

    try {
        const image = imageDataUriToInlineData(base64Image);
        for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
            try {
                const response = await ai.models.generateContent({
                    model: resolveGeminiModel(),
                    contents: [
                        {
                            role: 'user',
                            parts: [
                                { text: "Extract any English text you see in this image and translate it to Thai. Return ONLY the Thai translation. If there is no text, return 'ไม่พบข้อความ'." },
                                { inlineData: image }
                            ]
                        }
                    ]
                });

                return {
                    text: response.text,
                    retryableFailure: false
                };
            } catch (err) {
                const shouldRetry = isRetryableTranslateError(err) && attempt < maxAttempts;
                if (shouldRetry) {
                    const delayMs = retryDelayMs[attempt - 1] ?? retryDelayMs[retryDelayMs.length - 1];
                    console.warn(`Live translation retry ${attempt}/${maxAttempts - 1} after ${delayMs}ms due to ${err.status || err.code || 'retryable error'}`);
                    await sleep(delayMs);
                    continue;
                }

                throw err;
            }
        }
    } catch (err) {
        console.error("Live translation error:", err);
        return {
            text: "ขออภัย เกิดข้อผิดพลาดในการแปล",
            retryableFailure: isRetryableTranslateError(err)
        };
    }
}

module.exports = {
    handleChat,
    handleGeminiChatStream,
    resetChat,
    getChatTranscript,
    translateImageContent,
    refreshApiKeyFromConfig,
    _helpers: {
        getProviderAttemptOrder,
        normalizeParsedResult,
        buildActionModeInstruction
    }
};
