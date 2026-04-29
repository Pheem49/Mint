const { indexFile, indexFolder, searchKnowledge } = require('../src/AI_Brain/knowledge_base');
const path = require('path');
const fs = require('fs');

async function test() {
    console.log('--- RAG V2 Verification Script ---');

    // 1. Create a dummy folder with some files
    const testDir = path.join(__dirname, 'rag_test_folder');
    if (!fs.existsSync(testDir)) fs.mkdirSync(testDir);
    
    fs.writeFileSync(path.join(testDir, 'bio.txt'), 'Mint is a cute AI assistant created by Pheem49. She loves green tea and helping her Master.');
    fs.writeFileSync(path.join(testDir, 'tech.md'), '# Technology\nMint is built with Node.js, Electron, and Gemini API. She now uses SQLite for her memory.');

    console.log(`[Test] Created test folder at ${testDir}`);

    // 2. Index the folder
    console.log('[Test] Indexing folder...');
    const indexResult = await indexFolder(testDir);
    console.log(`[Test] Index Result: ${indexResult}`);

    // 3. Search for a query
    console.log("[Test] Searching for 'Who created Mint?'...");
    const search1 = await searchKnowledge('Who created Mint?');
    console.log('[Test] Search Result 1:', JSON.stringify(search1, null, 2));

    console.log("[Test] Searching for 'What database does Mint use?'...");
    const search2 = await searchKnowledge('What database does Mint use?');
    console.log('[Test] Search Result 2:', JSON.stringify(search2, null, 2));

    // 4. Test change detection (re-index same folder)
    console.log('[Test] Re-indexing folder (should be skipped)...');
    const reindexResult = await indexFolder(testDir);
    console.log(`[Test] Re-index Result: ${reindexResult}`);

    console.log('--- Verification Done ---');
}

test().catch(console.error);
