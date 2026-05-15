const axios = require('axios');
const { readConfig } = require('../System/config_manager');

const NOTION_API_BASE = 'https://api.notion.com/v1';
const NOTION_VERSION = '2022-06-28';

function hasNotionConfig(config) {
    return Boolean(config.notionApiKey);
}

function parseInstruction(instruction) {
    const raw = (instruction || '').trim();
    if (!raw) return { action: 'help' };

    try {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === 'object') {
            return {
                action: normalizeAction(parsed.action || 'create_page'),
                ...parsed
            };
        }
    } catch {
        // Plain text creates a note/page.
    }

    const lower = raw.toLowerCase();
    if (lower === 'help') return { action: 'help' };
    if (lower === 'list' || lower.startsWith('list database')) return { action: 'query_database' };
    if (lower.startsWith('read database')) return { action: 'query_database' };

    const [firstLine, ...rest] = raw.split('\n');
    return {
        action: 'create_page',
        title: firstLine.trim() || 'Mint Note',
        content: rest.join('\n').trim() || raw
    };
}

function normalizeAction(action) {
    const normalized = String(action || '').toLowerCase();
    if (['create', 'create_note', 'note', 'create_page', 'page'].includes(normalized)) return 'create_page';
    if (['list', 'read', 'query', 'query_database', 'read_database'].includes(normalized)) return 'query_database';
    if (['append', 'append_block', 'append_to_page'].includes(normalized)) return 'append_block';
    return normalized;
}

function notionHeaders(config) {
    return {
        Authorization: `Bearer ${config.notionApiKey}`,
        'Notion-Version': NOTION_VERSION,
        'Content-Type': 'application/json'
    };
}

function textBlock(text) {
    return {
        object: 'block',
        type: 'paragraph',
        paragraph: {
            rich_text: [
                {
                    type: 'text',
                    text: { content: String(text || '') }
                }
            ]
        }
    };
}

function headingBlock(text) {
    return {
        object: 'block',
        type: 'heading_2',
        heading_2: {
            rich_text: [
                {
                    type: 'text',
                    text: { content: String(text || '') }
                }
            ]
        }
    };
}

function buildChildren(input) {
    const content = input.content || input.body || input.text || '';
    const blocks = [];

    if (Array.isArray(input.children)) return input.children;
    if (input.heading) blocks.push(headingBlock(input.heading));

    const paragraphs = String(content || '')
        .split(/\n\s*\n/)
        .map(part => part.trim())
        .filter(Boolean);

    for (const paragraph of paragraphs.length ? paragraphs : ['Created by Mint.']) {
        blocks.push(textBlock(paragraph));
    }

    return blocks;
}

function buildDatabaseProperties(input, config) {
    const title = input.title || input.summary || input.name || 'Mint Note';
    const titleProperty = input.titleProperty || config.notionTitleProperty || 'Name';
    const properties = {
        [titleProperty]: {
            title: [
                {
                    text: { content: title }
                }
            ]
        }
    };

    if (input.properties && typeof input.properties === 'object') {
        return { ...properties, ...input.properties };
    }

    return properties;
}

function formatNotionTitle(properties = {}) {
    for (const property of Object.values(properties)) {
        if (property && property.type === 'title' && Array.isArray(property.title)) {
            const title = property.title.map(part => part.plain_text || part.text?.content || '').join('');
            if (title) return title;
        }
    }
    return '(Untitled)';
}

async function createPage(config, input) {
    const databaseId = input.databaseId || config.notionDatabaseId;
    const pageId = input.pageId || config.notionPageId;

    if (!databaseId && !pageId) {
        throw new Error('Missing Notion databaseId or pageId. Configure one in onboarding or pass it in the instruction JSON.');
    }

    const payload = databaseId
        ? {
            parent: { database_id: databaseId },
            properties: buildDatabaseProperties(input, config),
            children: buildChildren(input)
        }
        : {
            parent: { page_id: pageId },
            properties: {
                title: [
                    {
                        text: { content: input.title || input.summary || input.name || 'Mint Note' }
                    }
                ]
            },
            children: buildChildren(input)
        };

    const response = await axios.post(`${NOTION_API_BASE}/pages`, payload, {
        headers: notionHeaders(config)
    });

    const page = response.data || {};
    const title = input.title || input.summary || input.name || 'Mint Note';
    return `Created Notion page "${title}".${page.url ? `\n${page.url}` : ''}`;
}

async function queryDatabase(config, input) {
    const databaseId = input.databaseId || config.notionDatabaseId;
    if (!databaseId) {
        throw new Error('Missing Notion databaseId. Configure one in onboarding or pass it in the instruction JSON.');
    }

    const payload = {
        page_size: Number(input.pageSize || input.limit || 10)
    };

    if (input.filter) payload.filter = input.filter;
    if (input.sorts) payload.sorts = input.sorts;

    const response = await axios.post(`${NOTION_API_BASE}/databases/${databaseId}/query`, payload, {
        headers: notionHeaders(config)
    });

    const results = response.data.results || [];
    if (results.length === 0) return 'No Notion database pages found.';

    const lines = results.map((page, index) => {
        const title = formatNotionTitle(page.properties);
        return `${index + 1}. ${title}${page.url ? ` — ${page.url}` : ''}`;
    });

    return `Notion database pages:\n${lines.join('\n')}`;
}

async function appendBlock(config, input) {
    const pageId = input.pageId || config.notionPageId;
    if (!pageId) {
        throw new Error('Missing Notion pageId. Configure one in onboarding or pass it in the instruction JSON.');
    }

    const response = await axios.patch(`${NOTION_API_BASE}/blocks/${pageId}/children`, {
        children: buildChildren(input)
    }, {
        headers: notionHeaders(config)
    });

    const count = response.data.results ? response.data.results.length : buildChildren(input).length;
    return `Appended ${count} block(s) to Notion page.`;
}

function helpText() {
    return [
        'Notion plugin commands:',
        '- Create page: {"action":"create_page","title":"Note title","content":"Body text"}',
        '- Query database: {"action":"query_database","limit":5}',
        '- Append to page: {"action":"append_block","pageId":"...","content":"Text"}',
        'Plain text creates a Notion page using the configured default database or page.'
    ].join('\n');
}

module.exports = {
    name: 'notion',
    description: 'Manage Notion. Target can be JSON: {"action":"create_page","title":"Note","content":"Body","databaseId":"optional","pageId":"optional"}, {"action":"query_database","databaseId":"optional","limit":10}, or {"action":"append_block","pageId":"optional","content":"Text"}. Plain text creates a note.',

    async execute(instruction) {
        const config = readConfig();
        const input = parseInstruction(instruction);

        if (input.action === 'help') return helpText();
        if (!hasNotionConfig(config)) {
            return 'Notion API is not configured. Add a Notion internal integration secret with `mint onboard`, then share your Notion page/database with that integration.';
        }

        switch (input.action) {
            case 'create_page':
                return await createPage(config, input);
            case 'query_database':
                return await queryDatabase(config, input);
            case 'append_block':
                return await appendBlock(config, input);
            default:
                throw new Error(`Unsupported Notion action: ${input.action}`);
        }
    },

    _helpers: {
        parseInstruction,
        buildChildren,
        buildDatabaseProperties,
        formatNotionTitle,
        hasNotionConfig
    }
};
