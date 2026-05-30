import axios from 'axios'
import { readConfig  } from '../System/config_manager'

const TOKEN_URL = 'https://oauth2.googleapis.com/token';
const GMAIL_API_BASE = 'https://gmail.googleapis.com/gmail/v1';

function hasGmailConfig(config) {
    return Boolean(config.gmailClientId && config.gmailClientSecret && config.gmailRefreshToken);
}

function parseInstruction(instruction) {
    const raw = (instruction || '').trim();
    if (!raw) return { action: 'help' };

    try {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === 'object') {
            return {
                action: normalizeAction(parsed.action || 'search'),
                ...parsed
            };
        }
    } catch {
        // Plain text searches Gmail.
    }

    const lower = raw.toLowerCase();
    if (lower === 'help') return { action: 'help' };
    if (lower === 'unread') return { action: 'search', query: 'is:unread' };
    if (lower === 'inbox') return { action: 'search', query: 'in:inbox' };
    if (lower.startsWith('read ')) return { action: 'read', id: raw.slice(5).trim() };
    if (lower.startsWith('draft ')) return { action: 'draft', body: raw.slice(6).trim() };
    if (lower.startsWith('search ')) return { action: 'search', query: raw.slice(7).trim() };

    return { action: 'search', query: raw };
}

function normalizeAction(action) {
    const normalized = String(action || '').toLowerCase();
    if (['list', 'search', 'inbox', 'unread'].includes(normalized)) return 'search';
    if (['get', 'read', 'read_email', 'message'].includes(normalized)) return 'read';
    if (['draft', 'create_draft', 'compose', 'write'].includes(normalized)) return 'draft';
    return normalized;
}

function gmailUserId(config) {
    return encodeURIComponent(config.gmailUserId || 'me');
}

async function getAccessToken(config) {
    const params = new URLSearchParams({
        client_id: config.gmailClientId,
        client_secret: config.gmailClientSecret,
        refresh_token: config.gmailRefreshToken,
        grant_type: 'refresh_token'
    });

    const response = await axios.post(TOKEN_URL, params.toString(), {
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' }
    });

    return response.data.access_token;
}

function gmailHeaders(accessToken) {
    return {
        Authorization: `Bearer ${accessToken}`,
        'Content-Type': 'application/json'
    };
}

function decodeBase64Url(data = '') {
    const normalized = String(data).replace(/-/g, '+').replace(/_/g, '/');
    const padded = normalized.padEnd(normalized.length + ((4 - normalized.length % 4) % 4), '=');
    return Buffer.from(padded, 'base64').toString('utf8');
}

function encodeBase64Url(data = '') {
    return Buffer.from(String(data), 'utf8')
        .toString('base64')
        .replace(/\+/g, '-')
        .replace(/\//g, '_')
        .replace(/=+$/g, '');
}

function getHeader(message, name) {
    const headers = message.payload?.headers || [];
    const found = headers.find(header => header.name && header.name.toLowerCase() === name.toLowerCase());
    return found ? found.value : '';
}

function findTextPart(payload) {
    if (!payload) return '';
    if (payload.mimeType === 'text/plain' && payload.body?.data) {
        return decodeBase64Url(payload.body.data);
    }
    if (payload.mimeType === 'text/html' && payload.body?.data) {
        return decodeBase64Url(payload.body.data).replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
    }
    for (const part of payload.parts || []) {
        const text = findTextPart(part);
        if (text) return text;
    }
    return payload.body?.data ? decodeBase64Url(payload.body.data) : '';
}

function formatMessageSummary(message) {
    const subject = getHeader(message, 'Subject') || '(No subject)';
    const from = getHeader(message, 'From') || '(Unknown sender)';
    const date = getHeader(message, 'Date');
    const snippet = message.snippet || '';
    return [
        `ID: ${message.id}`,
        `From: ${from}`,
        `Subject: ${subject}`,
        date ? `Date: ${date}` : '',
        snippet ? `Snippet: ${snippet}` : ''
    ].filter(Boolean).join('\n');
}

async function fetchMessage(config, accessToken, id, format = 'metadata') {
    const response = await axios.get(`${GMAIL_API_BASE}/users/${gmailUserId(config)}/messages/${encodeURIComponent(id)}`, {
        headers: gmailHeaders(accessToken),
        params: {
            format,
            metadataHeaders: ['From', 'To', 'Subject', 'Date']
        }
    });
    return response.data;
}

async function searchMessages(config, input, accessToken) {
    const query = input.query || input.q || 'in:inbox';
    const maxResults = Number(input.maxResults || input.limit || 10);
    const response = await axios.get(`${GMAIL_API_BASE}/users/${gmailUserId(config)}/messages`, {
        headers: gmailHeaders(accessToken),
        params: {
            q: query,
            maxResults
        }
    });

    const messages = response.data.messages || [];
    if (messages.length === 0) return `No Gmail messages found for query: ${query}`;

    const detailed = [];
    for (const message of messages.slice(0, maxResults)) {
        const full = await fetchMessage(config, accessToken, message.id, 'metadata');
        detailed.push(formatMessageSummary(full));
    }

    return `Gmail search results for "${query}":\n\n${detailed.join('\n\n')}`;
}

async function readMessage(config, input, accessToken) {
    const id = input.id || input.messageId;
    if (!id) throw new Error('Missing Gmail message id.');

    const message = await fetchMessage(config, accessToken, id, 'full');
    const body = findTextPart(message.payload);
    return [
        formatMessageSummary(message),
        '',
        body ? `Body:\n${body.slice(0, Number(input.maxChars || 4000))}` : 'Body: (No readable text body found)'
    ].join('\n');
}

function sanitizeHeader(value = '') {
    return String(value).replace(/[\r\n]+/g, ' ').trim();
}

function buildRawEmail(input) {
    const to = sanitizeHeader(input.to || input.recipient || '');
    if (!to) throw new Error('Missing email recipient.');

    const cc = sanitizeHeader(input.cc || '');
    const bcc = sanitizeHeader(input.bcc || '');
    const subject = sanitizeHeader(input.subject || '(No subject)');
    const body = String(input.body || input.content || input.text || '');

    const headers = [
        `To: ${to}`,
        cc ? `Cc: ${cc}` : '',
        bcc ? `Bcc: ${bcc}` : '',
        `Subject: ${subject}`,
        'MIME-Version: 1.0',
        'Content-Type: text/plain; charset="UTF-8"'
    ].filter(Boolean);

    return encodeBase64Url(`${headers.join('\r\n')}\r\n\r\n${body}`);
}

async function createDraft(config, input, accessToken) {
    const raw = buildRawEmail(input);
    const response = await axios.post(`${GMAIL_API_BASE}/users/${gmailUserId(config)}/drafts`, {
        message: { raw }
    }, {
        headers: gmailHeaders(accessToken)
    });

    const draft = response.data || {};
    return `Created Gmail draft${draft.id ? ` ${draft.id}` : ''} for ${sanitizeHeader(input.to || input.recipient)}. Review it in Gmail before sending.`;
}

function helpText() {
    return [
        'Gmail plugin commands:',
        '- Search inbox: {"action":"search","query":"in:inbox newer_than:7d","limit":5}',
        '- Read message: {"action":"read","id":"MESSAGE_ID"}',
        '- Create draft: {"action":"draft","to":"person@example.com","subject":"Hello","body":"Draft body"}',
        'For safety, this plugin creates drafts only. It does not send email automatically.'
    ].join('\n');
}

const plugin = {
    name: 'gmail',
    description: 'Manage Gmail safely. Target can be JSON: {"action":"search","query":"in:inbox is:unread","limit":10 }, {"action":"read","id":"MESSAGE_ID"}, or {"action":"draft","to":"person@example.com","subject":"Subject","body":"Body"}. This plugin creates drafts only and does not send email.',

    async execute(instruction: any) {
        const config = readConfig();
        const input = parseInstruction(instruction);

        if (input.action === 'help') return helpText();
        if (!hasGmailConfig(config)) {
            return 'Gmail API is not configured. Add Gmail OAuth credentials with `mint onboard`. Use scopes for gmail.readonly and gmail.compose.';
        }

        const accessToken = await getAccessToken(config);

        switch (input.action) {
            case 'search':
                return await searchMessages(config, input, accessToken);
            case 'read':
                return await readMessage(config, input, accessToken);
            case 'draft':
                return await createDraft(config, input, accessToken);
            default:
                throw new Error(`Unsupported Gmail action: ${input.action}`);
        }
    },

    _helpers: {
        parseInstruction,
        buildRawEmail,
        decodeBase64Url,
        encodeBase64Url,
        findTextPart,
        formatMessageSummary,
        hasGmailConfig
    }
};

export = plugin;
