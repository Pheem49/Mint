const puppeteer = require('puppeteer');
const { GoogleGenAI } = require('@google/genai');
const { readConfig } = require('../System/config_manager');

const ai = new GoogleGenAI({});

const BROWSER_SYSTEM_PROMPT = `You are an Autonomous Browser Agent. Your goal is to fulfill the user's web instruction by driving a headless browser.

CRITICAL INSTRUCTIONS:
Always respond EXACTLY with valid JSON. NO MARKDOWN. NO CODE BLOCKS (\`\`\`json). The JSON must have this exact structure:
{
  "thought": "Reasoning about what to do next based on the current page content and goal.",
  "action": "goto" | "click" | "eval" | "done",
  "target": "URL for goto | CSS selector for click | JavaScript expression for eval | Final answer for done"
}

Actions:
- "goto": Navigate to the specified URL. Target MUST be a full URL (e.g. "https://www.google.com/search?q=AI+news")
- "click": Click an element. Target MUST be a valid CSS selector.
- "eval": Evaluate JavaScript to extract text. Target MUST be JS code returning a string (e.g. "document.body.innerText.substring(0, 1000)").
- "done": Task finished. Target MUST be the final summary or answer to present to the user.

You will receive the result of your previous action in the next message. If you get stuck or fail, try another approach or use "done" to report the failure.`;

async function performWebAutomation(query) {
    if (!query) return "No query provided.";

    console.log("Starting web automation for:", query);

    const config = readConfig();
    const browserPath = config.automationBrowser;

    let browser;
    try {
        const launchOptions = {
            headless: false,
            defaultViewport: null,
            args: ['--start-maximized']
        };

        // If it's a specific path (like /usr/bin/firefox), set executablePath
        if (browserPath && browserPath !== 'chromium') {
            launchOptions.executablePath = browserPath;
            if (browserPath.toLowerCase().includes('firefox')) {
                launchOptions.browser = 'firefox';
            }
        }

        browser = await puppeteer.launch(launchOptions);

        const page = await browser.newPage();
        
        const chat = ai.chats.create({
            model: 'gemini-2.5-flash',
            config: {
                systemInstruction: BROWSER_SYSTEM_PROMPT,
                responseMimeType: "application/json"
            }
        });

        let currentObservation = `Goal: ${query}\nSystem Note: You have a blank browser page. What is your first action? Start by using "goto" to navigate to a relevant search engine or website.`;
        
        let maxSteps = 10;
        let step = 0;

        while (step < maxSteps) {
            step++;
            console.log(`\n--- Agent Step ${step} ---`);
            console.log(`Observation:`, currentObservation.substring(0, 150) + (currentObservation.length > 150 ? '...' : ''));

            const response = await chat.sendMessage({ message: currentObservation });
            
            let parsed;
            try {
                const text = response.text;
                const cleanText = text.replace(/^```json\n/, '').replace(/\n```$/, '').trim();
                parsed = JSON.parse(cleanText);
            } catch (e) {
                console.error("Agent failed to return valid JSON:", response.text);
                currentObservation = "Error: Invalid JSON returned. Please reply with ONLY valid JSON matching the schema.";
                continue;
            }

            console.log("Agent Thought:", parsed.thought);
            console.log("Agent Action:", parsed.action);
            console.log("Agent Target:", parsed.target);

            const { action, target } = parsed;

            if (action === 'done') {
                console.log("Agent finished with answer:", target);
                // Intentionally keeping the browser open so the user can see the page.
                return `🤖 Web Automation Result: ${target}`;
            }

            try {
                if (action === 'goto') {
                    await page.goto(target, { waitUntil: 'domcontentloaded', timeout: 30000 });
                    const pageTitle = await page.title();
                    currentObservation = `Successfully navigated to ${pageTitle}. ` + await page.evaluate(() => document.body.innerText.substring(0, 1500));
                } else if (action === 'click') {
                    await page.waitForSelector(target, { timeout: 5000 });
                    await page.click(target);
                    await new Promise(r => setTimeout(r, 2000)); 
                    const pageTitle = await page.title();
                    currentObservation = `Clicked element. Current page: ${pageTitle}. ` + await page.evaluate(() => document.body.innerText.substring(0, 1500));
                } else if (action === 'eval') {
                    const evalResult = await page.evaluate(target);
                    currentObservation = `Eval result: ` + String(evalResult).substring(0, 1500);
                } else {
                    currentObservation = `Error: Unknown action type "${action}".`;
                }
            } catch (actionError) {
                console.error("Action execution failed:", actionError);
                currentObservation = `Action failed: ${actionError.message}. Please try again or use another method (for instance, try a different CSS selector or just read the current page).`;
            }
        }

        // Intentionally keeping the browser open
        return "Agent reached maximum steps (10) without finding a final answer.";

    } catch (error) {
        console.error("Web Automation Error:", error);
        if (browser) browser.close();
        return `I encountered an overall error while automating the browser: ${error.message}`;
    }
}

module.exports = { performWebAutomation };
