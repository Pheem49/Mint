/**
 * Tests: google_calendar.js plugin
 */

jest.mock('axios', () => ({
    post: jest.fn(),
    get: jest.fn()
}));

jest.mock('electron', () => ({
    shell: {
        openExternal: jest.fn()
    }
}));

jest.mock('../dist/src/System/config_manager', () => ({
    readConfig: jest.fn()
}));

const axios = require('axios');
const { shell } = require('electron');
const { readConfig } = require('../dist/src/System/config_manager');

describe('google_calendar plugin', () => {
    let plugin;

    beforeEach(() => {
        jest.clearAllMocks();
        plugin = require('../dist/src/Plugins/google_calendar');
    });

    test('has required plugin fields', () => {
        expect(plugin.name).toBe('google_calendar');
        expect(typeof plugin.description).toBe('string');
        expect(typeof plugin.execute).toBe('function');
    });

    test('falls back to opening Google Calendar when API config is missing', async () => {
        readConfig.mockReturnValue({});

        const result = await plugin.execute('open');

        expect(shell.openExternal).toHaveBeenCalledWith('https://calendar.google.com/');
        expect(result).toContain('Google Calendar');
    });

    test('creates event through Google Calendar API', async () => {
        readConfig.mockReturnValue({
            googleCalendarClientId: 'client-id',
            googleCalendarClientSecret: 'client-secret',
            googleCalendarRefreshToken: 'refresh-token',
            googleCalendarId: 'primary'
        });
        axios.post
            .mockResolvedValueOnce({ data: { access_token: 'access-token' } })
            .mockResolvedValueOnce({
                data: {
                    summary: 'Demo meeting',
                    htmlLink: 'https://calendar.google.com/event/demo'
                }
            });

        const result = await plugin.execute(JSON.stringify({
            action: 'create',
            summary: 'Demo meeting',
            start: '2026-05-15T10:00:00+07:00',
            end: '2026-05-15T11:00:00+07:00'
        }));

        expect(axios.post).toHaveBeenCalledTimes(2);
        expect(axios.post.mock.calls[1][0]).toContain('/calendars/primary/events');
        expect(axios.post.mock.calls[1][1].summary).toBe('Demo meeting');
        expect(result).toContain('Demo meeting');
        expect(result).toContain('https://calendar.google.com/event/demo');
    });

    test('lists upcoming events through Google Calendar API', async () => {
        readConfig.mockReturnValue({
            googleCalendarClientId: 'client-id',
            googleCalendarClientSecret: 'client-secret',
            googleCalendarRefreshToken: 'refresh-token',
            googleCalendarId: 'primary'
        });
        axios.post.mockResolvedValueOnce({ data: { access_token: 'access-token' } });
        axios.get.mockResolvedValueOnce({
            data: {
                items: [
                    {
                        summary: 'Planning',
                        start: { dateTime: '2026-05-15T10:00:00+07:00' },
                        end: { dateTime: '2026-05-15T11:00:00+07:00' }
                    }
                ]
            }
        });

        const result = await plugin.execute(JSON.stringify({ action: 'list', days: 3 }));

        expect(axios.get).toHaveBeenCalledTimes(1);
        expect(axios.get.mock.calls[0][0]).toContain('/calendars/primary/events');
        expect(result).toContain('Planning');
    });

    test('builds all-day event payload with exclusive end date', () => {
        const payload = plugin._helpers.buildEventPayload({
            summary: 'All day',
            date: '2026-05-15'
        });

        expect(payload.start).toEqual({ date: '2026-05-15' });
        expect(payload.end).toEqual({ date: '2026-05-16' });
    });
});
