const { execFile } = require('child_process');
const os = require('os');

const MAX_TEXT_LENGTH = 2000;
const BROWSER_NAMES = [
    'chrome',
    'chromium',
    'brave',
    'firefox',
    'edge',
    'safari',
    'opera',
    'vivaldi'
];

function run(command, args = [], options = {}) {
    return new Promise((resolve) => {
        execFile(command, args, { timeout: options.timeout || 1200 }, (error, stdout) => {
            if (error) {
                resolve(null);
                return;
            }
            resolve(String(stdout || '').trim() || null);
        });
    });
}

function truncateText(value, maxLength = MAX_TEXT_LENGTH) {
    const text = String(value || '').replace(/\0/g, '').trim();
    if (text.length <= maxLength) return text;
    return `${text.slice(0, maxLength)}\n[truncated ${text.length - maxLength} chars]`;
}

function normalizeProcessName(value) {
    return String(value || '').trim().replace(/\.exe$/i, '');
}

function isBrowserProcess(name = '') {
    const normalized = normalizeProcessName(name).toLowerCase();
    return BROWSER_NAMES.some(browser => normalized.includes(browser));
}

async function getLinuxActiveWindow() {
    const windowId = await run('xdotool', ['getactivewindow']);
    if (!windowId) return null;

    const [title, pid] = await Promise.all([
        run('xdotool', ['getwindowname', windowId]),
        run('xdotool', ['getwindowpid', windowId])
    ]);

    let processName = '';
    if (pid) {
        processName = await run('ps', ['-p', pid, '-o', 'comm=']) || '';
    }

    return {
        id: windowId,
        title: title || '',
        appName: normalizeProcessName(processName),
        processName: normalizeProcessName(processName),
        pid: pid ? Number(pid) : null,
        platform: 'linux'
    };
}

async function getMacActiveWindow() {
    const script = [
        'tell application "System Events"',
        'set frontApp to first application process whose frontmost is true',
        'set appName to name of frontApp',
        'set windowTitle to ""',
        'try',
        'set windowTitle to name of front window of frontApp',
        'end try',
        'return appName & linefeed & windowTitle',
        'end tell'
    ].join('\n');
    const output = await run('osascript', ['-e', script]);
    if (!output) return null;
    const [appName = '', title = ''] = output.split(/\r?\n/);
    return {
        title,
        appName,
        processName: appName,
        pid: null,
        platform: 'darwin'
    };
}

async function getWindowsActiveWindow() {
    const script = [
        'Add-Type @\'',
        'using System;',
        'using System.Runtime.InteropServices;',
        'using System.Text;',
        'public class Win {',
        '[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();',
        '[DllImport("user32.dll")] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);',
        '[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);',
        '}',
        '\'@',
        '$hwnd = [Win]::GetForegroundWindow()',
        '$builder = New-Object System.Text.StringBuilder 1024',
        '[void][Win]::GetWindowText($hwnd, $builder, $builder.Capacity)',
        '$pid = 0',
        '[void][Win]::GetWindowThreadProcessId($hwnd, [ref]$pid)',
        '$proc = Get-Process -Id $pid -ErrorAction SilentlyContinue',
        '[PSCustomObject]@{ title = $builder.ToString(); appName = $proc.ProcessName; pid = $pid } | ConvertTo-Json -Compress'
    ].join('\n');
    const output = await run('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], { timeout: 1800 });
    if (!output) return null;
    try {
        const parsed = JSON.parse(output);
        return {
            title: parsed.title || '',
            appName: normalizeProcessName(parsed.appName),
            processName: normalizeProcessName(parsed.appName),
            pid: parsed.pid ? Number(parsed.pid) : null,
            platform: 'win32'
        };
    } catch (_) {
        return null;
    }
}

async function getActiveWindowContext(platform = process.platform) {
    try {
        if (platform === 'darwin') return await getMacActiveWindow();
        if (platform === 'win32') return await getWindowsActiveWindow();
        return await getLinuxActiveWindow();
    } catch (_) {
        return null;
    }
}

async function getLinuxSelectedText() {
    const attempts = [
        ['wl-paste', ['--primary', '--no-newline']],
        ['xclip', ['-selection', 'primary', '-out']],
        ['xsel', ['--primary', '--output']]
    ];

    for (const [command, args] of attempts) {
        const text = await run(command, args);
        if (text) return truncateText(text);
    }
    return '';
}

async function getSelectedText(platform = process.platform) {
    if (platform === 'linux') return getLinuxSelectedText();
    return '';
}

async function getMacBrowserContext(appName) {
    const normalized = String(appName || '').toLowerCase();
    let script = '';
    if (normalized.includes('safari')) {
        script = 'tell application "Safari" to return name of front document & linefeed & URL of front document';
    } else if (normalized.includes('chrome') || normalized.includes('chromium') || normalized.includes('brave') || normalized.includes('edge')) {
        script = `tell application "${appName}" to return title of active tab of front window & linefeed & URL of active tab of front window`;
    }
    if (!script) return null;
    const output = await run('osascript', ['-e', script], { timeout: 1500 });
    if (!output) return null;
    const [title = '', url = ''] = output.split(/\r?\n/);
    return { title, url };
}

async function getBrowserContext(activeWindow, platform = process.platform) {
    if (!activeWindow || !isBrowserProcess(activeWindow.appName || activeWindow.processName)) {
        return null;
    }
    if (platform === 'darwin') {
        const browser = await getMacBrowserContext(activeWindow.appName);
        if (browser) return browser;
    }
    return {
        title: activeWindow.title || '',
        url: '',
        urlUnavailableReason: 'Browser URL is not available from the current OS context without browser integration.'
    };
}

async function getSmartContext(options = {}) {
    const platform = options.platform || process.platform;
    const clipboard = options.clipboard || null;
    const [activeWindow, selectedText] = await Promise.all([
        getActiveWindowContext(platform),
        getSelectedText(platform)
    ]);

    let clipboardText = '';
    try {
        clipboardText = clipboard && typeof clipboard.readText === 'function'
            ? truncateText(clipboard.readText())
            : '';
    } catch (_) {
        clipboardText = '';
    }

    const browser = await getBrowserContext(activeWindow, platform);
    return {
        capturedAt: new Date().toISOString(),
        platform,
        host: os.hostname(),
        activeWindow,
        currentApp: activeWindow ? {
            name: activeWindow.appName || activeWindow.processName || '',
            processName: activeWindow.processName || activeWindow.appName || '',
            pid: activeWindow.pid || null
        } : null,
        browser,
        selectedText,
        clipboardText
    };
}

module.exports = {
    getSmartContext,
    getActiveWindowContext,
    getBrowserContext,
    getSelectedText,
    truncateText,
    isBrowserProcess
};
