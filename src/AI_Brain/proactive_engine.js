const { GoogleGenAI } = require('@google/genai');
const path = require('path');
const fs = require('fs');
const { app } = require('electron');
const { readConfig } = require('../System/config_manager');

// ============================================================
// Proactive Engine — Smart Suggestion Engine (Multi-Choice)
// ============================================================

const ai = new GoogleGenAI({});
const DEFAULT_GEMINI_MODEL = 'gemini-3.1-flash-lite-preview';
let lastLoggedModel = '';

const PROACTIVE_SYSTEM_PROMPT = `You are a Smart Suggestion Engine built into a Desktop AI Agent called "Mint".
Your job: observe the user's screen + behavior, then offer MULTIPLE relevant quick-action options — NOT just one question.

CRITICAL RULES:
1. Respond ONLY with valid JSON, no markdown.
2. If nothing notable is on screen, return: {"message": null, "context": "", "suggestions": []}
3. Generate 2–4 SHORT suggestion chips that are genuinely useful based on what's visible.
4. Each suggestion must have a clear label (1–3 words) and an action.
5. Write "message" in Thai — short, friendly, observational (e.g. "พบว่าคุณเพิ่งเปิด Chrome").
6. Do NOT repeat suggestions from recent activities.
7. Suggestions should feel like smart shortcuts, not questions.

Response schema (STRICT):
{
  "context": "short English description of what you see on screen",
  "message": "สั้น ๆ เป็นภาษาไทย บอกว่า AI เห็นอะไร และเสนออะไร",
  "suggestions": [
    { "label": "YouTube", "action": { "type": "open_url", "target": "https://youtube.com" } },
    { "label": "Gmail",   "action": { "type": "open_url", "target": "https://mail.google.com" } },
    { "label": "GitHub",  "action": { "type": "open_url", "target": "https://github.com" } }
  ]
}

Action types allowed: "open_url", "open_app", "search", "none"

Examples:

SCENARIO: User opened Chrome or Firefox
→ message: "เพิ่งเปิด Browser — ต้องการเข้าเว็บไหนคะ?"
→ suggestions: YouTube, Gmail, GitHub, Google Maps (based on behavior history)

SCENARIO: User is in VS Code / coding
→ message: "กำลัง Code อยู่ใช่ไหมคะ? มีอะไรช่วยได้บ้าง"
→ suggestions: Stack Overflow, MDN Docs, GitHub, ค้นหา Error

SCENARIO: User opened Spotify
→ message: "เปิด Spotify แล้ว ต้องการเล่นอะไรคะ?"
→ suggestions: เพลง Chill, เพลง Focus, Top Charts, Podcast

SCENARIO: User opened Terminal
→ message: "เปิด Terminal แล้ว ต้องการทำอะไรคะ?"
→ suggestions: GitHub, Stack Overflow, DevDocs, ค้นหา Command

BAD examples (return null):
- Nothing notable on screen
- User is actively typing
- Same context as before
`;

let lastSuggestionContext = '';
let lastSuggestionTime = 0;

function resolveGeminiModel() {
    try {
        const cfg = readConfig();
        const model = (cfg.geminiModel || '').trim();
        return model || DEFAULT_GEMINI_MODEL;
    } catch {
        return DEFAULT_GEMINI_MODEL;
    }
}

function getMinSuggestionIntervalMs() {
    try {
        const CONFIG_PATH = path.join(app.getPath('userData'), 'mint-config.json');
        if (fs.existsSync(CONFIG_PATH)) {
            const cfg = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
            return (cfg.proactiveCooldown || 120) * 1000;
        }
    } catch {
        // ignore
    }
    return 120_000;
}

/**
 * Analyze screen and return a multi-choice suggestion object.
 * @param {string} base64Image
 * @param {string} behaviorSummary
 * @returns {Promise<{message: string, context: string, suggestions: Array} | null>}
 */
async function analyzeAndSuggest(base64Image, behaviorSummary) {
    try {
        const model = resolveGeminiModel();
        if (model && model !== lastLoggedModel) {
            console.log(`[Gemini] Proactive Engine model: ${model}`);
            lastLoggedModel = model;
        }

        const now = Date.now();
        const minInterval = getMinSuggestionIntervalMs();

        if (now - lastSuggestionTime < minInterval) return null;

        const base64Data = base64Image.replace(/^data:image\/\w+;base64,/, '');

        const userMessage = [
            {
                text: `Analyze the screen and generate smart multi-choice suggestions for the user.

User behavior context: ${behaviorSummary || 'No history yet.'}

Rules: Only suggest if you see a clear opportunity. Return 2–4 relevant chips. Return null message if nothing notable.`
            },
            {
                inlineData: {
                    mimeType: 'image/png',
                    data: base64Data
                }
            }
        ];

        const response = await ai.models.generateContent({
            model,
            config: {
                systemInstruction: PROACTIVE_SYSTEM_PROMPT,
                responseMimeType: 'application/json'
            },
            contents: [{ role: 'user', parts: userMessage }]
        });

        let parsed;
        try {
            parsed = JSON.parse(response.text);
        } catch {
            const jsonMatch = response.text.match(/\{[\s\S]*\}/);
            if (jsonMatch) parsed = JSON.parse(jsonMatch[0]);
            else return null;
        }

        // Validate: must have message and at least 1 suggestion
        if (!parsed || !parsed.message || !Array.isArray(parsed.suggestions) || parsed.suggestions.length === 0) {
            return null;
        }

        // Skip repeat context
        if (parsed.context && parsed.context === lastSuggestionContext) {
            console.log('[ProactiveEngine] Skipping repeat context.');
            return null;
        }

        lastSuggestionContext = parsed.context || '';
        lastSuggestionTime = now;

        console.log(`[ProactiveEngine] ${parsed.suggestions.length} suggestions for: ${parsed.context}`);
        return parsed;

    } catch (err) {
        console.error('[ProactiveEngine] Error:', err.message);
        return null;
    }
}

module.exports = { analyzeAndSuggest };
