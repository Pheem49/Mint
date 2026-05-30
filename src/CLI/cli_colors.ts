/**
 * ANSI color constants for Mint CLI output.
 */
export const colors = {
    reset:  '\x1b[0m',
    bright: '\x1b[1m',
    mint:   '\x1b[38;5;121m',
    pink:   '\x1b[38;5;213m',
    gray:   '\x1b[90m',
    cyan:   '\x1b[36m',
    yellow: '\x1b[33m'
};

let isExiting = false;

export interface ExitSummary {
    message?: string;
    wallMs?: number;
    agentActiveMs?: number;
    toolCalls?: {
        total?: number;
        success?: number;
        failed?: number;
        successRate?: number;
    };
    modelUsage?: Array<{
        provider?: string;
        model?: string;
        inputTokens?: number;
        cacheReads?: number;
        outputTokens?: number;
        reasoningTokens?: number;
        totalTokens?: number;
    }>;
}

function formatCount(value: any): string {
    const number = Number(value) || 0;
    return number.toLocaleString('en-US');
}

function formatDurationMs(value: any): string {
    const ms = Math.max(0, Number(value) || 0);
    const seconds = ms / 1000;
    if (seconds < 10) return `${seconds.toFixed(1)}s`;
    if (seconds < 60) return `${Math.round(seconds)}s`;
    const minutes = Math.floor(seconds / 60);
    const remaining = Math.round(seconds % 60);
    return `${minutes}m ${remaining}s`;
}

function fitText(value: any, width: number): string {
    const text = String(value || '');
    if (text.length <= width) return text;
    if (width <= 3) return text.slice(0, width);
    return `${text.slice(0, width - 3)}...`;
}

function line(label: string, value: any, width: number): string {
    const text = fitText(`  ${label.padEnd(28)}${String(value || '')}`, width - 2);
    return `│${text.padEnd(width - 2)}│`;
}

function divider(width: number): string {
    return `│  ${'─'.repeat(Math.max(0, width - 4))}│`;
}

export function formatExitSummary(summary: Partial<ExitSummary> = {}): string {
    const width = Math.max(76, Math.min(135, process.stdout.columns || 100));
    const top = `╭${'─'.repeat(width - 2)}╮`;
    const bottom = `╰${'─'.repeat(width - 2)}╯`;
    const empty = `│${''.padEnd(width - 2)}│`;
    const message = summary.message || 'Goodbye! See you again.';
    const toolCalls = summary.toolCalls || {};
    const modelUsage = Array.isArray(summary.modelUsage) ? summary.modelUsage : [];
    const primaryModel = modelUsage[0]
        ? `${modelUsage[0].provider || 'provider'}:${modelUsage[0].model || 'model'}`
        : '';
    const totals = modelUsage.reduce((acc, item) => {
        acc.input += Number(item.inputTokens) || 0;
        acc.cache += Number(item.cacheReads) || 0;
        acc.output += Number(item.outputTokens) || 0;
        acc.reasoning += Number(item.reasoningTokens) || 0;
        acc.total += Number(item.totalTokens) || 0;
        return acc;
    }, { input: 0, cache: 0, output: 0, reasoning: 0, total: 0 });
    if (!totals.total) totals.total = totals.input + totals.output;

    const lines = [
        top,
        line(message, '', width),
        empty,
        line('Performance', '', width),
        line('Wall Time:', formatDurationMs(summary.wallMs), width),
        line('Agent Active:', formatDurationMs(summary.agentActiveMs), width)
    ];

    if (Number(toolCalls.total) > 0) {
        lines.push(
            line('Tool Calls:', `${formatCount(toolCalls.total)} ( ✓ ${formatCount(toolCalls.success)} x ${formatCount(toolCalls.failed)} )`, width),
            line('Success Rate:', `${Number(toolCalls.successRate || 0).toFixed(1)}%`, width)
        );
    }

    lines.push(
        empty,
        line('Model:', primaryModel || 'No model calls recorded.', width),
        divider(width),
        line('Token usage:', `total=${formatCount(totals.total)} input=${formatCount(totals.input)} (+ ${formatCount(totals.cache)} cached) output=${formatCount(totals.output)}${totals.reasoning ? ` (reasoning ${formatCount(totals.reasoning)})` : ''}`, width)
    );

    lines.push(bottom);
    return lines.join('\n');
}

/**
 * Restore terminal state, print goodbye, and exit.
 * @param code
 * @param summary
 */
export function exitWithGoodbye(code = 0, summary: Partial<ExitSummary> | null = null): void {
    if (isExiting) return;
    isExiting = true;

    process.stdout.write('\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l');
    process.stdout.write('\x1b[?25h');
    if (summary) {
        console.log(`\n${colors.pink}${formatExitSummary(summary)}${colors.reset}\n`);
    } else {
        console.log(`\n${colors.pink}Goodbye! See you again soon!${colors.reset}\n`);
    }
    process.exit(code);
}

export const _helpers = { formatExitSummary, formatDurationMs };
