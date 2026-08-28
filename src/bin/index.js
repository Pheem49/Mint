#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

// The `postinstall` script runs `cargo build -p mint-cli --release`, which
// produces the native binary here. `.exe` suffix on Windows.
const exe = process.platform === 'win32' ? 'mint.exe' : 'mint';
const binaryPath = path.join(__dirname, '..', '..', 'target', 'release', exe);

// Forward every argument straight through to the Rust binary.
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit'
});

child.on('error', (err) => {
  if (err.code === 'ENOENT') {
    console.error(
      `mint: native binary not found at ${binaryPath}\n` +
      `Reinstall with a Rust toolchain available: npm install -g @pheem49/mint`
    );
    process.exit(1);
  }
  throw err;
});

child.on('close', (code) => {
  process.exit(code);
});
