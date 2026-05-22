#!/usr/bin/env node
const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const binDir = __dirname;
const binary = path.join(binDir, 'oxrls' + (process.platform === 'win32' ? '.exe' : ''));

function buildBinary() {
  console.error('oxrls binary not found. Building from source...');
  execSync('cargo build --release --bin oxrls', { stdio: 'inherit', cwd: path.join(binDir, '..') });
  const targetDir = JSON.parse(
    execSync('cargo metadata --format-version=1 --no-dep-deps', { encoding: 'utf-8', cwd: path.join(binDir, '..') })
  ).target_directory;
  const builtBinary = path.join(targetDir, 'release', 'oxrls' + (process.platform === 'win32' ? '.exe' : ''));
  if (!fs.existsSync(builtBinary)) {
    console.error('Build failed: binary not found at', builtBinary);
    process.exit(1);
  }
  fs.copyFileSync(builtBinary, binary);
  fs.chmodSync(binary, 0o755);
}

if (!fs.existsSync(binary)) {
  buildBinary();
}

try {
  execSync(`"${binary}" ${process.argv.slice(2).map(a => `"${a}"`).join(' ')}`, {
    stdio: 'inherit',
    cwd: process.cwd(),
  });
} catch (e) {
  process.exit(e.status || 1);
}
