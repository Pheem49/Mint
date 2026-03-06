const { shell } = require('electron');

function openWebsite(targetUrl) {
    if (!targetUrl) return;
    let url = targetUrl;
    if (!url.startsWith('http')) {
        url = 'https://' + url;
    }
    shell.openExternal(url);
}

function openSearch(query) {
    if (!query) return;
    shell.openExternal(`https://www.google.com/search?q=${encodeURIComponent(query)}`);
}

module.exports = { openWebsite, openSearch };
