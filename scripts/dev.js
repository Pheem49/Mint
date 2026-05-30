const { spawn } = require('child_process');
const path = require('path');

// 1. Start Vite dev server for the React renderer
const vite = spawn('npx', ['vite'], {
  stdio: 'inherit',
  shell: true
});

// 2. Start TypeScript compiler in watch mode for the Node main/preload processes
const tsc = spawn('npx', ['tsc', '-p', 'tsconfig.node.json', '-w'], {
  stdio: 'inherit',
  shell: true
});

// 3. Launch Electron once the compilation completes initially
let electronProcess = null;
function startElectron() {
  if (electronProcess) {
    electronProcess.kill();
  }
  
  electronProcess = spawn('npx', ['electron', '.', '--no-sandbox'], {
    stdio: 'inherit',
    shell: true,
    env: {
      ...process.env,
      ELECTRON_RENDERER_URL: 'http://localhost:9000'
    }
  });

  electronProcess.on('close', () => {
    // Electron closed, shutdown helper processes
    console.log('Electron closed, exiting...');
    vite.kill('SIGTERM');
    tsc.kill('SIGTERM');
    setTimeout(() => process.exit(0), 500);
  });
}

process.on('SIGTERM', () => {
  process.emit('SIGINT');
});

// Allow 3.5 seconds for Vite and TypeScript to start and build the first time, then run Electron
setTimeout(() => {
  startElectron();
}, 3500);

// Clean exit handlers
let isExiting = false;

process.on('SIGINT', () => {
  if (isExiting) return;
  isExiting = true;
  
  console.log('\n🛑 Shutting down...');
  
  // Kill all child processes
  try {
    process.kill(-vite.pid, 'SIGTERM');
  } catch (e) {}
  
  try {
    process.kill(-tsc.pid, 'SIGTERM');
  } catch (e) {}
  
  if (electronProcess) {
    try {
      process.kill(-electronProcess.pid, 'SIGTERM');
    } catch (e) {}
  }
  
  // Force exit after 2 seconds if still running
  setTimeout(() => {
    console.log('Force exiting...');
    process.exit(0);
  }, 2000);
});

process.on('exit', () => {
  try { vite.kill('SIGKILL'); } catch (e) {}
  try { tsc.kill('SIGKILL'); } catch (e) {}
  if (electronProcess) { try { electronProcess.kill('SIGKILL'); } catch (e) {} }
});
