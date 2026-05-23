'use strict';

const { colors } = require('./cli_colors');

/**
 * Formats a code-agent progress event into a console-friendly string.
 * @param {string|object} info
 * @returns {string}
 */
function formatProgress(info) {
    if (typeof info === 'string') return `${colors.gray}[Mint Code] ${info}${colors.reset}`;

    const { action, target, message } = info;

    if (action === 'ask_user') {
        return `\n${colors.mint}✓${colors.reset} ${colors.bright}Ask User${colors.reset}\n${colors.gray}   ${target || message || ''}${colors.reset}`;
    }

    let icon  = `${colors.mint}✓${colors.reset}`;
    let label = action || info.phase;
    let color = colors.reset;

    switch (action) {
        case 'thinking':
            return `\n${colors.yellow}* ${colors.bright}Thinking${colors.reset}`;
        case 'web_search':  label = 'WebSearch';    break;
        case 'list_files':
        case 'find_path':   label = 'Explored';     break;
        case 'read_file':   label = 'ReadFile';     break;
        case 'search_code': label = 'SearchText';   break;
        case 'plan':        label = 'Plan';         break;
        case 'apply_patch':
        case 'write_file':  label = 'Edited';       break;
        case 'run_shell':   label = 'Ran command';  break;
        case 'json_repair':
            icon  = '*';
            label = 'Repairing JSON';
            break;
        case 'reviewer_start': label = 'Reviewing'; break;
        default: break;
    }

    const content = target || message || '';
    return ` ${icon} ${colors.bright}${label}${colors.reset} ${color}${content}${colors.reset}`;
}

/**
 * Formats a list of memory interactions for display.
 * @param {Array} interactions
 * @param {string} [title]
 * @returns {string}
 */
function formatMemoryInteractions(interactions, title = 'Remembered interactions') {
    if (!Array.isArray(interactions) || interactions.length === 0) {
        return `${title}:\n(no memories found)`;
    }

    const lines = [`${title}:`];
    interactions.forEach((item, index) => {
        const when = item.created_at ? ` (${item.created_at})` : '';
        const id   = item.id ? `#${item.id} ` : '';
        lines.push(`${index + 1}. ${id}User${when}: ${item.user_text}`);
        lines.push(`   Mint: ${item.ai_text}`);
    });
    return lines.join('\n');
}

/**
 * Splits a response text into sentence-level chunks for streaming.
 * @param {string} text
 * @returns {string[]}
 */
function splitResponseSentences(text) {
    const normalized = String(text || '').replace(/\r\n/g, '\n');
    if (!normalized) return [];

    const chunks = [];
    let buffer = '';
    for (const char of normalized) {
        buffer += char;
        if (char === '\n' || /[.!?。！？…]/u.test(char)) {
            if (buffer.trim()) chunks.push(buffer);
            buffer = '';
        }
    }
    if (buffer.trim()) chunks.push(buffer);
    return chunks.length > 0 ? chunks : [normalized];
}

module.exports = { formatProgress, formatMemoryInteractions, splitResponseSentences };
