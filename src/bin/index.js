#!/usr/bin/env node

const { spawn } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const exe = process.platform === 'win32' ? 'mint.exe' : 'mint';

function findBinary() {
  const candidates = [];

  // 1. Explicit override via environment variable
  if (process.env.MINT_BIN) {
    candidates.push(process.env.MINT_BIN);
  }

  // 2. Local workspace release build
  candidates.push(path.join(__dirname, '..', '..', 'target', 'release', exe));

  // 3. Local workspace debug build (fast dev iteration)
  candidates.push(path.join(__dirname, '..', '..', 'target', 'debug', exe));

  // 4. Cargo global install (~/.cargo/bin/mint)
  candidates.push(path.join(os.homedir(), '.cargo', 'bin', exe));

  // 5. System local bin (~/.local/bin/mint)
  candidates.push(path.join(os.homedir(), '.local', 'bin', exe));

  // 6. Global system paths
  if (process.platform !== 'win32') {
    candidates.push(`/usr/local/bin/${exe}`);
    candidates.push(`/usr/bin/${exe}`);
  }

  for (const candidate of candidates) {
    try {
      if (fs.existsSync(candidate) && fs.realpathSync(candidate) !== fs.realpathSync(__filename)) {
        if (process.platform !== 'win32') {
          fs.accessSync(candidate, fs.constants.X_OK);
        }
        return candidate;
      }
    } catch {
      // Continue to next candidate
    }
  }

  return null;
}

async function main() {
  const root = path.resolve(__dirname, '../..');
  const sourceCheckout = fs.existsSync(path.join(root, '.git')) && fs.existsSync(path.join(root, 'crates/mint-cli/src/main.rs'));
  // Explicit overrides and distributed packages keep their existing launch behavior.
  const binaryPath = !process.env.MINT_BIN && sourceCheckout
    ? await require('./auto-build').autoBuild(root)
    : findBinary();
  if (!binaryPath) throw new Error('native binary not found; run npm run build:cli first');
  const child = spawn(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
  child.once('error', err => { console.error(`mint: ${err.message}`); process.exitCode = 1; });
  child.once('close', (code, signal) => {
    if (signal) process.kill(process.pid, signal);
    else process.exitCode = code ?? 1;
  });
}
main().catch(err => {
  console.error(`mint: ${err.message}`);
  process.exitCode = err.exitCode || 1;
});
