#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawn } = require('node:child_process');
const root = path.resolve(__dirname, '../..');
const target = path.join(root, 'target');
const stamp = path.join(target, '.mint-desktop-build.json');
const executable = process.platform === 'win32' ? 'mint-desktop.exe' : 'mint-desktop';

function digest(inputs) {
  const hash = crypto.createHash('sha256');
  function visit(relative) {
    const full = path.join(root, relative);
    if (!fs.existsSync(full)) return;
    if (fs.statSync(full).isDirectory()) {
      for (const name of fs.readdirSync(full).sort()) {
        // Tauri rewrites schemas while building; they are outputs, not inputs.
        if (['target', '.git', 'node_modules'].includes(name) || relative === 'src-tauri' && name === 'gen') continue;
        visit(path.join(relative, name));
      }
    } else {
      hash.update(relative); hash.update('\0'); hash.update(fs.readFileSync(full)); hash.update('\0');
    }
  }
  inputs.forEach(visit);
  return hash.digest('hex');
}
function fingerprint() {
  const source = digest(['src/bin/desktop.js', 'src-tauri', 'crates/mint-core', 'crates/mint-companion-protocol', 'src/renderer', 'public', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml', '.cargo', 'package.json', 'package-lock.json', 'vite.config.ts', 'tsconfig.json', 'tsconfig.node.json', '.env', '.env.local', '.env.production', '.env.production.local', 'Release_Note.md', 'slash-commands.json', 'mcp-registry.json']);
  const env = Object.keys(process.env).filter(k => /^(CARGO_|RUST|VITE_|TAURI_|CC$|CXX$|AR$)/.test(k)).sort().map(k => [k, process.env[k]]);
  return crypto.createHash('sha256').update(source).update(JSON.stringify(env)).digest('hex');
}
function identity(binary) {
  fs.accessSync(binary, process.platform === 'win32' ? fs.constants.F_OK : fs.constants.X_OK);
  const stat = fs.statSync(binary);
  return `${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeMs}:${stat.ctimeMs}`;
}
function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: 'inherit', ...options });
    child.once('error', reject);
    child.once('close', (code, signal) => {
      if (code === 0) resolve();
      else reject(Object.assign(new Error(`${path.basename(command)} failed${signal ? ` (${signal})` : ''}`), { exitCode: code || 1 }));
    });
  });
}
async function build({ fast = false, bundle = false, args = [] } = {}) {
  const before = fingerprint();
  const debug = args.includes('--debug');
  const custom = args.some(arg => arg !== '--no-bundle');
  if (custom && !debug) fs.rmSync(stamp, { force: true });
  // Custom output targets must never accidentally validate an older host binary.
  if (process.env.CARGO_BUILD_TARGET || args.some(a => a === '--target' || a.startsWith('--target='))) throw new Error('custom build targets are not supported by this Desktop launcher');
  const binary = path.join(target, debug ? 'debug' : 'release', executable);
  const env = { ...process.env, CARGO_TARGET_DIR: target };
  if (fast) Object.assign(env, { CARGO_PROFILE_RELEASE_LTO: 'false', CARGO_PROFILE_RELEASE_CODEGEN_UNITS: '8' });
  console.error(`mint: building ${fast ? 'fast' : 'full'} Desktop${bundle ? ' package' : ''}…`);
  const tauri = path.join(root, 'node_modules/@tauri-apps/cli/tauri.js');
  await run(process.execPath, [tauri, 'build', ...(bundle ? [] : ['--no-bundle']), ...args, '--', '--locked'], { cwd: root, env });
  if (fingerprint() !== before) throw new Error('source changed during build; run the command again');
  const state = { fingerprint: before, binary, identity: identity(binary), mode: fast ? 'fast' : 'full', assets: digest(['out/renderer']) };
  if (!debug && !custom) {
    fs.mkdirSync(target, { recursive: true });
    const temporary = `${stamp}.${process.pid}.tmp`;
    fs.writeFileSync(temporary, JSON.stringify(state)); fs.renameSync(temporary, stamp);
  }
  return binary;
}
async function start(args) {
  const force = args.includes('--rebuild');
  const full = args.includes('--release');
  const appArgs = args.filter(arg => !['--rebuild', '--release'].includes(arg));
  let binary;
  if (!force) {
    try {
      const state = JSON.parse(fs.readFileSync(stamp, 'utf8'));
      if (state.fingerprint === fingerprint() && state.identity === identity(state.binary) && state.assets === digest(['out/renderer']) && (!full || state.mode === 'full')) binary = state.binary;
    } catch { /* Missing or invalid success receipt requires a build. */ }
  }
  if (!binary) binary = await build({ fast: !full });
  await run(binary, appArgs);
}
async function packageDesktop(args) {
  const binary = await build({ bundle: true, args });
  // Keep Tauri's native Windows/macOS installers; Linux also gets a portable tarball.
  if (process.platform !== 'linux' || args.includes('--no-bundle')) return;
  const output = path.dirname(binary);
  const portable = path.join(output, 'mint-desktop-portable');
  fs.mkdirSync(portable, { recursive: true });
  fs.copyFileSync(binary, path.join(portable, executable));
  const tarDir = path.join(output, 'bundle/tar');
  fs.mkdirSync(tarDir, { recursive: true });
  try {
    await run('tar', ['-czf', path.join(tarDir, 'mint-agent.tar.gz'), '-C', output, 'mint-desktop-portable'], { cwd: root });
  } finally { fs.rmSync(portable, { recursive: true, force: true }); }
  const debDir = path.join(output, 'bundle/deb');
  const installers = fs.readdirSync(debDir).filter(name => /^Mint_.*_amd64\.deb$/.test(name));
  if (installers.length !== 1) throw new Error('expected one Linux amd64 installer from Tauri');
  fs.renameSync(path.join(debDir, installers[0]), path.join(debDir, 'mint-agent_amd64.deb'));
}
async function main() {
  const [command = 'start', ...args] = process.argv.slice(2);
  if (command === 'start') await start(args);
  else if (command === 'build') await build({ fast: args.includes('--fast'), args: args.filter(a => a !== '--fast') });
  else if (command === 'package') await packageDesktop(args);
  else throw new Error(`unknown Desktop command: ${command}`);
}
main().catch(error => {
  console.error(`mint: ${error.message}`);
  process.exitCode = error.exitCode || 1;
});
