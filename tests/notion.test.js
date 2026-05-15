/**
 * Tests: notion.js plugin
 */

jest.mock('axios', () => ({
    post: jest.fn(),
    patch: jest.fn()
}));

jest.mock('../src/System/config_manager', () => ({
    readConfig: jest.fn()
}));

const axios = require('axios');
const { readConfig } = require('../src/System/config_manager');
const notion = require('../src/Plugins/notion');

describe('notion plugin', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    test('has required plugin fields', () => {
        expect(notion.name).toBe('notion');
        expect(typeof notion.description).toBe('string');
        expect(typeof notion.execute).toBe('function');
    });

    test('returns configuration message when API key is missing', async () => {
        readConfig.mockReturnValue({});

        const result = await notion.execute('My note');

        expect(result).toContain('Notion API is not configured');
        expect(axios.post).not.toHaveBeenCalled();
    });

    test('creates a page in default database', async () => {
        readConfig.mockReturnValue({
            notionApiKey: 'secret',
            notionDatabaseId: 'db-id',
            notionTitleProperty: 'Name'
        });
        axios.post.mockResolvedValueOnce({
            data: {
                url: 'https://notion.so/page'
            }
        });

        const result = await notion.execute(JSON.stringify({
            action: 'create_page',
            title: 'Project note',
            content: 'Body text'
        }));

        expect(axios.post).toHaveBeenCalledTimes(1);
        expect(axios.post.mock.calls[0][0]).toBe('https://api.notion.com/v1/pages');
        expect(axios.post.mock.calls[0][1].parent).toEqual({ database_id: 'db-id' });
        expect(axios.post.mock.calls[0][1].properties.Name.title[0].text.content).toBe('Project note');
        expect(result).toContain('Project note');
        expect(result).toContain('https://notion.so/page');
    });

    test('queries database pages', async () => {
        readConfig.mockReturnValue({
            notionApiKey: 'secret',
            notionDatabaseId: 'db-id'
        });
        axios.post.mockResolvedValueOnce({
            data: {
                results: [
                    {
                        url: 'https://notion.so/one',
                        properties: {
                            Name: {
                                type: 'title',
                                title: [{ plain_text: 'First page' }]
                            }
                        }
                    }
                ]
            }
        });

        const result = await notion.execute(JSON.stringify({ action: 'query_database', limit: 1 }));

        expect(axios.post).toHaveBeenCalledWith(
            'https://api.notion.com/v1/databases/db-id/query',
            { page_size: 1 },
            expect.any(Object)
        );
        expect(result).toContain('First page');
    });

    test('appends blocks to default page', async () => {
        readConfig.mockReturnValue({
            notionApiKey: 'secret',
            notionPageId: 'page-id'
        });
        axios.patch.mockResolvedValueOnce({
            data: { results: [{ id: 'block-1' }] }
        });

        const result = await notion.execute(JSON.stringify({
            action: 'append_block',
            content: 'Follow-up note'
        }));

        expect(axios.patch).toHaveBeenCalledTimes(1);
        expect(axios.patch.mock.calls[0][0]).toBe('https://api.notion.com/v1/blocks/page-id/children');
        expect(result).toContain('Appended 1 block');
    });

    test('plain text becomes create_page instruction', () => {
        const parsed = notion._helpers.parseInstruction('Title line\n\nBody line');

        expect(parsed.action).toBe('create_page');
        expect(parsed.title).toBe('Title line');
        expect(parsed.content).toBe('Body line');
    });
});
