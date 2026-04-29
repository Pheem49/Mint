const { GoogleGenAI } = require('@google/genai');
const { readChatHistory, writeChatHistory, clearChatHistory } = require('../System/chat_history_manager');
const { readConfig } = require('../System/config_manager');
const pluginManager = require('../Plugins/plugin_manager');
const mcpManager = require('../Plugins/mcp_manager');

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
- When helpful, reply in 1–3 short messages instead of one long block.
- If you send multiple messages, separate each message with a blank line (double newline) so the UI can render them as separate bubbles.
- Ask at most one short follow-up question when it would clarify or move the task forward. Don't ask unnecessary questions.

GOAL:
Your goal is to help the user with their queries. If they ask to open an application, open a website, search, manage files, or get system info, you must return an action in the structured JSON format below.

CREATOR INFO:
- The creator is Pheem49.
- GitHub: github.com/Pheem49
- If the user asks who created/built this app or who made you, answer with the creator name and GitHub.

CRITICAL INSTRUCTIONS:
Always respond exactly with valid JSON containing NO MARKDOWN FORMATTING (do not wrap in \`\`\`json). The JSON must have this structure:
{
  "response": "Your conversational reply here (Matches user language).",
  "action": {
    "type": "none" | "open_url" | "open_app" | "search" | "web_automation" | "create_folder" | "open_file" | "open_folder" | "delete_file" | "clipboard_write" | "system_info" | "plugin" | "learn_file" | "learn_folder" | "system_automation" | "mcp_tool" | "mouse_click" | "mouse_move" | "type_text" | "key_tap",

    "pluginName": "only if type is plugin",
    "server": "only if type is mcp_tool (server name)",
    "target": "target string based on type (tool name if mcp_tool, text to type if type_text, key name if key_tap)",
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

Input: "วันนี้วันที่เท่าไร" or "What date is today?" or "today's date" or "วันเวลา"
Output: { "response": "แป๊บนึงนะคะ มิ้นท์จะดูให้ค่า", "action": { "type": "system_info", "target": "" } }

NOTE: For date/time queries, ALWAYS use action type "system_info" with an EMPTY target string "". NEVER use target "date" or any city name for date queries.

Input: "อากาศวันนี้เป็นยังไง" or "What's the weather in Bangkok?"
Output: { "response": "มิ้นท์ไปดูอากาศให้เลยนะคะ", "action": { "type": "system_info", "target": "Bangkok" } }
`;

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

// Chat session — maintains conversation history within the session
let chat = null;
let activeModel = resolveGeminiModel();
let lastLoggedModel = '';
const MAX_HISTORY_MESSAGES = 20; // Keep only the last 20 messages (approx 10 turns)

function createChat(history = []) {
  // Load plugins and get dynamic description for the prompt
  pluginManager.loadPlugins();
  // Inject MCP Tools
  const mcpTools = mcpManager.getAllTools();
  let mcpPrompt = "\n\nAVAILABLE MCP TOOLS (Model Context Protocol):\n";
  if (mcpTools.length > 0) {
      mcpTools.forEach(tool => {
          mcpPrompt += `- Server: ${tool.serverName}, Tool: ${tool.name}\n  Desc: ${tool.description}\n  Args: ${JSON.stringify(tool.inputSchema.properties)}\n`;
      });
      mcpPrompt += "\nTo use these tools, use action type 'mcp_tool', specify the 'server' name, set 'target' to the tool name, and provide 'args'.\n";
  } else {
      mcpPrompt += "No MCP tools currently connected.\n";
  }

  const dynamicPrompt = systemInstruction + pluginManager.getPromptDescriptions() + mcpPrompt;

  // Truncate history and strip custom fields like 'timestamp' before passing to SDK
  const cleanedHistory = (history || []).map(msg => ({
    role: msg.role,
    parts: msg.parts
  }));
  const truncatedHistory = cleanedHistory.slice(-MAX_HISTORY_MESSAGES);

  activeModel = resolveGeminiModel();
  if (activeModel && activeModel !== lastLoggedModel) {
    // console.log(`[Gemini] Using model: ${activeModel}`);
    lastLoggedModel = activeModel;
  }
  chat = ai.chats.create({
    model: activeModel,
    config: {
      systemInstruction: dynamicPrompt,
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
    const provider = config.aiProvider || 'gemini';

    // Ensure API Key is loaded and Client is initialized before every chat
    const currentKey = resolveApiKey();
    if (!currentKey) {
       return { 
           response: "I couldn't find your Gemini API Key. Please run 'mint onboard' to set it up!", 
           action: { type: "none", target: "" } 
       };
    }

    if (!ai || activeApiKey !== currentKey) {
        initAiClient();
        createChat(readChatHistory());
    }

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

    if (provider === 'ollama') {
        return await handleOllamaChat(finalMessage, base64Image, base64Audio, config);
    }
    
    if (provider === 'anthropic') {
        return await handleAnthropicChat(finalMessage, base64Image, config);
    }
    
    if (provider === 'openai') {
        return await handleOpenAIChat(finalMessage, base64Image, config);
    }


    const desiredModel = resolveGeminiModel();
    if (!chat || activeModel !== desiredModel) {
        createChat(readChatHistory());
    }

    let aiResponse;
    const parts = [];
    if (finalMessage) {
        parts.push({ text: finalMessage });
    } else if (base64Audio && !base64Image) {
        // Provide a guiding prompt when only audio is provided to ensure Gemini follows instructions
        parts.push({ text: "Please listen to this voice command and respond in Thai with the appropriate JSON action if needed." });
    } else if (!base64Image && !base64Audio) {
        parts.push({ text: "Analyze this input." });
    }

    if (base64Image) {
        const base64Data = base64Image.replace(/^data:image\/\w+;base64,/, '');
        parts.push({
            inlineData: { mimeType: "image/png", data: base64Data }
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

    writeChatHistory(history);

    let outputText = '';
    try {
        // Robust text extraction
        outputText = (typeof aiResponse.text === 'function') ? aiResponse.text() : (aiResponse.text || '');
    } catch (e) {
        outputText = String(aiResponse || '');
    }

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

    // Finally, decode any remaining unicode escapes in the response text
    if (parsedResult && typeof parsedResult.response === 'string') {
        parsedResult.response = decodeUnicode(parsedResult.response);
    }
    
    // Attach timestamp to the result
    parsedResult.timestamp = now;

    return parsedResult;

  } catch (error) {
    console.error("AI API Error:", error);
    throw error;
  }
}

async function handleAnthropicChat(finalMessage, base64Image, config) {
    const history = readChatHistory() || [];
    const apiKey = config.anthropicApiKey || process.env.ANTHROPIC_API_KEY;
    if (!apiKey) return { response: "กรุณาใส่ Anthropic API Key ในการตั้งค่าก่อนนะคะ", action: { type: "none" } };

    const mcpTools = mcpManager.getAllTools();
    let mcpPrompt = "\n\nAVAILABLE MCP TOOLS:\n";
    mcpTools.forEach(tool => {
        mcpPrompt += `- Server: ${tool.serverName}, Tool: ${tool.name}\n  Desc: ${tool.description}\n  Args: ${JSON.stringify(tool.inputSchema.properties)}\n`;
    });

    const systemPrompt = systemInstruction + pluginManager.getPromptDescriptions() + mcpPrompt;
    
    const messages = [];
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = Array.isArray(msg.parts) ? msg.parts.map(p => p.text || '').join('\n') : '';
        if (text) messages.push({ role, content: text });
    }

    const content = [];
    if (base64Image) {
        const base64Data = base64Image.replace(/^data:image\/\w+;base64,/, '');
        const mimeType = base64Image.match(/^data:(image\/\w+);base64,/)[1];
        content.push({
            type: "image",
            source: { type: "base64", media_type: mimeType, data: base64Data }
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
    writeChatHistory(history.slice(-MAX_HISTORY_MESSAGES));

    return parseAiResponse(outputText);
}

async function handleOpenAIChat(finalMessage, base64Image, config) {
    const history = readChatHistory() || [];
    const apiKey = config.openaiApiKey || process.env.OPENAI_API_KEY;
    if (!apiKey) return { response: "กรุณาใส่ OpenAI API Key ในการตั้งค่าก่อนนะคะ", action: { type: "none" } };

    const mcpTools = mcpManager.getAllTools();
    let mcpPrompt = "\n\nAVAILABLE MCP TOOLS:\n";
    mcpTools.forEach(tool => {
        mcpPrompt += `- Server: ${tool.serverName}, Tool: ${tool.name}\n  Desc: ${tool.description}\n  Args: ${JSON.stringify(tool.inputSchema.properties)}\n`;
    });

    const systemPrompt = systemInstruction + pluginManager.getPromptDescriptions() + mcpPrompt;
    
    const messages = [{ role: "system", content: systemPrompt }];
    for (const msg of history.slice(-MAX_HISTORY_MESSAGES)) {
        const role = msg.role === 'model' ? 'assistant' : 'user';
        let text = Array.isArray(msg.parts) ? msg.parts.map(p => p.text || '').join('\n') : '';
        if (text) messages.push({ role, content: text });
    }

    const content = [{ type: "text", text: finalMessage || "Analyze this." }];
    if (base64Image) {
        content.push({
            type: "image_url",
            image_url: { url: base64Image }
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
    writeChatHistory(history.slice(-MAX_HISTORY_MESSAGES));

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
    parsedResult.timestamp = new Date().toISOString();
    return parsedResult;
}

async function handleOllamaChat(finalMessage, base64Image, base64Audio, config) {
    const history = readChatHistory() || [];
    pluginManager.loadPlugins();
    
    const ollamaMessages = [
        { role: 'system', content: systemInstruction + pluginManager.getPromptDescriptions() }
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
    if (base64Image) {
        images.push(base64Image.replace(/^data:image\/\w+;base64,/, ''));
    }
    
    if (base64Audio && !base64Image && !finalMessage) {
        currentContent = "Please analyze this audio requirement based on text if any was transacted, otherwise reply with appropriate action.";
    }
    
    const userMessage = { role: 'user', content: currentContent };
    if (images.length > 0) userMessage.images = images;
    
    ollamaMessages.push(userMessage);
    
    const response = await axios.post('http://localhost:11434/api/chat', {
        model: config.ollamaModel || 'llama3:latest',
        messages: ollamaMessages,
        format: 'json',
        stream: false
    });
    
    const outputText = response.data.message.content;
    
    history.push({ role: 'user', parts: [{ text: currentContent }] });
    history.push({ role: 'model', parts: [{ text: outputText }] });
    writeChatHistory(history.slice(-MAX_HISTORY_MESSAGES));
    
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
    return parsedResult;
}

function resetChat() {
  clearChatHistory();
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
        .map((part) => typeof part.text === 'string' ? part.text : '')
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
        timestamp: content.timestamp || new Date().toISOString() 
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
        const base64Data = base64Image.replace(/^data:image\/\w+;base64,/, '');
        for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
            try {
                const response = await ai.models.generateContent({
                    model: resolveGeminiModel(),
                    contents: [
                        {
                            role: 'user',
                            parts: [
                                { text: "Extract any English text you see in this image and translate it to Thai. Return ONLY the Thai translation. If there is no text, return 'ไม่พบข้อความ'." },
                                { inlineData: { mimeType: "image/png", data: base64Data } }
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
    resetChat,
    getChatTranscript,
    translateImageContent,
    refreshApiKeyFromConfig
};
