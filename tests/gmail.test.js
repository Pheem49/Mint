/**
 * Tests: gmail.js plugin
 */

jest.mock('axios', () => ({
    post: jest.fn(),
    get: jest.fn()
}));

jest.mock('../dist/src/System/config_manager', () => ({
    readConfig: jest.fn()
}));

const axios = require('axios');
const { readConfig } = require('../dist/src/System/config_manager');
const gmail = require('../dist/src/Plugins/gmail');

describe('gmail plugin', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    test('has required plugin fields', () => {
        expect(gmail.name).toBe('gmail');
        expect(typeof gmail.description).toBe('string');
        expect(typeof gmail.execute).toBe('function');
    });

    test('returns configuration message when OAuth config is missing', async () => {
        readConfig.mockReturnValue({});

        const result = await gmail.execute('inbox');

        expect(result).toContain('Gmail API is not configured');
        expect(axios.get).not.toHaveBeenCalled();
    });

    test('searches Gmail and fetches metadata for results', async () => {
        readConfig.mockReturnValue({
            gmailClientId: 'client-id',
            gmailClientSecret: 'client-secret',
            gmailRefreshToken: 'refresh-token',
            gmailUserId: 'me'
        });
        axios.post.mockResolvedValueOnce({ data: { access_token: 'access-token' } });
        axios.get
            .mockResolvedValueOnce({ data: { messages: [{ id: 'msg-1' }] } })
            .mockResolvedValueOnce({
                data: {
                    id: 'msg-1',
                    snippet: 'Hello snippet',
                    payload: {
                        headers: [
                            { name: 'From', value: 'A <a@example.com>' },
                            { name: 'Subject', value: 'Hello' },
                            { name: 'Date', value: 'Fri, 15 May 2026 10:00:00 +0700' }
                        ]
                    }
                }
            });

        const result = await gmail.execute(JSON.stringify({ action: 'search', query: 'is:unread', limit: 1 }));

        expect(axios.get).toHaveBeenCalledTimes(2);
        expect(axios.get.mock.calls[0][0]).toBe('https://gmail.googleapis.com/gmail/v1/users/me/messages');
        expect(result).toContain('Hello');
        expect(result).toContain('msg-1');
    });

    test('reads full Gmail message body', async () => {
        readConfig.mockReturnValue({
            gmailClientId: 'client-id',
            gmailClientSecret: 'client-secret',
            gmailRefreshToken: 'refresh-token',
            gmailUserId: 'me'
        });
        axios.post.mockResolvedValueOnce({ data: { access_token: 'access-token' } });
        axios.get.mockResolvedValueOnce({
            data: {
                id: 'msg-1',
                payload: {
                    headers: [
                        { name: 'From', value: 'A <a@example.com>' },
                        { name: 'Subject', value: 'Hello' }
                    ],
                    mimeType: 'text/plain',
                    body: { data: gmail._helpers.encodeBase64Url('Full body text') }
                }
            }
        });

        const result = await gmail.execute(JSON.stringify({ action: 'read', id: 'msg-1' }));

        expect(result).toContain('Full body text');
        expect(result).toContain('Hello');
    });

    test('creates Gmail draft only', async () => {
        readConfig.mockReturnValue({
            gmailClientId: 'client-id',
            gmailClientSecret: 'client-secret',
            gmailRefreshToken: 'refresh-token',
            gmailUserId: 'me'
        });
        axios.post
            .mockResolvedValueOnce({ data: { access_token: 'access-token' } })
            .mockResolvedValueOnce({ data: { id: 'draft-1' } });

        const result = await gmail.execute(JSON.stringify({
            action: 'draft',
            to: 'person@example.com',
            subject: 'Draft subject',
            body: 'Draft body'
        }));

        expect(axios.post).toHaveBeenCalledTimes(2);
        expect(axios.post.mock.calls[1][0]).toBe('https://gmail.googleapis.com/gmail/v1/users/me/drafts');
        expect(axios.post.mock.calls[1][1].message.raw).toBeTruthy();
        expect(result).toContain('Created Gmail draft');
        expect(result).toContain('Review it in Gmail before sending');
    });

    test('buildRawEmail removes newline injection from headers', () => {
        const raw = gmail._helpers.buildRawEmail({
            to: 'person@example.com\nBcc: attacker@example.com',
            subject: 'Hello\r\nBad: header',
            body: 'Body'
        });
        const decoded = gmail._helpers.decodeBase64Url(raw);

        expect(decoded).toContain('To: person@example.com Bcc: attacker@example.com');
        expect(decoded).toContain('Subject: Hello Bad: header');
        expect(decoded).not.toContain('\nBcc: attacker@example.com');
    });
});
