'use strict';

const test = require('node:test');
const assert = require('node:assert');

const { resolvePlatform, releaseUrl, executablePath } = require('../lib/platform');

test('resolvePlatform supports darwin', () => {
  const p = resolvePlatform('darwin', 'arm64');
  assert.strictEqual(p.artifact, 'InfoMatrix-macos.zip');
});

test('resolvePlatform supports linux x64', () => {
  const p = resolvePlatform('linux', 'x64');
  assert.strictEqual(p.artifact, 'InfoMatrix-linux-x64.tar.gz');
});

test('resolvePlatform supports win32 x64', () => {
  const p = resolvePlatform('win32', 'x64');
  assert.strictEqual(p.artifact, 'InfoMatrix-windows-x64.zip');
});

test('resolvePlatform rejects unsupported platforms', () => {
  assert.throws(() => resolvePlatform('freebsd', 'x64'));
});

test('releaseUrl contains repo and version', () => {
  const url = releaseUrl('0.1.4', 'InfoMatrix-macos.zip');
  assert(url.includes('github.com/MengyangGao/infoMatrix/releases/download/v0.1.4/InfoMatrix-macos.zip'));
});

test('executablePath resolves macOS app', () => {
  Object.defineProperty(process, 'platform', { value: 'darwin' });
  const p = executablePath('/cache', 'darwin');
  assert.strictEqual(p, '/cache/InfoMatrix.app');
});
