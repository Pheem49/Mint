/**
 * Mint Workspace Manager
 * -----------------------
 * Manages project-specific contexts and persistent workspaces.
 * Stores data in ~/.config/mint/workspaces.json
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const WORKSPACE_FILE = path.join(os.homedir(), '.config', 'mint', 'workspaces.json');

function ensureDir() {
    const dir = path.dirname(WORKSPACE_FILE);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function loadWorkspaces() {
    ensureDir();
    if (!fs.existsSync(WORKSPACE_FILE)) return {};
    try {
        return JSON.parse(fs.readFileSync(WORKSPACE_FILE, 'utf8'));
    } catch (e) {
        return {};
    }
}

function saveWorkspaces(data) {
    ensureDir();
    fs.writeFileSync(WORKSPACE_FILE, JSON.stringify(data, null, 2));
}

function addWorkspace(name, rootPath, instructions = '') {
    const workspaces = loadWorkspaces();
    const absolutePath = path.resolve(rootPath);
    workspaces[name] = {
        name,
        path: absolutePath,
        instructions,
        addedAt: new Date().toISOString(),
        lastAccessed: new Date().toISOString()
    };
    saveWorkspaces(workspaces);
    return workspaces[name];
}

function removeWorkspace(name) {
    const workspaces = loadWorkspaces();
    if (workspaces[name]) {
        delete workspaces[name];
        saveWorkspaces(workspaces);
        return true;
    }
    return false;
}

function getWorkspaceByPath(currentPath) {
    const workspaces = loadWorkspaces();
    const absoluteCurrent = path.resolve(currentPath);
    
    // Find workspace where current path is inside or equal to workspace path
    for (const name in workspaces) {
        const ws = workspaces[name];
        if (absoluteCurrent.startsWith(ws.path)) {
            return ws;
        }
    }
    return null;
}

function listWorkspaces() {
    return loadWorkspaces();
}

module.exports = {
    addWorkspace,
    removeWorkspace,
    getWorkspaceByPath,
    listWorkspaces
};
