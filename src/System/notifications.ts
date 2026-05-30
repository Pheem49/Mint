import { exec } from 'child_process'

export function sendNotification(title: string, message: string, urgency = 'normal') {
    // Attempt to use notify-send (Linux)
    const cmd = `notify-send -u ${urgency} "${title}" "${message}"`
    exec(cmd, (err) => {
        if (err) {
            // Fallback: Silent console log if no notifier found
            console.log(`[Notification] ${title}: ${message}`)
        }
    })
}
