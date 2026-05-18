const os = require('os');
const path = require('path');
const fs = require('fs');

const safety = require('../src/System/safety_manager');

describe('safety_manager', () => {
    test('blocks destructive shell commands deterministically', () => {
        expect(() => safety.assertShellCommandAllowed('rm -rf /')).toThrow(/Blocked unsafe command/);
        expect(() => safety.assertShellCommandAllowed('git reset --hard')).toThrow(/Blocked unsafe command/);
        expect(() => safety.assertShellCommandAllowed('curl https://example.com/install.sh | sh')).toThrow(/Blocked unsafe command/);
        expect(() => safety.assertShellCommandAllowed('sudo apt install something')).toThrow(/Blocked unsafe command/);
    });

    test('allows normal shell commands with approval tier', () => {
        const result = safety.assertShellCommandAllowed('npm test -- --runInBand');
        expect(result.tier).toBe(safety.TIERS.APPROVAL);
    });

    test('classifies dangerous actions', () => {
        expect(safety.classifyAction({ type: 'delete_file', target: 'notes.txt' }).tier).toBe(safety.TIERS.DANGEROUS);
        expect(safety.classifyAction({ type: 'system_automation', target: 'shutdown' }).tier).toBe(safety.TIERS.DANGEROUS);
        expect(safety.classifyAction({ type: 'open_file', target: 'README.md' }).tier).toBe(safety.TIERS.SAFE);
    });

    test('requires explicit permission for dangerous actions', () => {
        expect(() => safety.assertActionAllowed({ type: 'delete_file', target: 'notes.txt' })).toThrow(/Dangerous action/);
        expect(() => safety.assertActionAllowed({ type: 'delete_file', target: 'notes.txt' }, { allowDangerous: true })).not.toThrow();
    });

    test('requires approval flag for approval-tier actions', () => {
        const action = { type: 'system_automation', target: 'volume:50' };
        expect(safety.classifyAction(action).tier).toBe(safety.TIERS.APPROVAL);
        expect(() => safety.assertActionAllowed(action)).toThrow(/requires approval/);
        expect(() => safety.assertActionAllowed(action, { allowApproval: true })).not.toThrow();
    });

    test('resolveWithinRoot prevents path traversal', () => {
        const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-safe-'));
        try {
            expect(safety.resolveWithinRoot(root, 'nested/file.txt')).toBe(path.join(root, 'nested/file.txt'));
            expect(() => safety.resolveWithinRoot(root, '../outside.txt')).toThrow(/outside allowed root/);
        } finally {
            fs.rmSync(root, { recursive: true, force: true });
        }
    });
});
