const { GoogleGenAI } = require('@google/genai');
const { readConfig } = require('../System/config_manager');
const { performWebAutomation } = require('../Automation_Layer/browser_automation');
const { createFolder, deleteFile } = require('../Automation_Layer/file_operations');
const { searchKnowledge } = require('./knowledge_base');
const fs = require('fs');
const path = require('path');

const os = require('os');

const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';

function expandHome(filePath) {
    if (filePath.startsWith('~/')) {
        return path.join(os.homedir(), filePath.slice(2));
    }
    return filePath;
}

const AUTONOMOUS_SYSTEM_PROMPT = `You are the "Mint Autonomous Brain". Your goal is to fulfill complex tasks by breaking them down into steps.
You operate in a ReAct loop: Thought -> Action -> Observation -> Thought...

CRITICAL INSTRUCTIONS:
1. Respond ONLY with valid JSON. NO MARKDOWN.
2. For BASH/Terminal commands: Use the "propose_bash" action. You are NOT allowed to run them yourself.
3. For Web tasks: Use "web_automation".
4. For File tasks: Use "create_folder", "write_file", "delete_file".
5. For Knowledge: Use "knowledge_search".

JSON Structure:
{
  "thought": "Your reasoning about the current state and what to do next.",
  "action": "web_automation" | "create_folder" | "write_file" | "delete_file" | "knowledge_search" | "propose_bash" | "done",
  "target": "The input for the action (URL/Path/Query/Filename/Command/Final Result)",
  "data": "Optional extra data (e.g., content for write_file)"
}

TOOL DETAILS:
- "web_automation": Use for any task requiring a browser. Target is the natural language instruction for the browser.
- "create_folder": Target is the folder name/path.
- "write_file": Target is the file path. Data is the content. Prefer using "~/Desktop" or "~/Documents" for home-relative paths.
- "delete_file": Target is the file path. (User will be notified).
- "knowledge_search": Target is the query for the local RAG.
- "propose_bash": Target is the bash command to show to the user. ALWAYS use SINGLE QUOTES (') for strings containing special characters like "!" to avoid Bash history expansion errors (e.g., use 'Pop!_OS' instead of "Pop!_OS").
- "done": Target is the final summary of what was accomplished.
`;

async function executeAutonomousTask(taskDescription, notifyCallback) {
    const config = readConfig();
    const modelName = config.geminiModel || DEFAULT_GEMINI_MODEL;
    const apiKey = config.apiKey || process.env.GEMINI_API_KEY;
    
    // Use the custom chat creation pattern from the project
    const ai = new GoogleGenAI({ apiKey });
    const chat = ai.chats.create({
        model: modelName,
        config: {
            systemInstruction: AUTONOMOUS_SYSTEM_PROMPT,
            responseMimeType: "application/json"
        },
        history: []
    });

    let currentObservation = `Task: ${taskDescription}\nWhat is your first step?`;
    let maxSteps = 10;
    let step = 0;
    let result = null;

    while (step < maxSteps) {
        step++;
        if (notifyCallback) notifyCallback(`Step ${step}: Thinking...`);

        try {
            const response = await chat.sendMessage({ message: [{ text: currentObservation }] });
            const text = response.text;
            const actionObj = JSON.parse(text);

            console.log(`[Brain] Thought: ${actionObj.thought}`);
            console.log(`[Brain] Action: ${actionObj.action} -> ${actionObj.target}`);

            if (actionObj.action === 'done') {
                result = actionObj.target;
                break;
            }

            // Execute the action
            let observation = "";
            switch (actionObj.action) {
                case 'web_automation':
                    if (notifyCallback) notifyCallback(`🌐 มิ้นท์กำลังเข้าเว็บเพื่อ ${actionObj.target}...`);
                    observation = await performWebAutomation(actionObj.target);
                    break;
                case 'create_folder':
                    const folderPath = expandHome(actionObj.target);
                    if (notifyCallback) notifyCallback(`📁 กำลังสร้างโฟลเดอร์: ${actionObj.target}`);
                    const resFolder = createFolder(folderPath);
                    observation = resFolder.success ? `Folder created at ${resFolder.path}` : `Failed: ${resFolder.message}`;
                    break;
                case 'write_file':
                    const filePath = expandHome(actionObj.target);
                    if (notifyCallback) notifyCallback(`✍️ กำลังบันทึกไฟล์: ${actionObj.target}`);
                    try {
                        fs.writeFileSync(filePath, actionObj.data || '');
                        observation = `File written successfully to ${actionObj.target}`;
                    } catch (e) {
                        observation = `Failed to write file: ${e.message}`;
                    }
                    break;
                case 'delete_file':
                    const delPath = expandHome(actionObj.target);
                    if (notifyCallback) notifyCallback(`🗑️ มิ้นท์ขอย้ายไฟล์ไปที่ถังขยะ: ${actionObj.target}`);
                    const resDel = await deleteFile(delPath);
                    observation = resDel.success ? "File moved to trash." : `Failed: ${resDel.message}`;
                    break;
                case 'knowledge_search':
                    if (notifyCallback) notifyCallback(`🔍 กำลังหาข้อมูลในเครื่อง: ${actionObj.target}`);
                    const docs = await searchKnowledge(actionObj.target);
                    observation = (docs && docs.length > 0) ? `Found: ${docs.map(d => d.text).join('\n')}` : "No information found in local knowledge base.";
                    break;
                case 'propose_bash':
                    if (notifyCallback) notifyCallback(`💡 มิ้นท์เสนอให้รันคำสั่ง: ${actionObj.target}`);
                    observation = `USER NOTIFIED of bash command: ${actionObj.target}. Note: You must wait for user to run it manually. If you can continue without it, do so. Otherwise, indicate you are waiting or done with this phase.`;
                    break;
                default:
                    observation = `Unknown action: ${actionObj.action}`;
            }

            currentObservation = `Observation: ${observation}`;

        } catch (err) {
            console.error('[AutonomousBrain] Error during loop:', err);
            currentObservation = `Error occurred: ${err.message}. Please try a different approach or conclude if task is impossible.`;
        }
    }

    return result || "Task reached maximum steps without a final result.";
}

module.exports = { executeAutonomousTask };
