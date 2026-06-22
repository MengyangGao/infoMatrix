#!/usr/bin/env node
'use strict';

const { launch } = require('../lib/launcher');

launch(process.argv.slice(2)).then((code) => {
  process.exit(code ?? 0);
}).catch((err) => {
  console.error('Failed to launch InfoMatrix:', err.message);
  process.exit(1);
});
