const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-desktop-launch-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const dir of ['src/bin', 'src/renderer', 'src-tauri/src', 'crates/mint-core/src', 'node_modules/@tauri-apps/cli', 'caller']) fs.mkdirSync(path.join(root, dir), { recursive: true });
  for (const name of fs.readdirSync(__dirname).filter(n => n.endsWith('.js') && !n.endsWith('.test.js'))) fs.copyFileSync(path.join(__dirname, name), path.join(root, 'src/bin', name));
  fs.writeFileSync(path.join(root, 'src/renderer/app.tsx'), 'first');
  fs.writeFileSync(path.join(root, 'src-tauri/src/main.rs'), 'first');
  const binary = path.join(root, 'target/release/mint-desktop');
  const app = '#!/usr/bin/env node\nconsole.log(JSON.stringify({args:process.argv.slice(2),cwd:process.cwd()}));process.exit(Number(process.env.APP_EXIT||0));\n';
  fs.mkdirSync(path.dirname(binary), { recursive: true });
  fs.writeFileSync(binary, app, { mode: 0o755 });
  fs.writeFileSync(path.join(root, 'node_modules/@tauri-apps/cli/tauri.js'), `
const fs=require('fs');
fs.appendFileSync(${JSON.stringify(path.join(root, 'builds'))},JSON.stringify({cwd:process.cwd(),args:process.argv.slice(2),lto:process.env.CARGO_PROFILE_RELEASE_LTO})+'\\n');
if(process.env.FAIL_BUILD){console.error('desktop compiler error');process.exit(19);}
fs.writeFileSync(${JSON.stringify(binary)},${JSON.stringify(app)},{mode:0o755});
fs.mkdirSync('out/renderer',{recursive:true});fs.writeFileSync('out/renderer/index.html','built UI');
fs.mkdirSync('src-tauri/gen/schemas',{recursive:true});fs.writeFileSync('src-tauri/gen/schemas/generated.json',String(Date.now()));
if(!process.argv.includes('--no-bundle')) {fs.mkdirSync('target/release/bundle/deb',{recursive:true});fs.writeFileSync('target/release/bundle/deb/Mint_1.16.0_amd64.deb','installer');}
if(process.env.EDIT_DURING_BUILD)fs.appendFileSync('src/renderer/app.tsx','edit');
`);
  return {
    root, binary,
    run(command = 'start', args = [], extra = {}) {
      return spawnSync(process.execPath, [path.join(root, 'src/bin/desktop.js'), command, ...args], { cwd: path.join(root, 'caller'), encoding: 'utf8', env: { ...process.env, ...extra } });
    },
    builds() { return fs.existsSync(path.join(root, 'builds')) ? fs.readFileSync(path.join(root, 'builds'), 'utf8').trim().split('\n').map(JSON.parse) : []; }
  };
}

test('start verifies existing desktop, skips unchanged build and rebuilds UI and Rust changes', t => {
  const f = fixture(t); let result = f.run('start', ['a b', '$(echo unsafe)']);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(f.builds().length, 1);
  assert.deepEqual(JSON.parse(result.stdout), { args: ['a b', '$(echo unsafe)'], cwd: path.join(f.root, 'caller') });
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 1);
  for (const file of ['src/renderer/app.tsx', 'src-tauri/src/main.rs', 'crates/mint-core/src/lib.rs', 'package-lock.json']) {
    fs.writeFileSync(path.join(f.root, file), 'changed');
    assert.equal(f.run().status, 0);
  }
  assert.equal(f.builds().length, 5);
  fs.unlinkSync(f.binary); assert.equal(f.run().status, 0); assert.equal(f.builds().length, 6);
});

test('failed desktop build stops before opening old binary and retries', t => {
  const f = fixture(t); const result = f.run('start', [], { FAIL_BUILD: '1' });
  assert.equal(result.status, 19); assert.equal(result.stdout, ''); assert.match(result.stderr, /desktop compiler error/);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 2);
});

test('full build does not launch and next start reuses it; rebuild and release options work', t => {
  const f = fixture(t); const built = f.run('build');
  assert.equal(built.status, 0); assert.equal(built.stdout, '');
  assert.deepEqual(f.builds()[0].args, ['build', '--no-bundle', '--', '--locked']);
  assert.equal(f.builds()[0].lto, undefined);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 1);
  assert.equal(f.run('start', ['--rebuild']).status, 0); assert.equal(f.builds().length, 2);
  assert.equal(f.builds()[1].lto, 'false');
  assert.equal(f.run('start', ['--release']).status, 0); assert.equal(f.builds().length, 3);
  assert.equal(f.run('start', ['--release'], { APP_EXIT: '7' }).status, 7);
});

test('changes during desktop build stop launch without claiming the build is current', t => {
  const f = fixture(t); const result = f.run('start', [], { EDIT_DURING_BUILD: '1' });
  assert.notEqual(result.status, 0); assert.equal(result.stdout, ''); assert.match(result.stderr, /changed during build/);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 2);
});


test('package produces the installer and a portable archive without launching desktop', t => {
  const f = fixture(t); const result = f.run('package');
  assert.equal(result.status, 0, result.stderr); assert.equal(result.stdout, '');
  assert.equal(fs.readFileSync(path.join(f.root, 'target/release/bundle/deb/mint-agent_amd64.deb'), 'utf8'), 'installer');
  const archive = path.join(f.root, 'target/release/bundle/tar/mint-agent.tar.gz');
  const listing = spawnSync('tar', ['-tzf', archive], { encoding: 'utf8' });
  assert.equal(listing.status, 0, listing.stderr); assert.match(listing.stdout, /mint-desktop-portable\/mint-desktop/);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 1);
});

test('custom Desktop build cannot validate the default start cache', t => {
  const f = fixture(t);
  assert.equal(f.run('build', ['--config', '{"app":{"windows":[]}}']).status, 0);
  assert.equal(f.run().status, 0);
  assert.equal(f.builds().length, 2);
});

test('protocol source edits invalidate a cached Desktop build', t => {
  const f = fixture(t);
  const protocol = path.join(f.root, 'crates/mint-companion-protocol/src');
  fs.mkdirSync(protocol, { recursive: true });
  fs.writeFileSync(path.join(protocol, 'lib.rs'), 'v1');
  assert.equal(f.run().status, 0);
  fs.writeFileSync(path.join(protocol, 'lib.rs'), 'v2');
  assert.equal(f.run().status, 0);
  assert.equal(f.builds().length, 2);
});
