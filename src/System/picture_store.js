const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');
const { pathToFileURL } = require('url');

const PICTURES_DIR = path.join(os.homedir(), '.config', 'mint', 'Pictures');
const INDEX_PATH = path.join(PICTURES_DIR, 'pictures.json');

const EXTENSIONS = {
    'image/png': 'png',
    'image/jpeg': 'jpg',
    'image/jpg': 'jpg',
    'image/webp': 'webp',
    'image/gif': 'gif'
};

function ensurePicturesDir() {
    fs.mkdirSync(PICTURES_DIR, { recursive: true });
}

function readIndex() {
    try {
        if (!fs.existsSync(INDEX_PATH)) return [];
        const parsed = JSON.parse(fs.readFileSync(INDEX_PATH, 'utf8'));
        return Array.isArray(parsed) ? parsed : [];
    } catch (error) {
        console.error('[Pictures] Failed to read index:', error.message);
        return [];
    }
}

function writeIndex(entries) {
    ensurePicturesDir();
    fs.writeFileSync(INDEX_PATH, JSON.stringify(entries, null, 2), 'utf8');
}

function parseImageDataUri(dataUri) {
    if (!dataUri || typeof dataUri !== 'string') return null;
    const match = dataUri.match(/^data:(image\/[\w.+-]+);base64,([\s\S]+)$/);
    if (!match) return null;

    const mimeType = match[1].toLowerCase();
    const extension = EXTENSIONS[mimeType] || 'png';
    return {
        mimeType,
        extension,
        buffer: Buffer.from(match[2], 'base64')
    };
}

function createFilename(extension) {
    const stamp = new Date().toISOString()
        .replace(/[-:]/g, '')
        .replace(/\..+$/, '')
        .replace('T', '-');
    const id = crypto.randomBytes(4).toString('hex');
    return `mint-${stamp}-${id}.${extension}`;
}

function saveChatImages(base64Image, metadata = {}) {
    const images = Array.isArray(base64Image) ? base64Image : (base64Image ? [base64Image] : []);
    const saved = [];
    if (images.length === 0) return saved;

    ensurePicturesDir();
    const index = readIndex();

    for (const item of images) {
        const parsed = parseImageDataUri(item);
        if (!parsed || parsed.buffer.length === 0) continue;

        const filename = createFilename(parsed.extension);
        const filePath = path.join(PICTURES_DIR, filename);
        fs.writeFileSync(filePath, parsed.buffer);

        const entry = {
            id: path.basename(filename, path.extname(filename)),
            filename,
            path: filePath,
            mimeType: parsed.mimeType,
            createdAt: new Date().toISOString(),
            source: metadata.source || 'chat',
            message: String(metadata.message || '').slice(0, 240)
        };

        index.unshift(entry);
        saved.push(entry);
    }

    writeIndex(index);
    return saved;
}

function listSavedPictures() {
    ensurePicturesDir();
    return readIndex()
        .filter(entry => entry && entry.path && fs.existsSync(entry.path))
        .map(entry => ({
            ...entry,
            url: pathToFileURL(entry.path).href
        }));
}

module.exports = {
    PICTURES_DIR,
    saveChatImages,
    listSavedPictures
};
