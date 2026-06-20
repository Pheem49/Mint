#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

// ชี้ไปยังไฟล์ Rust Binary ที่คอมไพล์สำเร็จแล้วในเครื่องผู้ใช้
const binaryPath = path.join(__dirname, '..', '..', 'bin', 'mint');

// สั่งทำงานไฟล์ Binary โดยส่งอาร์กิวเมนต์ทั้งหมดต่อเข้าไป
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit'
});

child.on('close', (code) => {
  process.exit(code);
});
