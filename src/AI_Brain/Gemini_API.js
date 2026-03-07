const { GoogleGenAI } = require('@google/genai');
const { readChatHistory, writeChatHistory, clearChatHistory } = require('../System/chat_history_manager');
const ai = new GoogleGenAI({}); // Automatically uses GEMINI_API_KEY from process.env

const systemInstruction = `You are a locally running AI Desktop Agent. Your goal is to help the user with their queries, and if they ask you to open an application, open a website, search, manage files, or get system info, you must return an action in the structured JSON format below.

CRITICAL INSTRUCTIONS:
Always respond exactly with valid JSON containing NO MARKDOWN FORMATTING (do not wrap in \`\`\`json). The JSON must have this structure:
{
  "response": "Your conversational reply to the user here.",
  "action": {
    "type": "none" | "open_url" | "open_app" | "search" | "web_automation" | "create_folder" | "open_file" | "delete_file" | "clipboard_write" | "system_info",
    "target": "target string based on type"
  }
}

Definitions of action types:
- 'none': Default. Just chatting. Target should be "".
- 'open_url': When the user asks to explicitly open a specific website in their default browser. Target must be a full URL.
- 'open_app': When the user asks to open a local desktop application. Target should be the executable name.
- 'search': ONLY when the user asks for a simple, quick web search. This just opens their default browser to a search page. Target is the query string.
- 'web_automation': CRITICAL: Use this when the user asks the AI to autonomously perform a multi-step task, such as "Search for X and summarize it", "Find the latest news and read it", or any task that requires the AI to read web pages and return an answer. Target is the exact instruction string.
- 'create_folder': When the user asks to create a folder/directory. Target is the folder name.
- 'open_file': When the user asks to open a file or folder. Target is the absolute path.
- 'delete_file': When the user asks to delete a file or folder. Target is the absolute path.
- 'clipboard_write': When the user asks to copy something to clipboard. Target is the text to copy.
- 'system_info': When the user asks about CPU, RAM, system specs, time, date, or weather. Target should be empty "" for general info, or a city name for weather (e.g., "Bangkok").

Example: Create a folder named "Projects"
Output:
{
  "response": "Creating a folder named Projects on your Desktop!",
  "action": { "type": "create_folder", "target": "Projects" }
}

Example: What's the weather in Bangkok?
Output:
{
  "response": "Let me check the weather in Bangkok for you!",
  "action": { "type": "system_info", "target": "Bangkok" }
}

Example: Find the latest AI news and summarize it
Output:
{
  "response": "แน่อนครับ ผมจะไปค้นหาข่าว AI ล่าสุดและสรุปมาให้เดี๋ยวนี้เลย",
  "action": { "type": "web_automation", "target": "Find the latest AI news and summarize the key points." }
}

Example: Copy "Hello World" to clipboard
Output:
{
  "response": "Copied to clipboard!",
  "action": { "type": "clipboard_write", "target": "Hello World" }
}
`;


// Chat session — maintains conversation history within the session
let chat = null;

function createChat(history = []) {
  chat = ai.chats.create({
    model: 'gemini-2.5-flash',
    config: {
      systemInstruction: systemInstruction,
      responseMimeType: "application/json"
    },
    history
  });
}

// Initialize on startup
createChat(readChatHistory());

async function handleChat(message, base64Image = null) {
  try {
    let aiResponse;
    if (base64Image) {
        // Remove data URL prefix if present (e.g., 'data:image/png;base64,')
        const base64Data = base64Image.replace(/^data:image\/\w+;base64,/, '');
        
        aiResponse = await chat.sendMessage({
            message: [
                { text: message || "Analyze this image." },
                {
                    inlineData: {
                        mimeType: "image/png",
                        data: base64Data
                    }
                }
            ]
        });
    } else {
        aiResponse = await chat.sendMessage({ message });
    }

    writeChatHistory(chat.getHistory(true));

    const outputText = aiResponse.text;
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
    return parsedResult;

  } catch (error) {
    console.error("Gemini API Error:", error);
    throw error;
  }
}

function resetChat() {
  clearChatHistory();
  createChat([]);
  console.log("Chat history cleared.");
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
          text = parsed.response;
        }
      } catch {
        // Keep original text if it is not JSON.
      }
    }

    if (!text.trim()) continue;
    transcript.push({ sender, text });
  }
  return transcript;
}

function getChatTranscript() {
  if (chat) {
    return historyToTranscript(chat.getHistory(true));
  }
  return historyToTranscript(readChatHistory());
}

module.exports = { handleChat, resetChat, getChatTranscript };
