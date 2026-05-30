import { colors } from './cli_colors';

export interface ProgressInfo {
    action?: string;
    phase?: string;
    target?: string;
    message?: string;
    thought?: string;
}

/**
 * Formats a code-agent progress event into a console-friendly string.
 * @param info
 * @returns
 */
export function formatProgress(info: string | ProgressInfo): string {
    if (typeof info === 'string') return `${colors.gray}[Mint Code] ${info}${colors.reset}`;

    const { action, target, message, thought } = info;

    if (thought && process.env.MINT_HIDE_AGENT_NOTES !== '1') {
        return `\n${colors.gray}${colors.bright}•${colors.reset} ${colors.gray}${thought}${colors.reset}`;
    }

    if (action === 'ask_user') {
        return `\n${colors.mint}✓${colors.reset} ${colors.bright}Ask User${colors.reset}\n${colors.gray}   ${target || message || ''}${colors.reset}`;
    }

    let icon  = `${colors.mint}✓${colors.reset}`;
    let label = action || info.phase || '';
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

export interface MemoryInteraction {
    id?: number | string;
    created_at?: string;
    user_text?: string;
    ai_text?: string;
}

/**
 * Formats a list of memory interactions for display.
 * @param interactions
 * @param title
 * @returns
 */
export function formatMemoryInteractions(interactions: MemoryInteraction[], title = 'Remembered interactions'): string {
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
 * @param text
 * @returns
 */
export function splitResponseSentences(text: string): string[] {
    const normalized = String(text || '').replace(/\r\n/g, '\n');
    if (!normalized) return [];

    const chunks: string[] = [];
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
