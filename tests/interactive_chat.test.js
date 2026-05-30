jest.mock('@google/genai', () => ({
    GoogleGenAI: jest.fn()
}));

jest.mock('axios', () => ({}));

jest.mock('../dist/src/AI_Brain/Gemini_API', () => ({
    handleChat: jest.fn(),
    getChatTranscript: jest.fn(() => []),
    resetChat: jest.fn()
}));

describe('interactive chat image display helpers', () => {
    test('does not append duplicate labels when prompt already references each image', () => {
        const { _helpers } = require('../dist/src/CLI/interactive_chat');
        const message = '2รูปนี้ [Image #1] กับ [Image #2] มิ้นคิดว่ารูปไหนสวยกว่า';

        expect(_helpers.hasAllImageLabels(message, 2)).toBe(true);
        expect(_helpers.formatImageDisplayMessage(message, '[Image #1] [Image #2]', 2)).toBe(message);
    });

    test('appends labels when prompt does not reference every image', () => {
        const { _helpers } = require('../dist/src/CLI/interactive_chat');

        expect(_helpers.formatImageDisplayMessage(
            'คิดยังไงกับรูปนี้ [Image #1]',
            '[Image #1] [Image #2]',
            2
        )).toBe('คิดยังไงกับรูปนี้ [Image #1]\n[Image #1] [Image #2]');
    });
});
