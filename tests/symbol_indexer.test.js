const fs = require('fs');
const os = require('os');
const path = require('path');

const { buildSymbolIndex, formatSymbolIndex } = require('../dist/src/CLI/symbol_indexer');

describe('symbol_indexer', () => {
    let tempDir;

    beforeEach(() => {
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-symbol-index-'));
        fs.mkdirSync(path.join(tempDir, 'src'));
        fs.writeFileSync(path.join(tempDir, 'src', 'app.js'), [
            'class App {}',
            'function start() {}',
            'const render = () => {};',
            'exports.stop = function stop() {};'
        ].join('\n'));
        fs.writeFileSync(path.join(tempDir, 'src', 'types.ts'), [
            'export interface User {}',
            'export type UserId = string;',
            'export enum Mode { Light }'
        ].join('\n'));
        fs.writeFileSync(path.join(tempDir, 'src', 'worker.py'), [
            'class Worker:',
            '    pass',
            'async def run_job():',
            '    pass'
        ].join('\n'));
        fs.mkdirSync(path.join(tempDir, 'node_modules'));
        fs.writeFileSync(path.join(tempDir, 'node_modules', 'ignored.js'), 'function ignored() {}');
    });

    afterEach(() => {
        fs.rmSync(tempDir, { recursive: true, force: true });
    });

    test('builds a symbol index for supported source files', () => {
        const index = buildSymbolIndex(tempDir);

        expect(index.root).toBe(tempDir);
        expect(index.fileCount).toBe(3);
        expect(index.symbols).toEqual(expect.arrayContaining([
            expect.objectContaining({ name: 'App', kind: 'class', file: path.join('src', 'app.js'), line: 1 }),
            expect.objectContaining({ name: 'start', kind: 'function', file: path.join('src', 'app.js'), line: 2 }),
            expect.objectContaining({ name: 'render', kind: 'function', file: path.join('src', 'app.js'), line: 3 }),
            expect.objectContaining({ name: 'stop', kind: 'export', file: path.join('src', 'app.js'), line: 4 }),
            expect.objectContaining({ name: 'User', kind: 'interface', file: path.join('src', 'types.ts'), line: 1 }),
            expect.objectContaining({ name: 'UserId', kind: 'type', file: path.join('src', 'types.ts'), line: 2 }),
            expect.objectContaining({ name: 'Mode', kind: 'enum', file: path.join('src', 'types.ts'), line: 3 }),
            expect.objectContaining({ name: 'Worker', kind: 'class', file: path.join('src', 'worker.py'), line: 1 }),
            expect.objectContaining({ name: 'run_job', kind: 'function', file: path.join('src', 'worker.py'), line: 3 })
        ]));
        expect(index.symbols).not.toEqual(expect.arrayContaining([
            expect.objectContaining({ name: 'ignored' })
        ]));
    });

    test('formats a readable symbol index', () => {
        const output = formatSymbolIndex(buildSymbolIndex(tempDir), { limit: 3 });

        expect(output).toContain('# Symbol Index');
        expect(output).toContain('Source files scanned: 3');
        expect(output).toContain('## By Kind');
        expect(output).toContain('## Symbols (first 3)');
    });
});
