/**
 * Tests: spotify.js plugin
 * Tests the plugin interface and all action handlers.
 * Mocks exec/playerctl so no real Spotify needed.
 */

// ── Mock child_process before requiring the module ─────────────────────────
jest.mock('child_process', () => ({
    exec: jest.fn(),
    execSync: jest.fn(),
    promisify: undefined, // will use our mock exec
}));

// Mock promisify to return our mock exec as async
const { exec: mockExec } = require('child_process');
jest.mock('util', () => ({
    ...jest.requireActual('util'),
    promisify: (fn) => {
        // Return an async wrapper around mockExec
        return (...args) => new Promise((resolve, reject) => {
            mockExec(...args, (err, stdout, stderr) => {
                if (err) reject(err);
                else resolve({ stdout: stdout || '', stderr: stderr || '' });
            });
        });
    }
}));

let spotify;

beforeEach(() => {
    jest.resetModules();
    jest.clearAllMocks();
    spotify = require('../src/Plugins/spotify');
});

// ── Plugin interface tests ─────────────────────────────────────────────────

describe('Spotify Plugin — interface', () => {
    test('has required plugin fields', () => {
        expect(spotify.name).toBe('spotify');
        expect(typeof spotify.description).toBe('string');
        expect(typeof spotify.execute).toBe('function');
    });

    test('description mentions valid commands', () => {
        expect(spotify.description).toMatch(/play/);
        expect(spotify.description).toMatch(/pause/);
        expect(spotify.description).toMatch(/next/);
    });
});

// ── Playback control tests ─────────────────────────────────────────────────

describe('Spotify Plugin — playback commands', () => {
    const playbackCmds = ['play', 'pause', 'stop', 'next', 'previous'];

    playbackCmds.forEach(cmd => {
        test(`executes "${cmd}" and returns success message`, async () => {
            mockExec.mockImplementation((command, callback) => {
                callback(null, '', '');
            });

            const result = await spotify.execute(cmd);
            expect(typeof result).toBe('string');
            expect(result.length).toBeGreaterThan(0);
            // Should not contain error text
            expect(result).not.toMatch(/เกิดข้อผิดพลาด/);
        });
    });

    test('returns "spotify not running" error when playerctl reports no players', async () => {
        const err = new Error('No players found');
        err.stderr = 'No players found';
        mockExec.mockImplementation((command, callback) => {
            callback(err, '', 'No players found');
        });

        const result = await spotify.execute('play');
        expect(result).toContain('Spotify ยังไม่ได้เปิดอยู่');
    });

    test('returns playerctl missing error when exit code is 127', async () => {
        const err = new Error('command not found: playerctl');
        err.code = 127;
        mockExec.mockImplementation((command, callback) => {
            callback(err, '', '');
        });

        const result = await spotify.execute('play');
        expect(result).toContain('playerctl');
    });
});

// ── Now Playing tests ──────────────────────────────────────────────────────

describe('Spotify Plugin — now_playing', () => {
    test('returns formatted now playing string', async () => {
        let callCount = 0;
        mockExec.mockImplementation((command, callback) => {
            callCount++;
            // Mock: title, artist, album, status calls
            if (command.includes('title'))  callback(null, 'Dynamite\n', '');
            else if (command.includes('artist')) callback(null, 'BTS\n', '');
            else if (command.includes('album'))  callback(null, 'BE\n', '');
            else if (command.includes('status')) callback(null, 'Playing\n', '');
            else callback(null, '', '');
        });

        const result = await spotify.execute('now_playing');
        expect(result).toContain('Dynamite');
        expect(result).toContain('BTS');
    });

    test('"status" alias also triggers now_playing', async () => {
        mockExec.mockImplementation((command, callback) => {
            if (command.includes('title'))  callback(null, 'Test Song\n', '');
            else if (command.includes('artist')) callback(null, 'Test Artist\n', '');
            else if (command.includes('album'))  callback(null, 'Test Album\n', '');
            else if (command.includes('status')) callback(null, 'Paused\n', '');
            else callback(null, '', '');
        });

        const result = await spotify.execute('status');
        expect(result).toContain('Test Song');
    });
});

// ── Volume tests ───────────────────────────────────────────────────────────

describe('Spotify Plugin — volume', () => {
    test('sets volume to valid level', async () => {
        mockExec.mockImplementation((command, callback) => {
            callback(null, '', '');
        });

        const result = await spotify.execute('volume 70');
        expect(result).toContain('70%');
    });

    test('rejects invalid volume level > 100', async () => {
        const result = await spotify.execute('volume 150');
        expect(result).toContain('0-100');
    });

    test('rejects non-numeric volume', async () => {
        const result = await spotify.execute('volume abc');
        expect(result).toContain('0-100');
    });
});

// ── Shuffle tests ──────────────────────────────────────────────────────────

describe('Spotify Plugin — shuffle', () => {
    test('enables shuffle with "shuffle on"', async () => {
        mockExec.mockImplementation((command, callback) => {
            callback(null, '', '');
        });

        const result = await spotify.execute('shuffle on');
        expect(result).toContain('Shuffle');
    });

    test('disables shuffle with "shuffle off"', async () => {
        mockExec.mockImplementation((command, callback) => {
            callback(null, '', '');
        });

        const result = await spotify.execute('shuffle off');
        expect(result).toContain('Shuffle');
    });
});

// ── Search tests ───────────────────────────────────────────────────────────

describe('Spotify Plugin — search', () => {
    test('returns message with search query', async () => {
        // xdg-open will fail in test env, but searchSpotify catches and returns URL
        const result = await spotify.execute('search BTS Dynamite');
        expect(result).toContain('BTS Dynamite');
    });

    test('empty search query returns error message', async () => {
        const result = await spotify.execute('search ');
        expect(result).toContain('กรุณาระบุ');
    });
});

// ── Unknown command tests ──────────────────────────────────────────────────

describe('Spotify Plugin — unknown command', () => {
    test('returns helpful error for unknown target', async () => {
        const result = await spotify.execute('dance');
        expect(result).toContain('ไม่รู้จักคำสั่ง');
    });

    test('returns helpful error for empty target', async () => {
        const result = await spotify.execute('');
        expect(result).toContain('ไม่รู้จักคำสั่ง');
    });
});
