/**
 * Tests: system_monitor.js plugin
 */

const systemMonitor = require('../src/Plugins/system_monitor');
const os = require('os');

describe('System Monitor Plugin', () => {
    test('has required plugin fields', () => {
        expect(systemMonitor.name).toBe('system_monitor');
        expect(typeof systemMonitor.description).toBe('string');
        expect(typeof systemMonitor.execute).toBe('function');
    });

    test('returns stats for "stats" target', async () => {
        const result = await systemMonitor.execute('stats');
        expect(result).toContain('System Health Report');
        expect(result).toContain('CPU Load');
        expect(result).toContain('Memory');
    });

    test('returns cpu info for "cpu" target', async () => {
        const result = await systemMonitor.execute('cpu');
        expect(result).toContain('CPU Load');
        expect(result).toContain('Cores');
    });

    test('returns memory info for "memory" target', async () => {
        const result = await systemMonitor.execute('memory');
        expect(result).toContain('Memory Status');
    });

    test('returns disk info for "disk" target', async () => {
        const result = await systemMonitor.execute('disk');
        expect(result).toContain('Disk Status');
    });
});
