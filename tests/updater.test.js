const {
    compareVersions,
    normalizeNpmVersionOutput,
    shouldRunAutoUpdate,
    _private
} = require('../dist/src/CLI/updater');

describe('Mint updater', () => {
    test('compares semantic versions', () => {
        expect(compareVersions('1.5.0', '1.5.1')).toBeLessThan(0);
        expect(compareVersions('1.6.0', '1.5.9')).toBeGreaterThan(0);
        expect(compareVersions('1.5.0', '1.5')).toBe(0);
        expect(compareVersions('v2.0.0', '1.9.9')).toBeGreaterThan(0);
    });

    test('normalizes npm version output', () => {
        expect(normalizeNpmVersionOutput('"1.5.1"\n')).toBe('1.5.1');
        expect(normalizeNpmVersionOutput('1.5.1\n')).toBe('1.5.1');
    });

    test('uses auto-update cooldown settings', () => {
        const now = Date.parse('2026-05-14T12:00:00.000Z');

        expect(shouldRunAutoUpdate({ enableAutoUpdate: false }, now)).toBe(false);
        expect(shouldRunAutoUpdate({ enableAutoUpdate: true, lastUpdateCheckAt: '' }, now)).toBe(true);
        expect(shouldRunAutoUpdate({
            enableAutoUpdate: true,
            autoUpdateCheckIntervalHours: 24,
            lastUpdateCheckAt: '2026-05-14T00:00:00.000Z'
        }, now)).toBe(false);
        expect(shouldRunAutoUpdate({
            enableAutoUpdate: true,
            autoUpdateCheckIntervalHours: 6,
            lastUpdateCheckAt: '2026-05-14T00:00:00.000Z'
        }, now)).toBe(true);
    });

    test('distinguishes failed update checks from failed installs', () => {
        const error = new Error('Command failed: npm view @pheem49/mint version --json');
        error.stderr = 'npm ERR! code E404\nnpm ERR! 404 Not Found';

        expect(_private.formatUpdateCheckError(error)).toContain('Update check unavailable');
        expect(_private.formatUpdateCheckError(error)).toContain('does not mean your local Mint version is outdated');
        expect(_private.formatUpdateError(error)).toContain('Could not find @pheem49/mint');
    });
});
