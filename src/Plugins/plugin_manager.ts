const fs = require('fs');
const path = require('path');

class PluginManager {
    constructor() {
        this.plugins = new Map();
        this.pluginsDir = path.join(__dirname);
    }

    // Load or reload plugins
    loadPlugins() {
        this.plugins.clear();
        
        try {
            if (!fs.existsSync(this.pluginsDir)) return;

            const files = fs.readdirSync(this.pluginsDir);
            for (const file of files) {
                // Ignore self and core system managers
                if (file === 'plugin_manager.js' || file === 'mcp_manager.js' || !file.endsWith('.js')) continue;

                const pluginPath = path.join(this.pluginsDir, file);
                
                // Clear require cache for hot-reloading
                delete require.cache[require.resolve(pluginPath)];
                
                try {
                    const plugin = require(pluginPath);
                    if (this.validatePlugin(plugin)) {
                        this.plugins.set(plugin.name, plugin);
                        // console.log(`[PluginManager] Loaded: ${plugin.name}`);
                    } else {
                        console.warn(`[PluginManager] Invalid plugin format: ${file}`);
                    }
                } catch (err) {
                    console.error(`[PluginManager] Error loading plugin ${file}:`, err);
                }
            }
        } catch (err) {
            console.error('[PluginManager] Error accessing plugin directory:', err);
        }
    }

    validatePlugin(plugin) {
        return plugin 
            && typeof plugin.name === 'string' 
            && typeof plugin.description === 'string' 
            && typeof plugin.execute === 'function';
    }

    // Returns formatted descriptions for the Gemini prompt
    getPromptDescriptions() {
        if (this.plugins.size === 0) return '';
        
        let descriptions = '\nPlugin Actions Available:\n';
        for (const [name, plugin] of this.plugins.entries()) {
            descriptions += `- Plugin: "${name}" | Description: ${plugin.description}\n`;
        }
        return descriptions;
    }

    // Execute a plugin's action
    async executePlugin(name, instruction) {
        const plugin = this.plugins.get(name);
        if (!plugin) {
            return `Plugin "${name}" not found.`;
        }

        try {
            // console.log(`[PluginManager] Executing ${name} with instruction: "${instruction}"`);
            return await plugin.execute(instruction);
        } catch (err) {
            console.error(`[PluginManager] Error executing plugin ${name}:`, err);
            return `Error executing plugin ${name}: ${err.message}`;
        }
    }
}

// Export a singleton instance
const pluginManager = new PluginManager();
module.exports = pluginManager;
