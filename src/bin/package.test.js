const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

test('npm CLI package resolves every Cargo workspace member without shipping Live2D', t => {
  const root = path.resolve(__dirname, '../..');
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-package-test-'));
  t.after(() => fs.rmSync(temporary, { recursive: true, force: true }));
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const packed = spawnSync(npm, ['pack', '--dry-run', '--ignore-scripts', '--json', '--cache', path.join(temporary, 'cache')], { cwd: root, encoding: 'utf8', shell: process.platform === 'win32' });
  assert.equal(packed.status, 0, packed.stderr);
  const files = JSON.parse(packed.stdout)[0].files;
  assert.equal(files.some(file => /Live2DCubismCore|models\/Shiroko_Model/.test(file.path)), false, 'CLI npm package should leave character assets in the Companion installer');
  const extracted = path.join(temporary, 'package');
  for (const { path: name } of files) {
    const destination = path.join(extracted, name);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(path.join(root, name), destination);
  }
  const metadata = spawnSync('cargo', ['metadata', '--locked', '--offline', '--no-deps', '--format-version', '1', '--manifest-path', path.join(extracted, 'Cargo.toml')], { encoding: 'utf8' });
  assert.equal(metadata.status, 0, metadata.stderr);
  const packages = JSON.parse(metadata.stdout).packages.map(pkg => pkg.name);
  for (const name of ['mint-cli', 'mint-core', 'mint-desktop', 'mint-companion', 'mint-companion-protocol']) assert.ok(packages.includes(name), `${name} manifest missing from the installed package`);
});
