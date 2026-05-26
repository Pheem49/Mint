const { _helpers } = require('../src/CLI/chat_ui');

describe('CLI chat UI formatting', () => {
    test('cleans common markdown from assistant responses', () => {
        const text = [
            '### วิธีเลือก Kernel',
            '**ขั้นตอนแรก:** เปิดเครื่อง',
            '- เลือก Pop!_OS',
            '1. กด Enter',
            '',
            '',
            '`uname -r` ใช้เช็ค kernel'
        ].join('\n');

        expect(_helpers.cleanDisplayText(text, 'assistant')).toBe([
            'วิธีเลือก Kernel',
            'ขั้นตอนแรก: เปิดเครื่อง',
            '• เลือก Pop!_OS',
            '1. กด Enter',
            '',
            'uname -r ใช้เช็ค kernel'
        ].join('\n'));
    });

    test('keeps user text mostly unchanged', () => {
        expect(_helpers.cleanDisplayText('**hello**', 'user')).toBe('**hello**');
    });

    test('formats code agent progress as compact activities', () => {
        expect(_helpers.formatActivityStep({
            action: 'read_file',
            target: 'src/CLI/chat_ui.js'
        })).toEqual({
            title: 'Explored',
            detail: 'Read chat_ui.js'
        });

        expect(_helpers.formatActivityStep({
            action: 'run_shell',
            target: 'npm test -- --runInBand'
        })).toEqual({
            title: 'Ran',
            detail: 'npm test -- --runInBand'
        });
    });

    test('skips empty assistant and system messages', () => {
        expect(_helpers.shouldAppendMessage('assistant', '')).toBe(false);
        expect(_helpers.shouldAppendMessage('assistant', '   \n')).toBe(false);
        expect(_helpers.shouldAppendMessage('system', '')).toBe(false);
        expect(_helpers.shouldAppendMessage('user', '')).toBe(true);
    });

    test('formats working duration', () => {
        expect(_helpers.formatDuration(5)).toBe('5s');
        expect(_helpers.formatDuration(118)).toBe('1m 58s');
    });

    test('splits edit diff stats for colored rendering', () => {
        expect(_helpers.splitDiffStatSegments('Edited src/CLI/code_agent.js (+52 -0)')).toEqual([
            { text: 'Edited src/CLI/code_agent.js ', color: 'cyanBright' },
            { text: '(', color: 'gray' },
            { text: '+52', color: 'greenBright' },
            { text: ' ', color: 'gray' },
            { text: '-0', color: 'redBright' },
            { text: ')', color: 'gray' }
        ]);
    });

    test('cycles approval choices including approve for session', () => {
        expect(_helpers.getNextApprovalChoice('approve')).toBe('approve_session');
        expect(_helpers.getNextApprovalChoice('approve_session')).toBe('deny');
        expect(_helpers.getNextApprovalChoice('deny')).toBe('approve');
        expect(_helpers.getNextApprovalChoice('approve', -1)).toBe('deny');
    });

    test('parses unified diff previews for approval rendering', () => {
        const files = _helpers.parseUnifiedDiffPreview([
            '--- a/src/demo.js',
            '+++ b/src/demo.js',
            '@@ -1,2 +1,2 @@',
            '-const oldValue = true;',
            '+const newValue = true;',
            ' context'
        ].join('\n'));

        expect(files).toEqual([{
            path: 'src/demo.js',
            additions: 1,
            deletions: 1,
            lines: [
                { type: 'hunk', text: '@@ -1,2 +1,2 @@' },
                { type: 'delete', text: '-const oldValue = true;' },
                { type: 'add', text: '+const newValue = true;' },
                { type: 'context', text: ' context' }
            ]
        }]);
        expect(_helpers.isUnifiedDiffPreview('plain text')).toBe(false);
    });

    test('styles diff lines with foreground colors only', () => {
        expect(_helpers.getDiffLineStyle({ type: 'add' })).toEqual({ color: 'greenBright' });
        expect(_helpers.getDiffLineStyle({ type: 'delete' })).toEqual({ color: 'redBright' });
        expect(_helpers.getDiffLineStyle({ type: 'hunk' })).toEqual({ color: 'cyanBright' });
        expect(_helpers.getDiffLineStyle({ type: 'context' })).toEqual({ color: 'gray', dimColor: true });
        expect(_helpers.getDiffLineStyle({ type: 'add' })).not.toHaveProperty('backgroundColor');
        expect(_helpers.getDiffLineStyle({ type: 'delete' })).not.toHaveProperty('backgroundColor');
    });

    test('inserts image placeholders inline with the typed prompt', () => {
        expect(_helpers.appendInlineImageToken('ฉันอยากทำ', 1)).toBe('ฉันอยากทำ [Image #1]');
        expect(_helpers.appendInlineImageToken('ฉันอยากทำ ', 1)).toBe('ฉันอยากทำ [Image #1]');
        expect(_helpers.appendInlineImageToken('', 1)).toBe('[Image #1]');
        expect(_helpers.appendInlineImageToken('เทียบ [Image #1] กับ', 2)).toBe('เทียบ [Image #1] กับ [Image #2]');
    });

    test('removes inline image placeholders when attachments are removed', () => {
        expect(_helpers.removeImageToken('ฉันอยากทำ [Image #1] ต่อ', 1)).toBe('ฉันอยากทำ ต่อ');
        expect(_helpers.removeAllImageTokens('ดู [Image #1] กับ [Image #2] หน่อย')).toBe('ดู กับ หน่อย');
    });
});
