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
      if (fs.existsSync(candidate)) {
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

const binaryPath = findBinary();

if (!binaryPath) {
  console.error(
    `\x1b[31mmint: native binary not found.\x1b[0m\n\n` +
    `Build or install the binary with:\n` +
    `  \x1b[32mcargo build -p mint-cli\x1b[0m              (fast debug build)\n` +
    `  \x1b[32mcargo build -p mint-cli --release\x1b[0m    (optimized release build)\n` +
    `  \x1b[32mcargo install --path crates/mint-cli\x1b[0m   (install permanently to ~/.cargo/bin)\n`
  );
  process.exit(1);
}

// Forward every argument straight through to the Rust binary.
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit'
});

child.on('error', (err) => {
  console.error(`\x1b[31mmint: failed to spawn binary at ${binaryPath}:\x1b[0m`, err.message);
  process.exit(1);
});

child.on('close', (code) => {
  process.exit(code ?? 0);
});
