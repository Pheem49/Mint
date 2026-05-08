/**
 * Mint Spotify Plugin — Complete Edition
 * ----------------------------------------
 * Controls Spotify playback via playerctl (no OAuth required).
 * Supports: play, pause, next, previous, stop, shuffle, volume,
 *           now_playing, search (opens Spotify search URL).
 *
 * Requirements: playerctl installed (sudo apt install playerctl)
 * Spotify must be running (Desktop app or Snap).
 */

const { exec, execSync } = require('child_process');
const { promisify } = require('util');
const execAsync = promisify(exec);

// ── Helpers ────────────────────────────────────────────────────────────────

async function runPlayerctl(args) {
    try {
        const { stdout } = await execAsync(`playerctl -p spotify ${args}`);
        return { ok: true, output: stdout.trim() };
    } catch (err) {
        const msg = (err.stderr || err.message || '').toLowerCase();
        if (msg.includes('no players found') || msg.includes('could not find player')) {
            return { ok: false, error: 'spotify_not_running' };
        }
        if (err.code === 127) {
            return { ok: false, error: 'playerctl_missing' };
        }
        return { ok: false, error: err.message };
    }
}

function formatError(errorCode) {
    if (errorCode === 'spotify_not_running') {
        return '🎵 Spotify ยังไม่ได้เปิดอยู่นะคะ กรุณาเปิด Spotify ก่อนนะคะ';
    }
    if (errorCode === 'playerctl_missing') {
        return '⚠️ ไม่พบ playerctl กรุณาติดตั้งด้วยคำสั่ง: sudo apt install playerctl';
    }
    return `❌ เกิดข้อผิดพลาด: ${errorCode}`;
}

// ── Action Handlers ────────────────────────────────────────────────────────

const ACTION_MAP = {
    'play':     () => runPlayerctl('play'),
    'pause':    () => runPlayerctl('pause'),
    'stop':     () => runPlayerctl('stop'),
    'next':     () => runPlayerctl('next'),
    'previous': () => runPlayerctl('previous'),
    'prev':     () => runPlayerctl('previous'),
};

const ACTION_MESSAGES = {
    'play':     '▶️ เล่น Spotify แล้วค่ะ 🎵',
    'pause':    '⏸️ หยุดเพลงชั่วคราวแล้วค่ะ',
    'stop':     '⏹️ หยุด Spotify แล้วค่ะ',
    'next':     '⏭️ ข้ามไปเพลงถัดไปแล้วค่ะ 🎵',
    'previous': '⏮️ กลับไปเพลงก่อนหน้าแล้วค่ะ',
    'prev':     '⏮️ กลับไปเพลงก่อนหน้าแล้วค่ะ',
};

async function getNowPlaying() {
    const [title, artist, album, status] = await Promise.all([
        runPlayerctl('metadata title'),
        runPlayerctl('metadata artist'),
        runPlayerctl('metadata album'),
        runPlayerctl('status'),
    ]);

    if (!title.ok) return formatError(title.error);

    const statusIcon = (status.output || '').toLowerCase() === 'playing' ? '▶️' : '⏸️';
    const titleText  = title.output  || 'ไม่ทราบชื่อเพลง';
    const artistText = artist.output || 'ไม่ทราบศิลปิน';
    const albumText  = album.output  || '';

    let reply = `${statusIcon} กำลังเล่น: **${titleText}**\n`;
    reply += `🎤 ศิลปิน: ${artistText}`;
    if (albumText) reply += `\n💿 อัลบั้ม: ${albumText}`;
    return reply;
}

async function setVolume(levelStr) {
    const level = parseInt(levelStr, 10);
    if (isNaN(level) || level < 0 || level > 100) {
        return '⚠️ กรุณาระบุระดับเสียง 0-100 ค่ะ เช่น "volume 70"';
    }
    // playerctl volume uses 0.0–1.0
    const result = await runPlayerctl(`volume ${(level / 100).toFixed(2)}`);
    if (!result.ok) return formatError(result.error);
    return `🔊 ปรับเสียงเป็น ${level}% แล้วค่ะ`;
}

async function setShuffle(state) {
    // state: 'on' | 'off' | 'toggle'
    const shuffleState = state === 'on' ? 'On' : state === 'off' ? 'Off' : 'Toggle';
    const result = await runPlayerctl(`shuffle ${shuffleState}`);
    if (!result.ok) return formatError(result.error);
    if (state === 'toggle') return '🔀 สลับโหมด Shuffle แล้วค่ะ';
    return `🔀 Shuffle ${state === 'on' ? 'เปิด' : 'ปิด'}แล้วค่ะ`;
}

function searchSpotify(query) {
    if (!query || !query.trim()) {
        return '⚠️ กรุณาระบุคำที่ต้องการค้นหาด้วยนะคะ เช่น "search BTS"';
    }
    const encoded = encodeURIComponent(query.trim());
    const url = `https://open.spotify.com/search/${encoded}`;
    try {
        const { exec: execSync2 } = require('child_process');
        execSync2(`xdg-open "${url}"`, { detached: true, stdio: 'ignore' });
        return `🔍 เปิดค้นหา "${query}" ใน Spotify แล้วค่ะ 🎵`;
    } catch (_) {
        return `🔍 ค้นหา "${query}" ที่: ${url}`;
    }
}

// ── Main Plugin Export ─────────────────────────────────────────────────────

module.exports = {
    name: 'spotify',
    description: [
        'Controls Spotify playback and gets now-playing info.',
        'Valid targets:',
        '  "play" | "pause" | "stop" | "next" | "previous" — playback control',
        '  "now_playing" or "status" — get current song info',
        '  "volume <0-100>" — set volume level (e.g. "volume 70")',
        '  "shuffle on" | "shuffle off" | "shuffle toggle" — toggle shuffle',
        '  "search <query>" — search Spotify (e.g. "search BTS Dynamite")',
    ].join(' '),

    async execute(target) {
        const raw = (target || '').trim().toLowerCase();

        // ── Basic playback commands ───────────────────────────────────────
        if (ACTION_MAP[raw]) {
            const result = await ACTION_MAP[raw]();
            if (!result.ok) return formatError(result.error);
            return ACTION_MESSAGES[raw];
        }

        // ── Now Playing ───────────────────────────────────────────────────
        if (raw === 'now_playing' || raw === 'status' || raw === 'what\'s playing' || raw === 'current') {
            return await getNowPlaying();
        }

        // ── Volume ────────────────────────────────────────────────────────
        if (raw.startsWith('volume')) {
            const levelStr = raw.replace('volume', '').trim();
            return await setVolume(levelStr);
        }

        // ── Shuffle ───────────────────────────────────────────────────────
        if (raw.startsWith('shuffle')) {
            const state = raw.replace('shuffle', '').trim() || 'toggle';
            return await setShuffle(state);
        }

        // ── Search ────────────────────────────────────────────────────────
        if (raw.startsWith('search')) {
            const query = target.replace(/^search\s*/i, '').trim();
            return searchSpotify(query);
        }

        // ── Fallback: try as playerctl arg directly ───────────────────────
        return `⚠️ ไม่รู้จักคำสั่ง Spotify: "${target}"\nคำสั่งที่รองรับ: play, pause, stop, next, previous, now_playing, volume <0-100>, shuffle on/off, search <query>`;
    },

    // Expose helpers for testing
    _helpers: { runPlayerctl, getNowPlaying, setVolume, setShuffle, searchSpotify }
};
