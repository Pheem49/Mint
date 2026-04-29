const { spawn } = require('child_process');
const { readConfig } = require('../System/config_manager');

/**
 * McpManager handles the lifecycle of multiple MCP servers.
 * Since MCP SDK is ESM and this project is CommonJS, we use dynamic imports.
 */
class McpManager {
    constructor() {
        this.clients = new Map(); // serverName -> { client, transport }
        this.tools = [];
    }

    async init() {
        const config = readConfig();
        const mcpServers = config.mcpServers || {};

        console.log(`[MCP] Initializing ${Object.keys(mcpServers).length} servers...`);

        // Load SDK via dynamic import
        const { Client } = await import('@modelcontextprotocol/sdk/client/index.js');
        const { StdioClientTransport } = await import('@modelcontextprotocol/sdk/client/stdio.js');

        for (const [name, serverConfig] of Object.entries(mcpServers)) {
            try {
                console.log(`[MCP] Connecting to server: ${name}`);
                
                const transport = new StdioClientTransport({
                    command: serverConfig.command,
                    args: serverConfig.args,
                    env: { ...process.env, ...serverConfig.env }
                });

                const client = new Client(
                    { name: 'mint-ai-assistant', version: '1.2.4' },
                    { capabilities: {} }
                );

                await client.connect(transport);
                
                // Discover tools
                const toolsResponse = await client.listTools();
                const serverTools = (toolsResponse.tools || []).map(t => ({
                    ...t,
                    serverName: name
                }));

                this.clients.set(name, { client, transport, tools: serverTools });
                this.tools.push(...serverTools);

                console.log(`[MCP] Server ${name} connected. Found ${serverTools.length} tools.`);
            } catch (err) {
                console.error(`[MCP] Failed to connect to server ${name}:`, err.message);
            }
        }
    }

    getAllTools() {
        return this.tools;
    }

    async callTool(serverName, toolName, args) {
        const server = this.clients.get(serverName);
        if (!server) throw new Error(`MCP Server "${serverName}" not found or not connected.`);

        try {
            console.log(`[MCP] Calling tool ${toolName} on server ${serverName}...`);
            const result = await server.client.callTool({
                name: toolName,
                arguments: args
            });
            return result;
        } catch (err) {
            console.error(`[MCP] Error calling tool ${toolName}:`, err);
            throw err;
        }
    }

    async shutdown() {
        console.log('[MCP] Shutting down all servers...');
        for (const [name, server] of this.clients.entries()) {
            try {
                await server.client.close();
                console.log(`[MCP] Server ${name} closed.`);
            } catch (err) {
                console.error(`[MCP] Error closing server ${name}:`, err.message);
            }
        }
        this.clients.clear();
        this.tools = [];
    }
}

const instance = new McpManager();
module.exports = instance;
