/**
 * Tests: file_operations helpers
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

describe('file_operations findPath', () => {
    let tempDir;
    let originalCwd;

    beforeEach(() => {
        jest.resetModules();
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-file-ops-'));
        originalCwd = process.cwd();
        process.chdir(tempDir);
    });

    afterEach(() => {
        process.chdir(originalCwd);
        fs.rmSync(tempDir, { recursive: true, force: true });
    });

    test('finds directory matches by name', () => {
        const targetDir = path.join(tempDir, 'nested', 'xidaidai');
        fs.mkdirSync(targetDir, { recursive: true });

        const { findPath } = require('../src/Automation_Layer/file_operations');
        const result = findPath('xidaidai', { type: 'dir', maxResults: 10, roots: [tempDir] });

        expect(result.success).toBe(true);
        expect(result.matches.some(match => match.path === targetDir && match.type === 'dir')).toBe(true);
    });

    test('returns not found message when no path matches', () => {
        const { findPath } = require('../src/Automation_Layer/file_operations');
        const result = findPath('does-not-exist', { type: 'dir', maxResults: 10, roots: [tempDir] });

        expect(result.success).toBe(false);
        expect(result.message).toContain('ไม่พบ');
    });

    test('prefers exact directory name matches over nested partial matches', () => {
        const exactDir = path.join(tempDir, 'xidaidai');
        const nestedPartial = path.join(tempDir, 'xidaidai collection', 'xidaidai gif');
        fs.mkdirSync(exactDir, { recursive: true });
        fs.mkdirSync(nestedPartial, { recursive: true });

        const { findPath } = require('../src/Automation_Layer/file_operations');
        const result = findPath('xidaidai', { type: 'dir', maxResults: 10, roots: [tempDir] });

        expect(result.success).toBe(true);
        expect(result.matches.length).toBe(1);
        expect(result.matches[0].path).toBe(exactDir);
    });
});
