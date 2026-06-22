'use strict';

const path = require('path');

const REPO = 'MengyangGao/infoMatrix';

function resolvePlatform(platform, arch) {
  if (platform === 'darwin') {
    return {
      artifact: 'InfoMatrix-macos.zip',
      checksumFile: 'InfoMatrix-macos.SHA256SUMS',
      extractFolder: 'InfoMatrix.app',
      executable: 'InfoMatrix.app',
    };
  }
  if (platform === 'linux' && arch === 'x64') {
    return {
      artifact: 'InfoMatrix-linux-x64.tar.gz',
      checksumFile: 'InfoMatrix-linux-x64.SHA256SUMS',
      extractFolder: 'InfoMatrix-linux',
      executable: path.join('InfoMatrix-linux', 'InfoMatrix'),
    };
  }
  if (platform === 'win32' && arch === 'x64') {
    return {
      artifact: 'InfoMatrix-windows-x64.zip',
      checksumFile: 'InfoMatrix-windows-x64.SHA256SUMS',
      extractFolder: '',
      executable: 'InfoMatrix.exe',
    };
  }
  throw new Error(`Unsupported platform/arch: ${platform}/${arch}. Supported: darwin/x64|arm64, linux/x64, win32/x64`);
}

function releaseUrl(version, filename) {
  return `https://github.com/${REPO}/releases/download/v${version}/${filename}`;
}

function executablePath(installDir, platform) {
  return path.join(installDir, resolvePlatform(platform, process.arch).executable);
}

module.exports = {
  REPO,
  resolvePlatform,
  releaseUrl,
  executablePath,
};
