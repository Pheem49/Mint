import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';
import { CONFIG_PATH } from '../System/config_manager';

export const SESSION_FILE = path.join(path.dirname(CONFIG_PATH), 'code-sessions.json');

export interface WorkspaceSession {
    workspaceRoot: string;
    summary: string;
    lastTask: string;
    lastVerification: string;
    updatedAt: string | null;
    [key: string]: any;
}

function ensureSessionStore(): void {
    if (!fs.existsSync(SESSION_FILE)) {
        fs.writeFileSync(SESSION_FILE, JSON.stringify({}, null, 2), 'utf8');
    }
}

function readAllSessions(): Record<string, WorkspaceSession> {
    ensureSessionStore();
    try {
        return JSON.parse(fs.readFileSync(SESSION_FILE, 'utf8'));
    } catch (error) {
        return {};
    }
}

function writeAllSessions(data: Record<string, WorkspaceSession>): void {
    ensureSessionStore();
    fs.writeFileSync(SESSION_FILE, JSON.stringify(data, null, 2), 'utf8');
}

function getWorkspaceKey(workspaceRoot: string): string {
    return crypto.createHash('sha1').update(path.resolve(workspaceRoot)).digest('hex');
}

export function readWorkspaceSession(workspaceRoot: string): WorkspaceSession {
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

export function writeWorkspaceSession(workspaceRoot: string, updates: Partial<WorkspaceSession>): WorkspaceSession {
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
