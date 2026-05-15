/**
 * Tests: gmail_auth.js OAuth helper
 */

jest.mock('axios', () => ({
    post: jest.fn()
}));

jest.mock('../src/System/config_manager', () => ({
    readConfig: jest.fn(),
    writeConfig: jest.fn()
}));

const axios = require('axios');
const gmailAuth = require('../src/CLI/gmail_auth');

describe('gmail_auth helper', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    test('buildAuthUrl includes offline access and consent prompt', () => {
        const url = gmailAuth.buildAuthUrl({
            clientId: 'client-id',
            redirectUri: 'http://127.0.0.1:3333/oauth2callback',
            state: 'state-1'
        });

        const parsed = new URL(url);
        expect(parsed.origin + parsed.pathname).toBe('https://accounts.google.com/o/oauth2/v2/auth');
        expect(parsed.searchParams.get('client_id')).toBe('client-id');
        expect(parsed.searchParams.get('access_type')).toBe('offline');
        expect(parsed.searchParams.get('prompt')).toBe('consent');
        expect(parsed.searchParams.get('scope')).toContain('gmail.readonly');
        expect(parsed.searchParams.get('scope')).toContain('gmail.compose');
    });

    test('exchanges authorization code for token', async () => {
        axios.post.mockResolvedValueOnce({
            data: {
                refresh_token: 'refresh-token'
            }
        });

        const token = await gmailAuth.exchangeCodeForToken({
            clientId: 'client-id',
            clientSecret: 'client-secret',
            code: 'code-1',
            redirectUri: 'http://127.0.0.1:3333/oauth2callback'
        });

        expect(token.refresh_token).toBe('refresh-token');
        expect(axios.post).toHaveBeenCalledWith(
            'https://oauth2.googleapis.com/token',
            expect.stringContaining('grant_type=authorization_code'),
            expect.any(Object)
        );
    });

    test('runGmailAuth opens browser, accepts callback, and saves refresh token', async () => {
        axios.post.mockResolvedValueOnce({
            data: {
                refresh_token: 'new-refresh-token'
            }
        });

        const writeConfig = jest.fn(() => ({ success: true }));
        const result = await gmailAuth.runGmailAuth({
            readConfig: () => ({
                gmailClientId: 'client-id',
                gmailClientSecret: 'client-secret',
                gmailUserId: 'me'
            }),
            writeConfig,
            logger: { log: jest.fn() },
            openBrowser: jest.fn(),
            getAuthorizationCode: async ({ authUrl, state, redirectUri }) => {
                expect(authUrl).toContain(encodeURIComponent(redirectUri));
                expect(state).toBeTruthy();
                return 'auth-code';
            },
            timeoutMs: 5000
        });

        expect(result.success).toBe(true);
        expect(writeConfig).toHaveBeenCalledWith(expect.objectContaining({
            gmailRefreshToken: 'new-refresh-token',
            gmailUserId: 'me',
            pluginGmailEnabled: true
        }));
    });

    test('runGmailAuth can print link without opening browser', async () => {
        axios.post.mockResolvedValueOnce({
            data: {
                refresh_token: 'manual-refresh-token'
            }
        });

        const openBrowser = jest.fn();
        const writeConfig = jest.fn(() => ({ success: true }));

        const result = await gmailAuth.runGmailAuth({
            readConfig: () => ({
                gmailClientId: 'client-id',
                gmailClientSecret: 'client-secret',
                gmailUserId: 'me'
            }),
            writeConfig,
            logger: { log: jest.fn() },
            openBrowser: false,
            getAuthorizationCode: async () => 'manual-code'
        });

        expect(result.success).toBe(true);
        expect(openBrowser).not.toHaveBeenCalled();
        expect(writeConfig).toHaveBeenCalledWith(expect.objectContaining({
            gmailRefreshToken: 'manual-refresh-token'
        }));
    });

    test('runGmailAuth requires saved client credentials', async () => {
        await expect(gmailAuth.runGmailAuth({
            readConfig: () => ({}),
            openBrowser: jest.fn(),
            logger: { log: jest.fn() }
        })).rejects.toThrow('Missing Gmail OAuth Client ID');
    });
});
