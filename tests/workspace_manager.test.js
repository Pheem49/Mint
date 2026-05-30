/**
 * Tests: workspace_manager.js
 */

const wsManager = require('../dist/src/CLI/workspace_manager');
const fs = require('fs');
const path = require('path');
const os = require('os');

describe('Workspace Manager', () => {
    let tempDir;
    let workspaceFile;

    beforeEach(() => {
        // Create a temp workspace directory
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-ws-test-'));
        workspaceFile = path.join(tempDir, 'workspaces.json');
        process.env.MINT_WORKSPACE_FILE = workspaceFile;
    });

    afterEach(() => {
        delete process.env.MINT_WORKSPACE_FILE;
        fs.rmSync(tempDir, { recursive: true, force: true });
    });

    test('adds and lists workspaces', () => {
        wsManager.addWorkspace('test-ws', tempDir, 'Be careful with files');
        const list = wsManager.listWorkspaces();
        expect(list['test-ws']).toBeDefined();
        expect(list['test-ws'].path).toBe(path.resolve(tempDir));
    });

    test('detects workspace by path', () => {
        wsManager.addWorkspace('test-ws', tempDir);
        const ws = wsManager.getWorkspaceByPath(tempDir);
        expect(ws.name).toBe('test-ws');
    });

    test('does not match sibling paths with same prefix', () => {
        const workspaceRoot = path.join(tempDir, 'project');
        const siblingPath = path.join(tempDir, 'project-two');
        fs.mkdirSync(workspaceRoot, { recursive: true });
        fs.mkdirSync(siblingPath, { recursive: true });

        wsManager.addWorkspace('test-ws', workspaceRoot);
        const ws = wsManager.getWorkspaceByPath(siblingPath);
        expect(ws).toBeNull();
    });

    test('removes workspaces', () => {
        wsManager.addWorkspace('test-ws', tempDir);
        wsManager.removeWorkspace('test-ws');
        const list = wsManager.listWorkspaces();
        expect(list['test-ws']).toBeUndefined();
    });
});
