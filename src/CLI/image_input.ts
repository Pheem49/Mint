const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const IMAGE_MIME_TYPES = {
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
    '.webp': 'image/webp',
    '.gif': 'image/gif'
};

function resolveImagePath(imagePath, cwd = process.cwd()) {
    if (!imagePath || typeof imagePath !== 'string') {
        throw new Error('Image path is required.');
    }

    return path.resolve(cwd, imagePath);
}

function getImageMimeType(imagePath) {
    const ext = path.extname(imagePath).toLowerCase();
    const mimeType = IMAGE_MIME_TYPES[ext];
    if (!mimeType) {
        throw new Error(`Unsupported image type "${ext || '(none)'}". Supported: ${Object.keys(IMAGE_MIME_TYPES).join(', ')}`);
    }
    return mimeType;
}

function loadImageAsDataUri(imagePath, cwd = process.cwd()) {
    const resolved = resolveImagePath(imagePath, cwd);
    if (!fs.existsSync(resolved)) {
        throw new Error(`Image file not found: ${imagePath}`);
    }

    const stat = fs.statSync(resolved);
    if (!stat.isFile()) {
        throw new Error(`Image path is not a file: ${imagePath}`);
    }

    const mimeType = getImageMimeType(resolved);
    const data = fs.readFileSync(resolved).toString('base64');
    return {
        path: resolved,
        mimeType,
        dataUri: `data:${mimeType};base64,${data}`
    };
}

function tryReadClipboardCommand(command, args) {
    try {
        return execFileSync(command, args, {
            encoding: 'buffer',
            maxBuffer: 1024 * 1024 * 20,
            stdio: ['ignore', 'pipe', 'ignore']
        });
    } catch (_) {
        return null;
    }
}

function loadClipboardImageAsDataUri() {
    const attempts = [
        { command: 'wl-paste', args: ['--type', 'image/png', '--no-newline'] },
        { command: 'xclip', args: ['-selection', 'clipboard', '-t', 'image/png', '-o'] }
    ];

    for (const attempt of attempts) {
        const data = tryReadClipboardCommand(attempt.command, attempt.args);
        if (data && data.length > 0) {
            return {
                path: 'clipboard',
                mimeType: 'image/png',
                dataUri: `data:image/png;base64,${data.toString('base64')}`
            };
        }
    }

    throw new Error('No clipboard image found. On Linux, install wl-clipboard or xclip, then copy an image and try Ctrl+V again.');
}

module.exports = {
    loadImageAsDataUri,
    loadClipboardImageAsDataUri,
    _helpers: {
        getImageMimeType,
        resolveImagePath,
        tryReadClipboardCommand
    }
};
