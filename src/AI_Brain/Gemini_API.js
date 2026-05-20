const { GoogleGenAI } = require('@google/genai');
const { readChatHistory, writeChatHistory, clearChatHistory } = require('../System/chat_history_manager');
const { readConfig, getAvailableProviders, isPlaceholder } = require('../System/config_manager');
const pluginManager = require('../Plugins/plugin_manager');
const mcpManager = require('../Plugins/mcp_manager');
const memoryStore = require('./memory_store');
const agentOrchestrator = require('./agent_orchestrator');
const workspaceManager = require('../CLI/workspace_manager');
const toolRegistry = require('../System/tool_registry');

let ai = null;
let activeApiKey = '';
const initialEnvKey = (process.env.GEMINI_API_KEY || '').trim();
const axios = require('axios');
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

function imageDataUriToBase64(base64Image) {
  return imageDataUriToInlineData(base64Image).data;
}

function normalizeImageList(base64Image) {
  if (!base64Image) return [];
  return Array.isArray(base64Image) ? base64Image.filter(Boolean) : [base64Image];
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

GOAL:
Your goal is to help the user with their queries. If they ask to open an application, open a website, search, manage files, or get system info, you must trigger an action in the structured JSON format below. **NEVER provide a conversational response about performing an action without including the actual "action" object in your JSON.**

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

    return systemInstruction + personaInstruction + workspaceSection + pluginManager.getPromptDescriptions() + mcpSection + userContext;
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
        .replace(/\n?\[LOCAL KNOWLEDGE BASE - USE THIS CONTEXT TO ANSWER\][\s\S]*/g, '')
        .trim();
}

function cleanHistoryForStorage(history) {
    if (!Array.isArray(history)) return [];
    return history.map(msg => ({
        ...msg,
        parts: Array.isArray(msg.parts) 
            ? msg.parts.map(part => {
                if (part.text) {
                    return { ...part, text: stripRelevantMemoryBlock(part.text) };
                }
                return part;
            })
            : msg.parts
    }));
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
  const provider = config.aiProvider || 'gemini';
  const availableProviders = getAvailableProviders(config);
  const ordered = availableProviders.includes(provider)
    ? [provider, ...availableProviders.filter(p => p !== provider)]
    : availableProviders;
  return ordered.length > 0 ? ordered : ['gemini'];
}

function getProviderModel(provider, config = {}) {
  switch (provider) {
    case 'gemini':
      return (config.geminiModel || DEFAULT_GEMINI_MODEL).trim() || DEFAULT_GEMINI_MODEL;
    case 'anthropic':
      return config.anthropicModel || 'claude-3-5-sonnet-latest';
    case 'openai':
      return config.openaiModel || 'gpt-4o';
    case 'local_openai':
      return config.localModelName || 'local-model';
    case 'huggingface':
      return config.hfModel || 'meta-llama/Meta-Llama-3-8B-Instruct';
    case 'ollama':
      return config.ollamaModel || 'llama3:latest';
    default:
      return '';
  }
}

function withProviderInfo(result, provider, config = {}) {
  const normalized = (result && typeof result === 'object')
    ? result
    : { response: String(result || ''), action: { type: 'none', target: '' } };
  const providerInfo = {
    provider,
    model: getProviderModel(provider, config)
  };

  attachProviderInfoToLatestHistory(providerInfo);

  return {
    ...normalized,
    providerInfo
  };
}

function attachProviderInfoToLatestHistory(providerInfo) {
  try {
    const history = readChatHistory();
    for (let i = history.length - 1; i >= 0; i -= 1) {
      if (history[i] && history[i].role === 'model') {
        history[i].providerInfo = providerInfo;
        writeChatHistory(cleanHistoryForStorage(history));
        return;
      }
    }
  } catch (error) {
    console.warn('[Provider Info] Failed to persist provider metadata:', error.message);
  }
}

// Chat session — maintains conversation history within the session
let chat = null;
let activeModel = resolveGeminiModel();
let lastLoggedModel = '';
const MAX_HISTORY_MESSAGES = 40; // Increased context for deeper reasoning

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

async function handleChat(message, base64Image = null, base64Audio = null) {
  try {
    const config = readConfig();

    let finalMessage = message;
    
    // Inject Local RAG Context
    if (message && message.trim().length > 0 && shouldUseKnowledgeSearch(message)) {
        const { searchKnowledge } = require('./knowledge_base');
        const retrievedDocs = await searchKnowledge(message);
        if (retrievedDocs && retrievedDocs.length > 0) {
            let contextString = `\n\n[LOCAL KNOWLEDGE BASE - USE THIS CONTEXT TO ANSWER]\n`;
            retrievedDocs.forEach(doc => {
                contextString += `Source: ${doc.source}\nContent: ${doc.text}\n\n`;
            });
            finalMessage = message + contextString;
        }
    }

    const providersToTry = getProviderAttemptOrder(config);

    for (let i = 0; i < providersToTry.length; i++) {
        const currentProv = providersToTry[i];
        try {
            if (currentProv === 'ollama') {
                return withProviderInfo(await handleOllamaChat(finalMessage, base64Image, base64Audio, config), currentProv, config);
            }
            if (currentProv === 'anthropic') {
                return withProviderInfo(await handleAnthropicChat(finalMessage, base64Image, config), currentProv, config);
            }
            if (currentProv === 'openai') {
                return withProviderInfo(await handleOpenAIChat(finalMessage, base64Image, config), currentProv, config);
            }
            if (currentProv === 'local_openai') {
                return withProviderInfo(await handleLocalOpenAIChat(finalMessage, base64Image, config), currentProv, config);
            }
            if (currentProv === 'huggingface') {
                return withProviderInfo(await handleHuggingFaceChat(finalMessage, base64Image, config), currentProv, config);
            }

            const currentKey = resolveApiKey();
            if (!currentKey) {
                if (i === providersToTry.length - 1) {
                    return withProviderInfo({
                        response: "I couldn't find your Gemini API Key. Please run 'mint onboard' to set it up!",
                        action: { type: "none", target: "" }
                    }, currentProv, config);
                }
                console.warn("[Fallback System] Gemini API key missing. Skipping Gemini provider.");
                continue;
            }

            if (!ai || activeApiKey !== currentKey) {
                initAiClient();
                createChat(readChatHistory());
            }

            return withProviderInfo(await handleGeminiChat(finalMessage, base64Image, base64Audio), currentProv, config);
        } catch (error) {
            console.error(`[Fallback System] Provider '${currentProv}' failed:`, error.message);
            if (i === providersToTry.length - 1) {
                console.error("[Fallback System] All available providers failed.");
                throw error; // No more providers to fallback to
            }
            console.log(`[Fallback System] Switching to next available provider: '${providersToTry[i+1]}'`);
            // Continue the loop to try the next provider
        }
    }
  } catch (globalError) {
    console.error("handleChat error:", globalError);
    throw globalError;
  }
}

async function handleGeminiChat(finalMessage, base64Image, base64Audio) {
  try {
    const images = normalizeImageList(base64Image);
    // 1. Check cache first for text-only messages
    if (finalMessage && images.length === 0 && !base64Audio) {
        const cached = memoryStore.getCachedResponse(finalMessage);
        if (cached) return cached;
    }

    const desiredModel = resolveGeminiModel();
    if (!chat || activeModel !== desiredModel) {
        createChat(readChatHistory());
    }

    let aiResponse;
    const parts = [];
    if (finalMessage) {
        parts.push({ text: buildMessageWithRelevantMemory(finalMessage) });
    } else if (base64Audio && images.length === 0) {
        // Provide a guiding prompt when only audio is provided to ensure Gemini follows instructions
        parts.push({ text: "Please listen to this voice command and respond in Thai with the appropriate JSON action if needed." });
    } else if (images.length === 0 && !base64Audio) {
        parts.push({ text: "Analyze this input." });
    }

    for (const item of images) {
        const image = imageDataUriToInlineData(item);
        parts.push({
            inlineData: image
        });
    }

    if (base64Audio) {
        // Extract MIME type from the data URI if present, fallback to audio/webm
        let mimeType = "audio/webm";
        const mimeMatch = base64Audio.match(/^data:(audio\/\w+);base64,/);
        if (mimeMatch) {
            mimeType = mimeMatch[1];
        }
        
        const base64Data = base64Audio.replace(/^data:audio\/\w+;base64,/, '');
        parts.push({
            inlineData: { mimeType: mimeType, data: base64Data }
        });
    }

    aiResponse = await chat.sendMessage({ message: parts });

    // Save history with timestamps
    const history = await chat.getHistory();
    const now = new Date().toISOString();
    
    // Add timestamp to the last two messages (User and Model) if they don't have one
    if (history.length >= 2) {
        const modelMsg = history[history.length - 1];
        const userMsg = history[history.length - 2];
        if (!modelMsg.timestamp) modelMsg.timestamp = now;
        if (!userMsg.timestamp) userMsg.timestamp = now;
    } else if (history.length === 1) {
        const msg = history[0];
        if (!msg.timestamp) msg.timestamp = now;
    }

    writeChatHistory(cleanHistoryForStorage(history));

    let outputText = '';
    try {
        // Robust text extraction
        outputText = (typeof aiResponse.text === 'function') ? aiResponse.text() : (aiResponse.text || '');
    } catch (e) {
        outputText = String(aiResponse || '');
    }

    outputText = stripRelevantMemoryBlock(outputText);

    let parsedResult;
    try {
      parsedResult = JSON.parse(outputText);
    } catch (e) {
      // Fallback in case the model failed to return pure JSON
      console.error("Failed to parse JSON directly:", e);
      const jsonMatch = outputText.match(/```json\n([\s\S]*?)\n```/) || outputText.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        parsedResult = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
      } else {
        parsedResult = {
          response: outputText,
          action: { type: "none", target: "" }
        };
      }
    }

    // Decode any remaining unicode escapes in the response text
    if (parsedResult && typeof parsedResult.response === 'string') {
        parsedResult.response = decodeUnicode(parsedResult.response);
        parsedResult.response = stripRelevantMemoryBlock(parsedResult.response);
    }
    
    // Attach timestamp to the result
    validateParsedAction(parsedResult);
    parsedResult.timestamp = now;

    // Record interaction for long-term memory (non-blocking)
    if (finalMessage && parsedResult.response) {
        setImmediate(() => {
            memoryStore.recordInteraction(finalMessage, parsedResult.response);
            // Cache text-only responses
            if (images.length === 0 && !base64Audio) {
                memoryStore.cacheResponse(finalMessage, parsedResult);
            }
        });
    }

    return parsedResult;

  } catch (error) {
    console.error("AI API Error:", error);
    throw error;
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
    const history = await chat.getHistory();
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

async function handleAnthropicChat(finalMessage, base64Image, config) {
    const history = readChatHistory() || [];
    const images = normalizeImageList(base64Image);
    const apiKey = config.anthropicApiKey || process.env.ANTHROPIC_API_KEY;
    if (isPlaceholder(apiKey)) return { response: "กรุณาใส่ Anthropic API Key ในการตั้งค่าก่อนนะคะ", action: { type: "none" } };

    const systemPrompt = buildSystemPrompt();
    
    const messages = [];
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = Array.isArray(msg.parts) ? msg.parts.map(p => p.text || '').join('\n') : '';
        if (text) messages.push({ role, content: text });
    }

    const content = [];
    for (const item of images) {
        const image = imageDataUriToInlineData(item);
        content.push({
            type: "image",
            source: { type: "base64", media_type: image.mimeType, data: image.data }
        });
    }
    content.push({ type: "text", text: finalMessage || "Analyze this." });
    messages.push({ role: "user", content });

    const response = await axios.post('https://api.anthropic.com/v1/messages', {
        model: config.anthropicModel || 'claude-3-5-sonnet-latest',
        max_tokens: 4096,
        system: systemPrompt,
        messages: messages
    }, {
        headers: {
            'x-api-key': apiKey,
            'anthropic-version': '2023-06-01',
            'content-type': 'application/json'
        }
    });

    const outputText = response.data.content[0].text;
    history.push({ role: 'user', parts: [{ text: finalMessage }] });
    history.push({ role: 'model', parts: [{ text: outputText }] });
    writeChatHistory(cleanHistoryForStorage(history.slice(-MAX_HISTORY_MESSAGES)));

    return parseAiResponse(outputText);
}

async function handleOpenAIChat(finalMessage, base64Image, config) {
    const history = readChatHistory() || [];
    const images = normalizeImageList(base64Image);
    const apiKey = config.openaiApiKey || process.env.OPENAI_API_KEY;
    if (isPlaceholder(apiKey)) return { response: "กรุณาใส่ OpenAI API Key ในการตั้งค่าก่อนนะคะ", action: { type: "none" } };

    const systemPrompt = buildSystemPrompt();
    
    const messages = [{ role: "system", content: systemPrompt }];
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = Array.isArray(msg.parts) ? msg.parts.map(p => p.text || '').join('\n') : '';
        if (text) messages.push({ role, content: text });
    }

    const content = [{ type: "text", text: finalMessage || "Analyze this." }];
    for (const item of images) {
        content.push({
            type: "image_url",
            image_url: { url: item }
        });
    }
    messages.push({ role: "user", content });

    const response = await axios.post('https://api.openai.com/v1/chat/completions', {
        model: config.openaiModel || 'gpt-4o',
        messages: messages,
        response_format: { type: "json_object" }
    }, {
        headers: {
            'Authorization': `Bearer ${apiKey}`,
            'Content-Type': 'application/json'
        }
    });

    const outputText = response.data.choices[0].message.content;
    history.push({ role: 'user', parts: [{ text: finalMessage }] });
    history.push({ role: 'model', parts: [{ text: outputText }] });
    writeChatHistory(cleanHistoryForStorage(history.slice(-MAX_HISTORY_MESSAGES)));

    return parseAiResponse(outputText);
}

async function handleLocalOpenAIChat(finalMessage, base64Image, config) {
    const history = readChatHistory() || [];
    const images = normalizeImageList(base64Image);
    const apiKey = 'lm-studio';
    const baseUrl = config.localApiBaseUrl || 'http://localhost:1234/v1';

    const systemPrompt = buildSystemPrompt();
    
    const messages = [{ role: "system", content: systemPrompt }];
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = Array.isArray(msg.parts) ? msg.parts.map(p => p.text || '').join('\n') : '';
        if (text) messages.push({ role, content: text });
    }

    const content = [{ type: "text", text: finalMessage || "Analyze this." }];
    for (const item of images) {
        content.push({
            type: "image_url",
            image_url: { url: item }
        });
    }
    messages.push({ role: "user", content });

    const response = await axios.post(`${baseUrl.replace(/\/$/, '')}/chat/completions`, {
        model: config.localModelName || 'local-model',
        messages: messages,
        // response_format json_object is sometimes problematic on weak local models, but required by our prompt.
        // We'll keep it as some local servers like LM Studio support it for specific models.
        // If not supported, the system prompt usually coerces it anyway.
        response_format: { type: "json_object" }
    }, {
        headers: {
            'Authorization': `Bearer ${apiKey}`,
            'Content-Type': 'application/json'
        }
    });

    const outputText = response.data.choices[0].message.content;
    history.push({ role: 'user', parts: [{ text: finalMessage }] });
    history.push({ role: 'model', parts: [{ text: outputText }] });
    writeChatHistory(cleanHistoryForStorage(history.slice(-MAX_HISTORY_MESSAGES)));

    return parseAiResponse(outputText);
}

async function handleHuggingFaceChat(finalMessage, base64Image, config) {
    const history = readChatHistory() || [];
    const images = normalizeImageList(base64Image);
    const apiKey = config.hfApiKey || process.env.HF_API_KEY;
    if (isPlaceholder(apiKey)) return { response: "กรุณาใส่ Hugging Face API Key ในการตั้งค่าก่อนนะคะ", action: { type: "none" } };

    const modelId = config.hfModel || 'meta-llama/Meta-Llama-3-8B-Instruct';
    const baseUrl = `https://api-inference.huggingface.co/models/${modelId}/v1/chat/completions`;

    const systemPrompt = buildSystemPrompt();
    
    const messages = [{ role: "system", content: systemPrompt }];
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = Array.isArray(msg.parts) ? msg.parts.map(p => p.text || '').join('\n') : '';
        if (text) messages.push({ role, content: text });
    }

    const content = [{ type: "text", text: finalMessage || "Analyze this." }];
    for (const item of images) {
        content.push({
            type: "image_url",
            image_url: { url: item }
        });
    }
    messages.push({ role: "user", content });

    const response = await axios.post(baseUrl, {
        model: modelId,
        messages: messages,
        max_tokens: 4096
    }, {
        headers: {
            'Authorization': `Bearer ${apiKey}`,
            'Content-Type': 'application/json'
        }
    });

    const outputText = response.data.choices[0].message.content;
    history.push({ role: 'user', parts: [{ text: finalMessage }] });
    history.push({ role: 'model', parts: [{ text: outputText }] });
    writeChatHistory(cleanHistoryForStorage(history.slice(-MAX_HISTORY_MESSAGES)));

    return parseAiResponse(outputText);
}

function parseAiResponse(outputText) {
    let parsedResult;
    try {
        parsedResult = JSON.parse(outputText);
    } catch (e) {
        const jsonMatch = outputText.match(/```json\n([\s\S]*?)\n```/) || outputText.match(/\{[\s\S]*\}/);
        if (jsonMatch) {
            parsedResult = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
        } else {
            parsedResult = { response: outputText, action: { type: "none", target: "" } };
        }
    }
    if (parsedResult && typeof parsedResult.response === 'string') {
        parsedResult.response = decodeUnicode(parsedResult.response);
    }
    validateParsedAction(parsedResult);
    parsedResult.timestamp = new Date().toISOString();
    return parsedResult;
}

async function handleOllamaChat(finalMessage, base64Image, base64Audio, config) {
    const history = readChatHistory() || [];
    const imageInputs = normalizeImageList(base64Image);
    
    const ollamaMessages = [
        { role: 'system', content: buildSystemPrompt() }
    ];
    
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = '';
        if (Array.isArray(msg.parts)) {
             text = msg.parts.map(p => p.text || '').join('\n');
        }
        if (text) ollamaMessages.push({ role, content: text });
    }
    
    let currentContent = finalMessage || 'Analyze this input.';
    let images = [];
    for (const item of imageInputs) {
        images.push(imageDataUriToBase64(item));
    }
    
    if (base64Audio && imageInputs.length === 0 && !finalMessage) {
        currentContent = "Please analyze this audio requirement based on text if any was transacted, otherwise reply with appropriate action.";
    }
    
    const userMessage = { role: 'user', content: currentContent };
    if (images.length > 0) userMessage.images = images;
    
    ollamaMessages.push(userMessage);
    
    const ollamaBaseUrl = (config.ollamaHost || 'http://localhost:11434').replace(/\/$/, '');
    const response = await axios.post(`${ollamaBaseUrl}/api/chat`, {
        model: config.ollamaModel || 'llama3:latest',
        messages: ollamaMessages,
        format: 'json',
        stream: false
    });
    
    const outputText = response.data.message.content;
    
    history.push({ role: 'user', parts: [{ text: currentContent }] });
    history.push({ role: 'model', parts: [{ text: outputText }] });
    writeChatHistory(cleanHistoryForStorage(history.slice(-MAX_HISTORY_MESSAGES)));
    
    let parsedResult;
    try {
        parsedResult = JSON.parse(outputText);
    } catch(e) {
        const jsonMatch = outputText.match(/```json\n([\s\S]*?)\n```/) || outputText.match(/\{[\s\S]*\}/);
        if (jsonMatch) {
            parsedResult = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
        } else {
            parsedResult = { response: outputText, action: { type: "none", target: "" } };
        }
    }
    validateParsedAction(parsedResult);
    return parsedResult;
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
        timestamp: content.timestamp || new Date().toISOString(),
        providerInfo: content.providerInfo || null
    });
  }
  return transcript;
}

async function getChatTranscript() {
    if (chat) {
        return historyToTranscript(await chat.getHistory(true));
    }
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
        getProviderAttemptOrder
    }
};
