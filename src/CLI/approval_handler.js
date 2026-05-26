'use strict';

const readline = require('readline');
const { colors } = require('./cli_colors');

/**
 * Prompts the user in the terminal to approve or deny a code-agent action.
 * Used by the non-interactive `mint code <task>` command.
 *
 * @param {{ type: string, label?: string, preview?: string }} request
 * @returns {Promise<boolean>}  true = approved, false = denied
 */
async function requestCodeApproval(request) {
    const typeLabel =
        request.type === 'shell' ? 'Shell Command' :
        request.type === 'patch' ? 'Patch Edit'    :
        'File Write';

    console.log(`\n${colors.yellow}${colors.bright}[Approval Required]${colors.reset} ${typeLabel}`);
    if (request.label)   console.log(`${colors.gray}${request.label}${colors.reset}`);
    if (Array.isArray(request.warnings) && request.warnings.length > 0) {
        request.warnings.forEach((warning) => {
            console.log(`${colors.yellow}Warning:${colors.reset} ${warning}`);
        });
    }
    if (request.preview) console.log(`${colors.gray}${request.preview}${colors.reset}\n`);

    const rl = readline.createInterface({
        input:  process.stdin,
        output: process.stdout
    });

    const answer = await new Promise((resolve) => {
        rl.question('Approve this action? [y/N]: ', (value) => {
            rl.close();
            resolve((value || '').trim().toLowerCase());
        });
    });

    const approved = answer === 'y' || answer === 'yes';
    console.log(approved
        ? `${colors.mint}[Mint Code] Approved.${colors.reset}\n`
        : `${colors.pink}[Mint Code] Denied.${colors.reset}\n`);
    return approved;
}

module.exports = { requestCodeApproval };
