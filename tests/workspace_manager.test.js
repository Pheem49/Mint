/**
 * Tests: workspace_manager.js
 */

const wsManager = require('../src/CLI/workspace_manager');
const fs = require('fs');
const path = require('path');
const os = require('os');

describe('Workspace Manager', () => {
    let tempDir;

    beforeEach(() => {
        // Create a temp workspace directory
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-ws-test-'));
    });

    afterEach(() => {
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

    test('removes workspaces', () => {
        wsManager.addWorkspace('test-ws', tempDir);
        wsManager.removeWorkspace('test-ws');
        const list = wsManager.listWorkspaces();
        expect(list['test-ws']).toBeUndefined();
    });
});
