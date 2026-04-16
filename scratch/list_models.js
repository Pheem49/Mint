const { GoogleGenAI } = require('@google/genai');
const fs = require('fs');
const path = require('path');

// Read config to get API key
const configPath = path.join(__dirname, 'mint-config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const apiKey = config.apiKey;

if (!apiKey) {
    console.error("No API Key found in mint-config.json");
    process.exit(1);
}

const genAI = new GoogleGenAI({ apiKey });

async function listModels() {
    try {
        const models = await genAI.models.list();
        console.log("Available Models:");
        models.forEach(m => {
            console.log(`- ${m.name} (${m.displayName})`);
        });
    } catch (err) {
        console.error("Error listing models:", err);
    }
}

listModels();
