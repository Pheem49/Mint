const fs = require('fs');
const path = require('path');
const os = require('os');
const { exec } = require('child_process');

// Handle electron dependency safely
let app, shell;
try {
    const electron = require('electron');
    app = electron.app;
    shell = electron.shell;
} catch (e) {
    app = null;
    shell = null;
}

function escapeRegExp(text) {
    return String(text).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

class CustomWorkflows {
    constructor() {
        const configDir = path.join(os.homedir(), '.config', 'mint');
        this.configPath = path.join(configDir, 'workflows.json');
        this.workflows = [];
        this.lastTriggered = {};
        this.cooldownMs = 60 * 60 * 1000; // 1 hour cooldown per rule
        this.checkIntervalMs = 15000;     // 15 seconds poll rate
        this.timer = null;
        this.webContents = null;
        
        if (!fs.existsSync(configDir)) {
            fs.mkdirSync(configDir, { recursive: true });
        }

        this.migrateConfig();
        this.ensureConfigExists();
        this.loadWorkflows();
    }
    
    migrateConfig() {
        if (!fs.existsSync(this.configPath) && app && app.getPath) {
            const electronPath = path.join(app.getPath('userData'), 'workflows.json');
            if (fs.existsSync(electronPath)) {
                try {
                    fs.copyFileSync(electronPath, this.configPath);
                    console.log('[CustomWorkflows] Migrated workflows from Electron userData');
                } catch (e) { console.error('[CustomWorkflows] Migration failed:', e); }
            }
        }
    }
    
    ensureConfigExists() {
        if (!fs.existsSync(this.configPath)) {
            const defaultWorkflows = [
                {
                    id: "wf-1",
                    name: "Check Mic on Zoom",
                    trigger: { type: "process_running", processName: "zoom" },
                    action: { type: "system_info", message: "Looks like you opened Zoom! Should I check your system resources? 📸", target: "" }
                },
                {
                    id: "wf-2",
                    name: "Coding Time",
                    trigger: { type: "process_running", processName: "code" },
                    action: { type: "open_app", target: "spotify", message: "Coding time! Want me to open Spotify for you? 🎧" }
                }
            ];
            fs.writeFileSync(this.configPath, JSON.stringify(defaultWorkflows, null, 4), 'utf-8');
        }
    }
    
    loadWorkflows() {
        try {
            const raw = fs.readFileSync(this.configPath, 'utf-8');
            this.workflows = JSON.parse(raw);
            console.log(`[CustomWorkflows] Loaded ${this.workflows.length} rules.`);
        } catch (e) {
            console.error("[CustomWorkflows] Failed to load workflows.json", e);
            this.workflows = [];
        }
    }
    
    startMonitoring(webContents) {
        this.webContents = webContents;
        if (this.timer) clearInterval(this.timer);
        this.timer = setInterval(() => this.checkProcesses(), this.checkIntervalMs);
        console.log('[CustomWorkflows] Started process monitoring.');
    }
    
    stopMonitoring() {
        if (this.timer) {
            clearInterval(this.timer);
            this.timer = null;
        }
    }
    
    openConfigFile() {
        // Try to open directly in VS Code since .json might natively open in a browser
        exec(`code "${this.configPath}"`, (err) => {
            if (err) {
                // Fallback: Open the folder containing the config file
                shell.showItemInFolder(this.configPath);
                console.log("[CustomWorkflows] Opened folder instead since VS Code wasn't found in PATH.");
            }
        });
    }
    
    async checkProcesses() {
        if (!this.workflows || this.workflows.length === 0) return;
        
        try {
            // Linux check using ps to get process names
            exec('ps -A -o comm=', (err, stdout) => {
                if (err) return; 
                const runningProcesses = stdout.toLowerCase();
                
                for (const wf of this.workflows) {
                    if (wf.trigger && wf.trigger.type === 'process_running' && wf.trigger.processName) {
                        const targetName = wf.trigger.processName.toLowerCase();
                        // simplistic exact-word match to avoid partial matches
                        const regex = new RegExp(`^${escapeRegExp(targetName)}$`, 'm');
                        const isRunning = regex.test(runningProcesses);
                        
                        if (isRunning) {
                            const lastTime = this.lastTriggered[wf.id] || 0;
                            const now = Date.now();
                            
                            // Cooldown mechanism
                            if (now - lastTime > this.cooldownMs) {
                                this.triggerWorkflow(wf);
                                this.lastTriggered[wf.id] = now;
                            }
                        }
                    }
                }
            });
        } catch (error) {
            console.error("Workflow check error:", error);
        }
    }
    
    triggerWorkflow(wf) {
        if (!this.webContents || this.webContents.isDestroyed()) return;
        
        console.log(`[CustomWorkflows] Triggering workflow: ${wf.name}`);
        
        const suggestion = {
            message: wf.action.message || `💡 Automation triggered: ${wf.name}`,
            suggestions: [
                { label: "Yes, please", action: wf.action },
                { label: "Dismiss", action: { type: "none" } }
            ]
        };
        
        this.webContents.send('proactive-suggestion', suggestion);
    }
}

const instance = new CustomWorkflows();
instance._helpers = { escapeRegExp };

module.exports = instance;
