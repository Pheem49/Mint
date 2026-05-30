const {
    getBrowserContext,
    isBrowserProcess,
    truncateText
} = require('../dist/src/System/smart_context');

describe('smart context helpers', () => {
    test('detects common browser process names', () => {
        expect(isBrowserProcess('firefox')).toBe(true);
        expect(isBrowserProcess('chromium-browser')).toBe(true);
        expect(isBrowserProcess('Code')).toBe(false);
    });

    test('truncates long text for context payloads', () => {
        const result = truncateText('a'.repeat(12), 5);

        expect(result).toBe('aaaaa\n[truncated 7 chars]');
    });

    test('returns browser title with unavailable URL reason when OS URL access is unavailable', async () => {
        const context = await getBrowserContext({
            appName: 'firefox',
            processName: 'firefox',
            title: 'Example Page - Mozilla Firefox'
        }, 'linux');

        expect(context).toMatchObject({
            title: 'Example Page - Mozilla Firefox',
            url: '',
            urlUnavailableReason: expect.stringContaining('not available')
        });
    });

    test('returns null browser context for non-browser apps', async () => {
        await expect(getBrowserContext({
            appName: 'code',
            processName: 'code',
            title: 'renderer.js'
        }, 'linux')).resolves.toBeNull();
    });
});
