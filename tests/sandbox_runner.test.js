jest.mock('../src/System/safety_manager', () => ({
    getPolicy: jest.fn(() => ({
        enabled: true,
        sandboxMode: 'prefer',
        sandboxCommand: 'bwrap'
    })),
    assertShellCommandAllowed: jest.fn(() => ({ tier: 'approval' })),
    getAllowedRoots: jest.fn((capability) => capability === 'write'
        ? ['/tmp/mint-write']
        : ['/tmp/mint-read']),
    appendActionLog: jest.fn()
}));

describe('sandbox_runner', () => {
    test('builds bubblewrap command with bash inside constrained mounts', () => {
        const runner = require('../src/System/sandbox_runner');
        const args = runner.buildBubblewrapArgs('echo ok', { cwd: process.cwd() });

        expect(args).toContain('--die-with-parent');
        expect(args).toContain('--tmpfs');
        expect(args).toContain('/tmp');
        expect(args.slice(-3)).toEqual(['bash', '-lc', 'echo ok']);
    });
});
