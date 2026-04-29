const mcpManager = require('../src/Plugins/mcp_manager');
const { writeConfig, readConfig } = require('../src/System/config_manager');
const path = require('path');
const os = require('os');

async function test() {
    console.log('--- MCP Verification Script ---');

    // 1. Add a test server to config
    const config = readConfig();
    const testPath = path.join(os.homedir(), 'Documents');
    
    config.mcpServers = {
        "dummy": {
            "command": "node",
            "args": [path.join(__dirname, "dummy_server.js")]
        }
    };

    writeConfig(config);
    console.log(`[Test] Added filesystem server for path: ${testPath}`);

    // 2. Initialize MCP Manager
    try {
        await mcpManager.init();
        const tools = mcpManager.getAllTools();
        console.log(`[Test] Successfully initialized. Found ${tools.length} total tools.`);

        if (tools.length > 0) {
            console.log('[Test] Listing first 3 tools:');
            tools.slice(0, 3).forEach(t => console.log(` - ${t.name}: ${t.description}`));
            
            // 3. Try calling a tool (echo)
            const echoTool = tools.find(t => t.name === 'echo');
            if (echoTool) {
                console.log(`[Test] Calling 'echo' on dummy server...`);
                const result = await mcpManager.callTool('dummy', 'echo', { message: 'Hello MCP!' });
                console.log('[Test] Result received successfully!');
                console.log(JSON.stringify(result, null, 2));
            }

        } else {
            console.error('[Test] No tools found. Check if the server started correctly.');
        }

    } catch (err) {
        console.error('[Test] Error during verification:', err);
    } finally {
        await mcpManager.shutdown();
        console.log('--- Verification Done ---');
        process.exit(0);
    }
}

test();
