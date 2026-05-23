'use strict';

/**
 * ANSI color constants for Mint CLI output.
 */
const colors = {
    reset:  '\x1b[0m',
    bright: '\x1b[1m',
    mint:   '\x1b[38;5;121m',
    pink:   '\x1b[38;5;213m',
    gray:   '\x1b[90m',
    cyan:   '\x1b[36m',
    yellow: '\x1b[33m'
};

let isExiting = false;

/**
 * Restore terminal state, print goodbye, and exit.
 * @param {number} [code=0]
 */
function exitWithGoodbye(code = 0) {
    if (isExiting) return;
    isExiting = true;

    process.stdout.write('\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l');
    process.stdout.write('\x1b[?25h');
    console.log(`\n${colors.pink}Goodbye! See you again soon!${colors.reset}\n`);
    process.exit(code);
}

module.exports = { colors, exitWithGoodbye };
