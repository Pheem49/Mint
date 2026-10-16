const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function runLauncher(t, launcher, args, exitCode = 0) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-web-launcher-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const binary = path.join(directory, 'mint');
  fs.writeFileSync(binary, '#!/usr/bin/env node\nconsole.log(JSON.stringify(process.argv.slice(2)));\nprocess.exit(Number(process.env.MINT_TEST_EXIT));\n');
  fs.chmodSync(binary, 0o755);
  return spawnSync(process.execPath, [launcher, ...args], {
    encoding: 'utf8',
    env: { ...process.env, MINT_BIN: binary, MINT_TEST_EXIT: String(exitCode) },
  });
}

test('mint-web launches the web command and forwards arguments without shell expansion', (t) => {
  const result = runLauncher(t, path.join(__dirname, 'web.js'), ['--dev', 'a b', '$(echo unexpected)']);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), ['web', '--dev', 'a b', '$(echo unexpected)']);
});

test('the registered mint-web executable preserves the native exit code', (t) => {
  const packageRoot = path.join(__dirname, '../..');
  const packageConfig = require(path.join(packageRoot, 'package.json'));
  const launcher = path.resolve(packageRoot, packageConfig.bin['mint-web'] || 'missing-mint-web');
  const result = runLauncher(t, launcher, ['--help'], 7);
  assert.equal(result.status, 7, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), ['web', '--help']);
});

test('mint still forwards commands without adding web', (t) => {
  const result = runLauncher(t, path.join(__dirname, 'index.js'), ['--resume', 'cli']);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), ['--resume', 'cli']);
});
