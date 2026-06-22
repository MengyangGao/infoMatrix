'use strict';

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const { install, cacheDir } = require('./install');
const { executablePath } = require('./platform');

async function launch(args = []) {
  const installDir = cacheDir();
  await install();

  const platform = process.platform;
  const target = executablePath(installDir, platform);

  if (!fs.existsSync(target)) {
    throw new Error(`InfoMatrix executable not found at ${target}`);
  }

  return new Promise((resolve, reject) => {
    let child;
    if (platform === 'darwin') {
      child = spawn('open', ['-W', target, '--args', ...args], { stdio: 'inherit' });
    } else if (platform === 'linux') {
      child = spawn(target, args, { stdio: 'inherit' });
    } else if (platform === 'win32') {
      child = spawn(target, args, { stdio: 'inherit', shell: false });
    } else {
      reject(new Error(`Unsupported platform: ${platform}`));
      return;
    }

    child.on('error', reject);
    child.on('exit', (code) => resolve(code));
  });
}

module.exports = { launch };
