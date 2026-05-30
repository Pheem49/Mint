import fs from 'fs'
import path from 'path'
import { execSync } from 'child_process'
import os from 'os'

/**
 * Installs Mint as a systemd user service
 */
export async function installDaemon() {
    if (process.platform !== 'linux') {
        throw new Error('Daemon installation is currently only supported on Linux (systemd).')
    }

    const homeDir = os.homedir()
    const serviceDir = path.join(homeDir, '.config', 'systemd', 'user')
    const servicePath = path.join(serviceDir, 'mint-agent.service')

    // Create systemd user directory if it doesn't exist
    if (!fs.existsSync(serviceDir)) {
        fs.mkdirSync(serviceDir, { recursive: true })
    }

    const nodePath = execSync('which node').toString().trim() || '/usr/bin/node'
    const projectPath = path.resolve(__dirname, '../../')
    // Points to compiled CLI path
    const cliPath = path.join(projectPath, 'dist', 'mint-cli.js')

    const serviceContent = `[Unit]
Description=Mint AI Background Agent
After=network.target

[Service]
Type=simple
WorkingDirectory=${projectPath}
ExecStart=${nodePath} ${cliPath} agent
Restart=always
RestartSec=10

[Install]
WantedBy=default.target
`

    fs.writeFileSync(servicePath, serviceContent)

    try {
        console.log('[Daemon] Reloading systemd user daemon...')
        execSync('systemctl --user daemon-reload')
        
        console.log(`[Daemon] Enabling and starting mint-agent.service...`)
        execSync('systemctl --user enable mint-agent.service')
        execSync('systemctl --user start mint-agent.service')

        return `Mint Agent installed and started! Check logs with: journalctl --user -u mint-agent -f`
    } catch (err: any) {
        throw new Error(`Failed to configure systemd: ${err.message}`)
    }
}

export async function stopDaemon() {
    try {
        execSync('systemctl --user stop mint-agent.service')
        return "Daemon stopped."
    } catch (err: any) {
        throw new Error(`Failed to stop daemon: ${err.message}`)
    }
}
