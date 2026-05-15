const MAX_GOOGLE_TTS_CHARS = 200;

function splitTextForTts(text, maxLength = MAX_GOOGLE_TTS_CHARS) {
    const normalized = String(text || '').replace(/\s+/g, ' ').trim();
    if (!normalized) return [];

    const chunks = [];
    let remaining = normalized;

    while (remaining.length > maxLength) {
        const slice = remaining.slice(0, maxLength + 1);
        const splitAt = Math.max(
            slice.lastIndexOf('.'),
            slice.lastIndexOf('?'),
            slice.lastIndexOf('!'),
            slice.lastIndexOf(','),
            slice.lastIndexOf(' ')
        );
        const safeSplit = splitAt > 0 ? splitAt : maxLength;
        chunks.push(remaining.slice(0, safeSplit).trim());
        remaining = remaining.slice(safeSplit).trim();
    }

    if (remaining) chunks.push(remaining);
    return chunks;
}

function getGoogleTtsUrls(text, options = {}) {
    const lang = options.lang || 'en';
    const host = options.host || 'https://translate.google.com';
    const chunks = splitTextForTts(text);

    return chunks.map((chunk, index) => {
        const params = new URLSearchParams({
            ie: 'UTF-8',
            q: chunk,
            tl: lang,
            client: 'tw-ob',
            idx: String(index),
            total: String(chunks.length),
            textlen: String(chunk.length)
        });

        return {
            shortText: chunk,
            url: `${host}/translate_tts?${params.toString()}`
        };
    });
}

module.exports = { getGoogleTtsUrls, splitTextForTts };
