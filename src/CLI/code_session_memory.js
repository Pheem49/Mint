const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { CONFIG_PATH } = require('../System/config_manager');

const SESSION_FILE = path.join(path.dirname(CONFIG_PATH), 'code-sessions.json');

function ensureSessionStore() {
    if (!fs.existsSync(SESSION_FILE)) {
        fs.writeFileSync(SESSION_FILE, JSON.stringify({}, null, 2), 'utf8');
    }
}

function readAllSessions() {
    ensureSessionStore();
    try {
        return JSON.parse(fs.readFileSync(SESSION_FILE, 'utf8'));
    } catch (error) {
        return {};
    }
}

function writeAllSessions(data) {
    ensureSessionStore();
    fs.writeFileSync(SESSION_FILE, JSON.stringify(data, null, 2), 'utf8');
}

function getWorkspaceKey(workspaceRoot) {
    return crypto.createHash('sha1').update(path.resolve(workspaceRoot)).digest('hex');
}

function readWorkspaceSession(workspaceRoot) {
    const sessions = readAllSessions();
    const key = getWorkspaceKey(workspaceRoot);
    return sessions[key] || {
        workspaceRoot: path.resolve(workspaceRoot),
        summary: '',
        lastTask: '',
        lastVerification: '',
        updatedAt: null
    };
}

function writeWorkspaceSession(workspaceRoot, updates) {
    const sessions = readAllSessions();
    const key = getWorkspaceKey(workspaceRoot);
    const current = readWorkspaceSession(workspaceRoot);
    sessions[key] = {
        ...current,
        ...updates,
        workspaceRoot: path.resolve(workspaceRoot),
        updatedAt: new Date().toISOString()
    };
    writeAllSessions(sessions);
    return sessions[key];
}

module.exports = {
    readWorkspaceSession,
    writeWorkspaceSession,
    SESSION_FILE
};
