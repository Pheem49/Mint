const { exec } = require('child_process');

let shell;
try {
    shell = require('electron').shell;
} catch (e) {
    shell = null;
}

function openWebsite(targetUrl) {
    if (!targetUrl) return;
    let url = targetUrl;
    if (!url.startsWith('http')) {
        url = 'https://' + url;
    }
    if (shell) {
        shell.openExternal(url);
    } else {
        // Fallback for Node.js (Linux focus)
        exec(`xdg-open "${url}"`, (err) => {
            if (err) console.error("Failed to open URL via xdg_open:", err);
        });
    }
}

function openSearch(query) {
    if (!query) return;
    const url = `https://www.google.com/search?q=${encodeURIComponent(query)}`;
    if (shell) {
        shell.openExternal(url);
    } else {
        exec(`xdg-open "${url}"`, (err) => {
            if (err) console.error("Failed to open search via xdg-open:", err);
        });
    }
}

module.exports = { openWebsite, openSearch };
