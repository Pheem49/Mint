import axios from 'axios'
import { readConfig  } from '../System/config_manager'

let shell = null;
try {
    ({ shell } = require('electron'));
} catch {
    shell = null;
}

const TOKEN_URL = 'https://oauth2.googleapis.com/token';
const CALENDAR_API_BASE = 'https://www.googleapis.com/calendar/v3';

function hasCalendarApiConfig(config) {
    return Boolean(
        config.googleCalendarClientId &&
        config.googleCalendarClientSecret &&
        config.googleCalendarRefreshToken
    );
}

function parseInstruction(instruction) {
    const raw = (instruction || '').trim();
    if (!raw) return { action: 'open' };

    try {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === 'object') {
            return {
                action: (parsed.action || 'create').toLowerCase(),
                ...parsed
            };
        }
    } catch {
        // Plain text remains supported for backward compatibility.
    }

    const lower = raw.toLowerCase();
    if (['open', 'view', 'calendar'].includes(lower)) return { action: 'open' };
    if (['today', 'list today'].includes(lower)) return { action: 'list', range: 'today' };
    if (lower.startsWith('list') || lower.startsWith('upcoming')) return { action: 'list', range: 'upcoming' };

    return { action: 'create', summary: raw };
}

async function getAccessToken(config) {
    const params = new URLSearchParams({
        client_id: config.googleCalendarClientId,
        client_secret: config.googleCalendarClientSecret,
        refresh_token: config.googleCalendarRefreshToken,
        grant_type: 'refresh_token'
    });

    const response = await axios.post(TOKEN_URL, params.toString(), {
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' }
    });

    return response.data.access_token;
}

function getCalendarId(config) {
    return config.googleCalendarId || 'primary';
}

function getLocalDayBounds(date = new Date()) {
    const start = new Date(date);
    start.setHours(0, 0, 0, 0);

    const end = new Date(start);
    end.setDate(end.getDate() + 1);

    return { start, end };
}

function addDays(date, days) {
    const next = new Date(date);
    next.setDate(next.getDate() + days);
    return next;
}

function addDaysToIsoDate(dateString, days) {
    const [year, month, day] = dateString.split('-').map(Number);
    const date = new Date(Date.UTC(year, month - 1, day));
    date.setUTCDate(date.getUTCDate() + days);
    return date.toISOString().slice(0, 10);
}

function formatEventTime(event) {
    const start = event.start || {};
    const end = event.end || {};
    const startValue = start.dateTime || start.date;
    const endValue = end.dateTime || end.date;

    if (!startValue) return '';
    if (start.date) return startValue;

    const startText = new Date(startValue).toLocaleString('th-TH', {
        dateStyle: 'medium',
        timeStyle: 'short'
    });
    if (!endValue) return startText;

    const endText = new Date(endValue).toLocaleTimeString('th-TH', {
        hour: '2-digit',
        minute: '2-digit'
    });
    return `${startText} - ${endText}`;
}

function buildEventPayload(input: any) {
    const summary = (input.summary || input.title || input.name || '').trim();
    if (!summary) {
        throw new Error('Missing event summary/title.');
    }

    const payload: any = {
        summary,
        description: input.description || undefined,
        location: input.location || undefined
    };

    if (input.start || input.startDateTime || input.end || input.endDateTime) {
        const start = input.start || input.startDateTime;
        const end = input.end || input.endDateTime;
        if (!start) throw new Error('Missing event start time.');

        payload.start = { dateTime: new Date(start).toISOString() };
        payload.end = { dateTime: end ? new Date(end).toISOString() : new Date(new Date(start).getTime() + 60 * 60 * 1000).toISOString() };
    } else if (input.date) {
        payload.start = { date: input.date };
        payload.end = { date: input.endDate || input.date };
        if (payload.end.date === payload.start.date) {
            payload.end.date = addDaysToIsoDate(input.date, 1);
        }
    } else {
        const now = new Date();
        const start = addDays(now, 1);
        start.setHours(9, 0, 0, 0);
        payload.start = { dateTime: start.toISOString() };
        payload.end = { dateTime: new Date(start.getTime() + 60 * 60 * 1000).toISOString() };
    }

    return payload;
}

async function listEvents(config, input, accessToken) {
    const now = new Date();
    let timeMin = now;
    let timeMax = addDays(now, Number(input.days || 7));

    if (input.range === 'today') {
        const bounds = getLocalDayBounds(now);
        timeMin = bounds.start;
        timeMax = bounds.end;
    }

    if (input.timeMin) timeMin = new Date(input.timeMin);
    if (input.timeMax) timeMax = new Date(input.timeMax);

    const calendarId = encodeURIComponent(getCalendarId(config));
    const response = await axios.get(`${CALENDAR_API_BASE}/calendars/${calendarId}/events`, {
        headers: { Authorization: `Bearer ${accessToken}` },
        params: {
            singleEvents: true,
            orderBy: 'startTime',
            maxResults: Number(input.maxResults || 10),
            timeMin: timeMin.toISOString(),
            timeMax: timeMax.toISOString()
        }
    });

    const events = response.data.items || [];
    if (events.length === 0) {
        return input.range === 'today'
            ? 'No Google Calendar events found for today. 📅'
            : 'No upcoming Google Calendar events found. 📅';
    }

    const lines = events.map((event, index) => {
        const when = formatEventTime(event);
        return `${index + 1}. ${event.summary || '(Untitled)'}${when ? ` — ${when}` : ''}`;
    });

    return `Google Calendar events:\n${lines.join('\n')}`;
}

async function createEvent(config, input, accessToken) {
    const payload = buildEventPayload(input);
    const calendarId = encodeURIComponent(getCalendarId(config));
    const response = await axios.post(`${CALENDAR_API_BASE}/calendars/${calendarId}/events`, payload, {
        headers: {
            Authorization: `Bearer ${accessToken}`,
            'Content-Type': 'application/json'
        }
    });

    const event = response.data || {};
    return `Created "${event.summary || payload.summary}" in Google Calendar. 📅${event.htmlLink ? `\n${event.htmlLink}` : ''}`;
}

function openCalendarFallback(input) {
    if (!shell || typeof shell.openExternal !== 'function') {
        return 'Google Calendar API is not configured, and this environment cannot open a browser.';
    }

    if (input.action === 'open') {
        shell.openExternal('https://calendar.google.com/');
        return 'Opening Google Calendar. 📅';
    }

    const title = encodeURIComponent(input.summary || input.title || input.name || 'New event');
    const url = `https://calendar.google.com/calendar/r/eventedit?text=${title}`;
    shell.openExternal(url);
    return `Google Calendar API is not configured, so I opened the event creation page for "${decodeURIComponent(title)}" instead. 📅`;
}

const plugin = {
    name: 'google_calendar',
    description: 'Manage Google Calendar. Target can be JSON: {"action":"list","range":"today|upcoming","days":7 } or {"action":"create","summary":"Meeting","start":"2026-05-15T10:00:00+07:00","end":"2026-05-15T11:00:00+07:00","description":"","location":""}. Plain text creates a new event title. Use action "open" to open Calendar.',

    async execute(instruction: any) {
        const config = readConfig();
        const input = parseInstruction(instruction);

        if (!hasCalendarApiConfig(config)) {
            return openCalendarFallback(input);
        }

        const accessToken = await getAccessToken(config);

        if (input.action === 'list' || input.action === 'today' || input.action === 'upcoming') {
            return await listEvents(config, input, accessToken);
        }

        if (input.action === 'open') {
            return openCalendarFallback(input);
        }

        if (input.action === 'create' || input.action === 'add') {
            return await createEvent(config, input, accessToken);
        }

        throw new Error(`Unsupported Google Calendar action: ${input.action}`);
    },

    _helpers: {
        parseInstruction,
        buildEventPayload,
        hasCalendarApiConfig,
        addDaysToIsoDate
    }
};

export = plugin;
