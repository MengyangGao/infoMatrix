'use strict';

const { install, cacheDir, markerPath } = require('./install');
const { launch } = require('./launcher');
const { executablePath } = require('./platform');

module.exports = {
  install,
  launch,
  cacheDir,
  markerPath,
  executablePath,
};
