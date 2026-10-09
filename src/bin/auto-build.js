const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawn } = require('node:child_process');
const readline = require('node:readline');

function fingerprint(root) {
  const hash = crypto.createHash('sha256');
  function visit(relative) {
    const full = path.join(root, relative);
    if (!fs.existsSync(full)) return;
    if (fs.statSync(full).isDirectory()) {
      for (const name of fs.readdirSync(full).sort()) {
        if (['target', '.git', 'node_modules'].includes(name)) continue;
        visit(path.join(relative, name));
      }
    } else {
      hash.update(relative); hash.update('\0'); hash.update(fs.readFileSync(full)); hash.update('\0');
    }
  }
  for (const name of ['src/bin/auto-build.js', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml', '.cargo', 'crates/mint-cli', 'crates/mint-core', 'crates/mint-companion-protocol', 'src-tauri/Cargo.toml', 'Release_Note.md', 'slash-commands.json', 'mcp-registry.json']) visit(name);
  for (const key of Object.keys(process.env).filter(k => /^(CARGO_|RUST|CC$|CXX$|AR$)/.test(k)).sort()) hash.update(`${key}=${process.env[key]}\0`);
  return hash.digest('hex');
}
function identity(binary) {
  const stat = fs.statSync(binary);
  fs.accessSync(binary, process.platform === 'win32' ? fs.constants.F_OK : fs.constants.X_OK);
  return `${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeMs}:${stat.ctimeMs}`;
}
async function autoBuild(root) {
  const stamp = path.join(root, 'target', '.mint-launcher-build.json');
  const before = fingerprint(root);
  try {
    const saved = JSON.parse(fs.readFileSync(stamp, 'utf8'));
    if (saved.fingerprint === before && saved.identity === identity(saved.binary)) return saved.binary;
  } catch { /* No successful matching build yet. */ }
  console.error('mint: source changed or build not verified; building latest CLI…');
  let binary;
  const child = spawn('cargo', ['build', '-p', 'mint-cli', '--release', '--locked', '--target-dir', path.join(root, 'target'), '--message-format=json-render-diagnostics'], { cwd: root, stdio: ['inherit', 'pipe', 'inherit'] });
  const lines = readline.createInterface({ input: child.stdout });
  lines.on('line', line => {
    try {
      const message = JSON.parse(line);
      if (message.reason === 'compiler-artifact' && message.target?.name === 'mint' && message.executable) binary = message.executable;
      if (message.reason === 'compiler-message' && message.message?.rendered) process.stderr.write(message.message.rendered);
    } catch { console.error(line); }
  });
  await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => {
      if (code === 0) resolve();
      else reject(Object.assign(new Error(`build failed${signal ? ` (${signal})` : ''}; latest CLI was not launched`), { exitCode: code || 1 }));
    });
  });
  if (fingerprint(root) !== before) throw new Error('source changed during build; run mint again to build latest CLI');
  if (!binary) throw new Error('build produced no mint executable; CLI was not launched');
  const state = { fingerprint: before, binary, identity: identity(binary) };
  fs.mkdirSync(path.dirname(stamp), { recursive: true });
  const temp = `${stamp}.${process.pid}.tmp`;
  fs.writeFileSync(temp, JSON.stringify(state));
  fs.renameSync(temp, stamp);
  return binary;
}
module.exports = { autoBuild };
