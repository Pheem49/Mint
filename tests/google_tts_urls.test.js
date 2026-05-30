const { getGoogleTtsUrls, splitTextForTts } = require('../dist/src/System/google_tts_urls');

describe('google_tts_urls', () => {
    test('returns no URLs for empty text', () => {
        expect(getGoogleTtsUrls('')).toEqual([]);
    });

    test('builds Google Translate TTS URLs with encoded text', () => {
        const urls = getGoogleTtsUrls('hello world', { lang: 'en' });

        expect(urls).toHaveLength(1);
        expect(urls[0].shortText).toBe('hello world');
        expect(urls[0].url).toContain('translate_tts?');
        expect(urls[0].url).toContain('q=hello+world');
        expect(urls[0].url).toContain('tl=en');
    });

    test('splits long text into bounded chunks', () => {
        const chunks = splitTextForTts('a '.repeat(150), 50);

        expect(chunks.length).toBeGreaterThan(1);
        expect(chunks.every(chunk => chunk.length <= 50)).toBe(true);
    });
});
