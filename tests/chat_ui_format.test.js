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
});
