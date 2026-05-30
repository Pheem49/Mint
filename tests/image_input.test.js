const fs = require('fs');
const os = require('os');
const path = require('path');

const { loadImageAsDataUri, _helpers } = require('../dist/src/CLI/image_input');

describe('CLI image input helpers', () => {
    test('detects supported image mime types', () => {
        expect(_helpers.getImageMimeType('screen.png')).toBe('image/png');
        expect(_helpers.getImageMimeType('photo.JPG')).toBe('image/jpeg');
        expect(_helpers.getImageMimeType('mockup.webp')).toBe('image/webp');
    });

    test('rejects unsupported file extensions', () => {
        expect(() => _helpers.getImageMimeType('notes.txt')).toThrow('Unsupported image type');
    });

    test('loads an image file as a data URI', () => {
        const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-image-input-'));
        const file = path.join(dir, 'pixel.png');
        fs.writeFileSync(file, Buffer.from([0x89, 0x50, 0x4e, 0x47]));

        const result = loadImageAsDataUri(file);

        expect(result.path).toBe(file);
        expect(result.mimeType).toBe('image/png');
        expect(result.dataUri).toBe(`data:image/png;base64,${Buffer.from([0x89, 0x50, 0x4e, 0x47]).toString('base64')}`);
    });

    test('returns null when clipboard command is unavailable or has no image', () => {
        expect(_helpers.tryReadClipboardCommand('mint-command-that-does-not-exist', [])).toBeNull();
    });
});
