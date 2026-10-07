const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-auto-build-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const dir of ['src/bin', 'crates/mint-cli/src', 'crates/mint-core/src', '.git', 'tools', 'caller']) fs.mkdirSync(path.join(root, dir), { recursive: true });
  for (const name of fs.readdirSync(__dirname).filter(n => n.endsWith('.js') && !n.endsWith('.test.js'))) fs.copyFileSync(path.join(__dirname, name), path.join(root, 'src/bin', name));
  fs.writeFileSync(path.join(root, 'Cargo.toml'), '[workspace]\n');
  fs.writeFileSync(path.join(root, 'crates/mint-cli/src/main.rs'), 'first');
  const binary = path.join(root, 'target/release/mint');
  const body = '#!/usr/bin/env node\nconsole.log(JSON.stringify({args:process.argv.slice(2),cwd:process.cwd(),version:"fresh"}));\n';
  fs.writeFileSync(path.join(root, 'tools/cargo'), `#!/usr/bin/env node
const fs=require('fs'); const path=require('path');
fs.appendFileSync(${JSON.stringify(path.join(root, 'builds'))}, process.cwd()+'\\n');
if(process.env.FAIL_BUILD) {console.error('compiler test error');process.exit(17);}
fs.mkdirSync(${JSON.stringify(path.dirname(binary))},{recursive:true});
fs.writeFileSync(${JSON.stringify(binary)},${JSON.stringify(body)},{mode:0o755});
console.log(JSON.stringify({reason:'compiler-artifact',target:{name:'mint',kind:['bin']},executable:${JSON.stringify(binary)}}));
if(process.env.EDIT_DURING_BUILD) fs.appendFileSync(${JSON.stringify(path.join(root, 'crates/mint-cli/src/main.rs'))},'changed');
`, { mode: 0o755 });
  const run = (extra = {}) => {
    const env = { ...process.env, PATH: path.join(root, 'tools') + path.delimiter + process.env.PATH, ...extra };
    delete env.MINT_BIN;
    return spawnSync(process.execPath, [path.join(root, 'src/bin/index.js'), '--test', 'a b', '$(echo unexpected)'], { cwd: path.join(root, 'caller'), env, encoding: 'utf8' });
  };
  const builds = () => fs.existsSync(path.join(root, 'builds')) ? fs.readFileSync(path.join(root, 'builds'), 'utf8').trim().split('\n') : [];
  return { root, run, builds, binary };
}

test('builds first launch, preserves caller cwd and arguments, skips unchanged launch', t => {
  const f = fixture(t); const first = f.run();
  assert.equal(first.status, 0, first.stderr);
  assert.deepEqual(JSON.parse(first.stdout), { args: ['--test', 'a b', '$(echo unexpected)'], cwd: path.join(f.root, 'caller'), version: 'fresh' });
  assert.deepEqual(f.builds(), [f.root]);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 1);
  fs.writeFileSync(path.join(f.root, 'crates/mint-cli/src/main.rs'), 'second');
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 2);
  fs.writeFileSync(path.join(f.root, 'mcp-registry.json'), '{}');
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 3);
  fs.unlinkSync(path.join(f.root, 'mcp-registry.json'));
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 4);
  fs.unlinkSync(f.binary);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 5);
});

test('failed rebuild shows compiler error, never launches old binary and retries next launch', t => {
  const f = fixture(t); assert.equal(f.run().status, 0);
  fs.writeFileSync(path.join(f.root, 'crates/mint-cli/src/main.rs'), 'changed');
  const failed = f.run({ FAIL_BUILD: '1' });
  assert.equal(failed.status, 17); assert.equal(failed.stdout, ''); assert.match(failed.stderr, /compiler test error/);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 3);
});

test('source edits during build do not launch or mark the build current', t => {
  const f = fixture(t); const result = f.run({ EDIT_DURING_BUILD: '1' });
  assert.notEqual(result.status, 0); assert.equal(result.stdout, '');
  assert.match(result.stderr, /changed during build/);
  assert.equal(f.run().status, 0); assert.equal(f.builds().length, 2);
});
