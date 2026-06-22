'use strict';

const crypto = require('crypto');
const fs = require('fs');
const https = require('https');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const { resolvePlatform, releaseUrl } = require('./platform');

const VERSION = require('../package.json').version;

function cacheDir() {
  return path.join(os.homedir(), '.cache', 'infomatrix-cli', VERSION);
}

function markerPath() {
  return path.join(cacheDir(), '.installed');
}

function httpsGet(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { 'User-Agent': 'infomatrix-npm-installer' } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        resolve(httpsGet(res.headers.location));
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`Download failed: ${url} (HTTP ${res.statusCode})`));
        return;
      }
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', reject);
    }).on('error', reject);
  });
}

function sha256(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

async function fetchChecksum(version, checksumFile, artifact) {
  const url = releaseUrl(version, checksumFile);
  const text = (await httpsGet(url)).toString('utf8');
  for (const line of text.split(/\r?\n/)) {
    const parts = line.trim().split(/\s+/);
    if (parts.length === 2 && parts[1] === artifact) {
      return parts[0].toLowerCase();
    }
  }
  throw new Error(`Checksum for ${artifact} not found in ${checksumFile}`);
}

function extract(artifactPath, installDir, platform) {
  fs.mkdirSync(installDir, { recursive: true });
  if (platform === 'darwin') {
    run('unzip', ['-o', artifactPath, '-d', installDir]);
    const app = path.join(installDir, 'InfoMatrix.app');
    if (fs.existsSync(app)) {
      run('xattr', ['-dr', 'com.apple.quarantine', app], { stdio: 'ignore' });
    }
    return;
  }
  if (platform === 'linux') {
    run('tar', ['-xzf', artifactPath, '-C', installDir]);
    const exe = path.join(installDir, 'InfoMatrix-linux', 'InfoMatrix');
    if (fs.existsSync(exe)) {
      fs.chmodSync(exe, 0o755);
    }
    return;
  }
  if (platform === 'win32') {
    run('powershell.exe', [
      '-NoProfile',
      '-Command',
      `Expand-Archive -Path '${artifactPath}' -DestinationPath '${installDir}' -Force`,
    ]);
    return;
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

function run(cmd, args, options = {}) {
  const result = spawnSync(cmd, args, { stdio: 'pipe', ...options });
  if (result.status !== 0) {
    const err = (result.stderr || '').toString().trim() || `${cmd} exited ${result.status}`;
    throw new Error(err);
  }
}

async function install(options = {}) {
  const platform = options.platform || process.platform;
  const arch = options.arch || process.arch;
  const version = options.version || VERSION;
  const installDir = options.installDir || cacheDir();

  const { artifact, checksumFile } = resolvePlatform(platform, arch);
  const marker = markerPath();
  if (fs.existsSync(marker) && !options.force) {
    return { installDir, alreadyInstalled: true };
  }

  const artifactUrl = releaseUrl(version, artifact);
  const [artifactBuffer, expectedChecksum] = await Promise.all([
    httpsGet(artifactUrl),
    fetchChecksum(version, checksumFile, artifact),
  ]);

  const actualChecksum = sha256(artifactBuffer);
  if (actualChecksum !== expectedChecksum) {
    throw new Error(`Checksum mismatch for ${artifact}: expected ${expectedChecksum}, got ${actualChecksum}`);
  }

  fs.rmSync(installDir, { recursive: true, force: true });
  fs.mkdirSync(installDir, { recursive: true });

  const artifactPath = path.join(installDir, artifact);
  fs.writeFileSync(artifactPath, artifactBuffer);

  extract(artifactPath, installDir, platform);
  fs.unlinkSync(artifactPath);
  fs.writeFileSync(marker, '');

  return { installDir, alreadyInstalled: false };
}

module.exports = { install, cacheDir, markerPath };

if (require.main === module) {
  install().then(({ installDir, alreadyInstalled }) => {
    console.log(`InfoMatrix ${alreadyInstalled ? 'already installed' : 'installed'} at ${installDir}`);
  }).catch((err) => {
    console.error(err.message);
    process.exit(1);
  });
}
