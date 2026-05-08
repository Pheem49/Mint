/**
 * Mint Notification System
 * ------------------------
 * Sends system-level notifications to the user.
 * Supports Linux (notify-send) as a primary target for CLI.
 */

const { exec } = require('child_process');

function sendNotification(title, message, urgency = 'normal') {
    // Attempt to use notify-send (Linux)
    const cmd = `notify-send -u ${urgency} "${title}" "${message}"`;
    exec(cmd, (err) => {
        if (err) {
            // Fallback: Silent console log if no notifier found
            console.log(`[Notification] ${title}: ${message}`);
        }
    });
}

module.exports = {
    sendNotification
};
