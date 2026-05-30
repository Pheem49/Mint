const fs = require('fs');
const os = require('os');
const path = require('path');

const { summarizeRepository, formatRepoSummary } = require('../dist/src/CLI/repo_summarizer');

describe('repo_summarizer', () => {
    let tempDir;

    beforeEach(() => {
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-repo-summary-'));
        fs.mkdirSync(path.join(tempDir, 'src'));
        fs.mkdirSync(path.join(tempDir, 'tests'));
        fs.writeFileSync(path.join(tempDir, 'README.md'), '# Demo\n');
        fs.writeFileSync(path.join(tempDir, 'src', 'index.js'), 'console.log("hi");\n');
        fs.writeFileSync(path.join(tempDir, 'tests', 'index.test.js'), 'test("ok", () => {});\n');
        fs.writeFileSync(path.join(tempDir, 'package.json'), JSON.stringify({
            name: 'demo-app',
            version: '0.1.0',
            description: 'Demo package',
            scripts: {
                test: 'jest',
                start: 'node src/index.js'
            },
            dependencies: {
                commander: '^1.0.0'
            },
            devDependencies: {
                jest: '^1.0.0'
            }
        }, null, 2));
    });

    afterEach(() => {
        fs.rmSync(tempDir, { recursive: true, force: true });
    });

    test('summarizes package, directories, languages, and important files', () => {
        const summary = summarizeRepository(tempDir);

        expect(summary.root).toBe(tempDir);
        expect(summary.package.name).toBe('demo-app');
        expect(summary.package.scripts).toContain('test');
        expect(summary.topDirs).toEqual(expect.arrayContaining([
            expect.objectContaining({ dir: 'src', count: 1 }),
            expect.objectContaining({ dir: 'tests', count: 1 })
        ]));
        expect(summary.languages).toEqual(expect.arrayContaining([
            expect.objectContaining({ language: 'JavaScript', count: 2 }),
            expect.objectContaining({ language: 'Markdown', count: 1 })
        ]));
        expect(summary.importantFiles).toEqual(expect.arrayContaining([
            'README.md',
            'package.json',
            path.join('tests', 'index.test.js')
        ]));
    });

    test('formats a readable markdown summary', () => {
        const output = formatRepoSummary(summarizeRepository(tempDir));

        expect(output).toContain('# Repository Summary');
        expect(output).toContain('Package: demo-app v0.1.0');
        expect(output).toContain('Description: Demo package\n\nFiles scanned:');
        expect(output).toContain('## Package Scripts');
        expect(output).toContain('- test');
        expect(output).toContain('## Important Files');
    });

    test('formats multi-line git status and diff readably', () => {
        const output = formatRepoSummary({
            root: tempDir,
            fileCount: 2,
            topDirs: [],
            languages: [],
            importantFiles: [],
            package: null,
            git: {
                isRepo: true,
                branch: 'main',
                status: ' M one.js\n?? two.js',
                diffStat: 'one.js | 2 +-\ntwo.js | 1 +'
            }
        });

        expect(output).toContain('- Status:\n   M one.js\n  ?? two.js');
        expect(output).toContain('- Diff:\n  one.js | 2 +-\n  two.js | 1 +');
    });
});
