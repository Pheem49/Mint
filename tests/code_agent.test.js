/**
 * Tests: code_agent helpers
 */

jest.mock('@google/genai', () => ({
    GoogleGenAI: jest.fn()
}));

jest.mock('axios', () => ({}));

jest.mock('../src/System/config_manager', () => ({
    readConfig: jest.fn(() => ({})),
    getAvailableProviders: jest.fn(() => ['ollama', 'gemini'])
}));

jest.mock('../src/CLI/code_session_memory', () => ({
    readWorkspaceSession: jest.fn(() => ({
        summary: '',
        lastTask: '',
        lastVerification: '',
        updatedAt: null
    })),
    writeWorkspaceSession: jest.fn()
}));

const fs = require('fs');
const path = require('path');
const os = require('os');

describe('code_agent helpers', () => {
    test('extractJson recovers JSON embedded in surrounding text', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const parsed = _helpers.extractJson('note\n{"action":"finish","input":{"summary":"ok"}}\nthanks');
        expect(parsed.action).toBe('finish');
        expect(parsed.input.summary).toBe('ok');
    });

    test('selectSupportedCodeProvider falls back away from unsupported code providers', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const selected = _helpers.selectSupportedCodeProvider(
            { aiProvider: 'ollama' },
            ['ollama', 'openai', 'gemini']
        );
        expect(selected).toBe('openai');
    });

    test('selectSupportedCodeProvider keeps configured supported provider when available', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const selected = _helpers.selectSupportedCodeProvider(
            { aiProvider: 'anthropic' },
            ['anthropic', 'gemini']
        );
        expect(selected).toBe('anthropic');
    });

    test('findPaths can locate directories by partial name', async () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-'));
        const targetDir = path.join(tempDir, 'projects', 'xidaidai');
        fs.mkdirSync(targetDir, { recursive: true });

        try {
            const result = await _helpers.findPaths(tempDir, 'xidaidai', 'dir');
            expect(result).toContain('[dir] projects/xidaidai');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    test('buildUnifiedDiffPreview formats patch approval preview as unified diff', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-diff-'));
        const targetFile = path.join(tempDir, 'demo.js');
        fs.writeFileSync(targetFile, [
            'function demo() {',
            '  const enabled = false;',
            '  return enabled;',
            '}'
        ].join('\n'));

        try {
            const preview = _helpers.buildUnifiedDiffPreview(tempDir, {
                path: 'demo.js',
                hunks: [{
                    oldText: '  const enabled = false;',
                    newText: '  const enabled = true;'
                }]
            });

            expect(preview).toContain('--- a/demo.js');
            expect(preview).toContain('+++ b/demo.js');
            expect(preview).toContain('@@ -1,4 +1,4 @@');
            expect(preview).toContain('-  const enabled = false;');
            expect(preview).toContain('+  const enabled = true;');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    test('buildUnifiedDiffPreview merges nearby hunks through git diff style output', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-merged-diff-'));
        const targetFile = path.join(tempDir, 'guide.md');
        fs.writeFileSync(targetFile, [
            '# Title',
            'Intro',
            '## One',
            'Body one',
            '## Two',
            'Body two'
        ].join('\n'));

        try {
            const preview = _helpers.buildUnifiedDiffPreview(tempDir, {
                path: 'guide.md',
                hunks: [
                    { oldText: '## One', newText: '## One - Updated' },
                    { oldText: '## Two', newText: '## Two - Updated' }
                ]
            });

            const hunkCount = (preview.match(/^@@/gm) || []).length;
            expect(hunkCount).toBe(1);
            expect(preview).toContain('-## One');
            expect(preview).toContain('+## One - Updated');
            expect(preview).toContain('-## Two');
            expect(preview).toContain('+## Two - Updated');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    test('formatPlanPreview displays a user-visible multi-file plan', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const preview = _helpers.formatPlanPreview({
            plan: [
                'Update src/CLI/code_agent.js',
                '- Add tests in tests/code_agent.test.js'
            ],
            files: ['src/CLI/code_agent.js', 'tests/code_agent.test.js']
        });

        expect(preview).toBe([
            'Plan:',
            '- Update src/CLI/code_agent.js',
            '- Add tests in tests/code_agent.test.js'
        ].join('\n'));
    });

    test('formatPlanPreview normalizes common Thai plan verbs to English', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const preview = _helpers.formatPlanPreview({
            plan: [
                'แก้ index.html',
                '- สร้าง styles.css',
                'เพิ่ม tests/code_agent.test.js'
            ],
            files: ['index.html', 'styles.css', 'tests/code_agent.test.js']
        });

        expect(preview).toBe([
            'Plan:',
            '- Update index.html',
            '- Create styles.css',
            '- Add tests/code_agent.test.js'
        ].join('\n'));
    });

    test('formatPlanMarkdown renders plan file content with task and files', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const markdown = _helpers.formatPlanMarkdown({
            plan: ['Update src/CLI/code_agent.js'],
            files: ['src/CLI/code_agent.js']
        }, {
            task: 'เพิ่ม plan file approval',
            createdAt: '2026-05-26T00:00:00.000Z'
        });

        expect(markdown).toContain('# Mint Plan');
        expect(markdown).toContain('Created: 2026-05-26T00:00:00.000Z');
        expect(markdown).toContain('## Task\n\nเพิ่ม plan file approval');
        expect(markdown).toContain('Plan:\n- Update src/CLI/code_agent.js');
        expect(markdown).toContain('## Expected Files\n\n- src/CLI/code_agent.js');
        expect(markdown).toContain('Status: Pending user approval');
    });

    test('formatPlanApprovalSummary summarizes planned file changes for compact UI', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        expect(_helpers.formatPlanApprovalSummary({
            plan: ['Update index.html', 'Create styles.css'],
            files: ['index.html', 'styles.css']
        })).toBe('2 planned changes across 2 files.');
    });

    test('writePlanFile writes mint_plan.md to the configured Mint directory', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-plan-'));

        try {
            const planPath = path.join(tempDir, 'mint_plan.md');
            const result = _helpers.writePlanFile(tempDir, {
                plan: ['Update README.md'],
                files: ['README.md']
            }, {
                task: 'ทดสอบ plan file',
                createdAt: '2026-05-26T00:00:00.000Z',
                planPath
            });

            expect(result.path).toBe(planPath);
            expect(fs.existsSync(planPath)).toBe(true);
            expect(fs.readFileSync(planPath, 'utf8')).toBe(result.content);
            expect(result.content).toContain('ทดสอบ plan file');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    test('updatePlanApprovalStatus rewrites plan file with approved state', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-plan-status-'));

        try {
            const planPath = path.join(tempDir, 'mint_plan.md');
            const input = {
                plan: ['แก้ README.md'],
                files: ['README.md']
            };
            const planFile = _helpers.writePlanFile(tempDir, input, {
                task: 'ทดสอบ approval status',
                createdAt: '2026-05-26T00:00:00.000Z',
                planPath
            });
            const updated = _helpers.updatePlanApprovalStatus(planFile, input, {
                task: 'ทดสอบ approval status',
                createdAt: '2026-05-26T00:00:00.000Z',
                approvalStatus: 'Approved',
                approvalTime: '2026-05-26T00:01:00.000Z'
            });
            const written = fs.readFileSync(planPath, 'utf8');

            expect(updated.content).toBe(written);
            expect(written).toContain('Status: Approved');
            expect(written).toContain('Approved: 2026-05-26T00:01:00.000Z');
            expect(written).not.toContain('Status: Pending user approval');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    test('formatWritePreview renders full-file writes as unified diff', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-write-diff-'));
        const targetFile = path.join(tempDir, 'demo.txt');
        fs.writeFileSync(targetFile, 'old\n');

        try {
            const preview = _helpers.formatWritePreview(tempDir, 'demo.txt', 'new\n');
            expect(preview).toContain('--- a/demo.txt');
            expect(preview).toContain('+++ b/demo.txt');
            expect(preview).toContain('-old');
            expect(preview).toContain('+new');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    test('buildApprovalWarnings flags scratch paths and mismatched bio guide content', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const warnings = _helpers.buildApprovalWarnings(
            'scratch/rag_test_folder/bio.txt',
            '# NPM Publishing Guide\nRun npm publish.'
        );

        expect(warnings.join('\n')).toMatch(/scratch/);
        expect(warnings.join('\n')).toMatch(/profile\/bio|guide or publishing/);
    });

    test('validateEditExplanation requires file and reason before edits', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        expect(_helpers.validateEditExplanation('write_file', {
            path: 'src/demo.js'
        }, 'I will edit src/demo.js because this file owns the demo behavior.')).toEqual({ ok: true });

        expect(_helpers.validateEditExplanation('write_file', {
            path: 'src/demo.js'
        }, 'I will make the change now.').ok).toBe(false);

        expect(_helpers.validateEditExplanation('apply_patch', {
            patch: { path: 'scratch/demo.txt' }
        }, 'I will edit scratch/demo.txt because this is intentionally disposable test content.')).toEqual({ ok: true });

        expect(_helpers.validateEditExplanation('apply_patch', {
            patch: { path: 'scratch/demo.txt' }
        }, 'I will edit scratch/demo.txt because it has the target text.').ok).toBe(false);
    });

    test('requiresMultiFilePlan blocks a second file edit without approved plan', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const editPlanState = {
            approved: false,
            touchedFiles: new Set(['src/CLI/code_agent.js'])
        };

        expect(_helpers.requiresMultiFilePlan('apply_patch', {
            patch: { path: 'tests/code_agent.test.js' }
        }, editPlanState)).toBe(true);

        expect(_helpers.requiresMultiFilePlan('apply_patch', {
            patch: { path: 'src/CLI/code_agent.js' }
        }, editPlanState)).toBe(false);

        editPlanState.approved = true;
        expect(_helpers.requiresMultiFilePlan('write_file', {
            path: 'tests/code_agent.test.js'
        }, editPlanState)).toBe(false);
    });

    test('getMissingPlanFiles reports approved plan files that were not touched', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const editPlanState = {
            approved: true,
            expectedFiles: new Set(['celebrate.html', 'celebrate.css', 'celebrate.js', 'script.js']),
            touchedFiles: new Set(['celebrate.html', 'celebrate.css', 'celebrate.js'])
        };

        expect(_helpers.getMissingPlanFiles(editPlanState)).toEqual(['script.js']);

        editPlanState.touchedFiles.add('script.js');
        expect(_helpers.getMissingPlanFiles(editPlanState)).toEqual([]);
    });

});
