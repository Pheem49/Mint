/**
 * Tests: docker.js plugin
 */

jest.mock('child_process', () => ({
    execFile: jest.fn()
}));

let docker;
let execFile;

beforeEach(() => {
    jest.resetModules();
    ({ execFile } = require('child_process'));
    execFile.mockReset();
    docker = require('../src/Plugins/docker');
});

describe('Docker Plugin', () => {
    test('lists running containers', async () => {
        execFile.mockImplementation((command, args, callback) => {
            callback(null, 'web (Up 2 hours)\n', '');
        });

        const result = await docker.execute('list');
        expect(execFile).toHaveBeenCalledWith('docker', ['ps', '--format', '{{.Names}} ({{.Status}})'], expect.any(Function));
        expect(result).toContain('Running Containers');
        expect(result).toContain('web (Up 2 hours)');
    });

    test('starts a named container without shell interpolation', async () => {
        execFile.mockImplementation((command, args, callback) => {
            callback(null, '', '');
        });

        const result = await docker.execute('start my-app');
        expect(execFile).toHaveBeenCalledWith('docker', ['start', 'my-app'], expect.any(Function));
        expect(result).toContain('Successfully executed "docker start"');
    });

    test('returns helpful message when command is invalid', async () => {
        const result = await docker.execute('remove my-app');
        expect(result).toContain('Invalid docker command');
        expect(execFile).not.toHaveBeenCalled();
    });
});
