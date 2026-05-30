import * as fs from 'fs'
import * as path from 'path'

const colors = {
    reset: "\x1b[0m",
    bright: "\x1b[1m",
    mint: "\x1b[38;5;121m",
    pink: "\x1b[38;5;213m",
    cyan: "\x1b[36m",
    yellow: "\x1b[33m"
};

function displayFeatures() {
    console.log(`\n${colors.mint}${colors.bright}Mint Feature & Command List${colors.reset}\n`);

    console.log(`${colors.bright}Quick Setup:${colors.reset}`);
    console.log(`  - ${colors.cyan}mint onboard${colors.reset}           : Setup API Key and Model`);
    console.log(`  - ${colors.cyan}mint onboard --daemon${colors.reset}  : Install background agent (Linux)`);
    console.log(`  - ${colors.cyan}mint chat${colors.reset}              : Start chatting (or just type "mint")`);

    console.log(`\n${colors.bright}CLI Commands:${colors.reset}`);
    const commands = [
        { cmd: 'mint', desc: 'Start interactive chat session (Default)' },
        { cmd: 'mint code "<task>"', desc: 'Run workspace-aware coding agent in current directory' },
        { cmd: 'mint summarize [path]', desc: 'Summarize repository structure, tooling, git state, and key files' },
        { cmd: 'mint symbols [path]', desc: 'Build a source symbol index for supported languages' },
        { cmd: 'mint semantic-code', desc: 'Index and search code semantically with embeddings' },
        { cmd: 'mint gmail auth', desc: 'Connect Gmail OAuth and save refresh token' },
        { cmd: 'mint mcp', desc: 'Manage Model Context Protocol (MCP) servers' },
        { cmd: 'mint task "<task>"', desc: 'Queue an autonomous task for the background agent' },
        { cmd: 'mint update', desc: 'Check for and install the latest Mint CLI version' },
        { cmd: 'mint onboard', desc: 'Run setup wizard (API Key, Model, Daemon)' },
        { cmd: 'mint agent', desc: 'Run Mint as a background agent (Headless)' },
        { cmd: 'mint list', desc: 'Show this features & commands list' }
    ];
    commands.forEach(c => console.log(`  - ${colors.cyan}${c.cmd.padEnd(18)}${colors.reset} : ${c.desc}`));

    console.log(`\n${colors.bright}AI Core Actions (Automation):${colors.reset}`);
    const actions = [
        { act: 'open_url', desc: 'Open any website or search Google' },
        { act: 'open_app', desc: 'Launch desktop applications' },
        { act: 'file_ops', desc: 'Create folders, Open files, Delete/Trash items' },
        { act: 'knowledge', desc: 'Learn from files (PDF, Docx, TXT, MD, XLSX) or Web URLs' },
        { act: 'system', desc: 'Volume, Brightness, Mute, Sleep, Power Control' }
    ];
    actions.forEach(a => console.log(`  - ${colors.yellow}${a.act.padEnd(15)}${colors.reset} : ${a.desc}`));

    console.log(`\n${colors.bright}Available Plugins:${colors.reset}`);
    const pluginsDir = path.join(__dirname, '../Plugins');
    try {
        const files = fs.readdirSync(pluginsDir);
        const plugins = files
            .filter(f => f.endsWith('.js') && f !== 'plugin_manager.js')
            .map(f => f.replace('.js', ''));
        
        plugins.forEach(p => console.log(`  - ${colors.pink}${p}${colors.reset}`));
    } catch (err) {
        console.log('  (Could not load plugins list)');
    }

    console.log(`\n${colors.mint}${colors.bright}Type "mint" to start exploring!${colors.reset}\n`);
}

export { displayFeatures  }
