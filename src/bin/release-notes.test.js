const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

// Compile the actual build script and reader in an isolated, dependency-free
// crate so this catches Cargo invalidation as well as the returned text.
function fixture(t, checkout) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-release-notes-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const crate = path.join(root, 'crates/mint-core');
  fs.mkdirSync(crate, { recursive: true });
  if (checkout) fs.mkdirSync(path.join(root, '.git'));
  const repo = path.resolve(__dirname, '../..');
  fs.copyFileSync(path.join(repo, 'crates/mint-core/build.rs'), path.join(crate, 'build.rs'));
  fs.copyFileSync(path.join(repo, 'crates/mint-core/src/system/release_notes.rs'), path.join(crate, 'release_notes.rs'));
  fs.writeFileSync(path.join(crate, 'Cargo.toml'), '[package]\nname="notes-fixture"\nversion="0.1.0"\nedition="2024"\n[[bin]]\nname="notes-fixture"\npath="main.rs"\n');
  fs.writeFileSync(path.join(crate, 'main.rs'), 'mod release_notes; fn main() { print!("{}", release_notes::text()); }');
  const env = { ...process.env, CARGO_TARGET_DIR: path.join(root, 'target') };
  delete env.MINT_BUNDLE_RELEASE_NOTES;
  const binary = path.join(root, 'target/debug', process.platform === 'win32' ? 'notes-fixture.exe' : 'notes-fixture');
  const notes = path.join(root, 'Release_Note.md');
  const caller = path.join(root, 'unrelated-workspace');
  fs.mkdirSync(caller);
  fs.writeFileSync(path.join(caller, 'Release_Note.md'), 'unrelated workspace notes');
  const build = (bundle = false) => {
    const result = spawnSync('cargo', ['run', '--quiet', '--offline', '--manifest-path', path.join(crate, 'Cargo.toml')], {
      env: { ...env, ...(bundle ? { MINT_BUNDLE_RELEASE_NOTES: '1' } : {}) }, encoding: 'utf8',
    });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout;
  };
  const run = () => {
    const result = spawnSync(binary, [], { cwd: caller, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout;
  };
  return { root, notes, build, run };
}

test('checkout reads live notes without rebuilding and retains its embedded fallback', t => {
  const f = fixture(t, true);
  fs.writeFileSync(f.notes, 'first snapshot');
  assert.equal(f.build(), 'first snapshot');
  fs.writeFileSync(f.notes, 'latest development notes');
  assert.equal(f.run(), 'latest development notes');
  assert.equal(f.build(), 'latest development notes');
  fs.writeFileSync(f.notes, '   ');
  assert.equal(f.run(), 'first snapshot');
  fs.writeFileSync(f.notes, Buffer.from([255]));
  assert.equal(f.run(), 'first snapshot');
  fs.unlinkSync(f.notes);
  // Cargo must not rerun the build script for a dev note edit/deletion.
  assert.equal(f.build(), 'first snapshot');
});

test('bundled builds refresh embedded notes and ignore runtime checkout edits', t => {
  const f = fixture(t, true);
  fs.writeFileSync(f.notes, 'release one');
  assert.equal(f.build(true), 'release one');
  fs.writeFileSync(f.notes, 'runtime edits');
  assert.equal(f.run(), 'release one');
  assert.equal(f.build(true), 'runtime edits');
  fs.unlinkSync(f.notes);
  assert.equal(f.run(), 'runtime edits');
  // Switching back to dev mode refreshes the live reader even in the same target.
  fs.writeFileSync(f.notes, 'back to development');
  assert.equal(f.build(), 'back to development');
  fs.writeFileSync(f.notes, 'live again');
  assert.equal(f.run(), 'live again');
});

test('source packages without git bundle current notes for standalone use', t => {
  const f = fixture(t, false);
  fs.writeFileSync(f.notes, 'npm release notes');
  assert.equal(f.build(), 'npm release notes');
  fs.writeFileSync(f.notes, 'new package notes');
  assert.equal(f.run(), 'npm release notes');
  assert.equal(f.build(), 'new package notes');
  fs.unlinkSync(f.notes);
  assert.equal(f.run(), 'new package notes');
});
